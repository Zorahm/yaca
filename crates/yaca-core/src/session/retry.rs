//! Opening a stream, and trying again when the provider says to (§5.10).
//!
//! Apart from the turn because the turn is not the only one asking: a
//! subagent talks to the same provider and meets the same 429s.

use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::error::ProviderError;
use crate::event::AgentEvent;
use crate::provider::{Provider, ProviderRequest, ProviderStream};

const MAX_ATTEMPTS: u32 = 5;

/// `scheduled` is told about each wait, as the event §5.10 draws a line from.
pub(crate) async fn open(
    provider: &dyn Provider,
    request: ProviderRequest,
    cancel: &CancellationToken,
    scheduled: impl Fn(AgentEvent),
) -> Result<ProviderStream, ProviderError> {
    let mut attempt = 1;
    loop {
        match provider.stream(request.clone()).await {
            Ok(stream) => return Ok(stream),
            Err(err) if err.is_retryable() && attempt < MAX_ATTEMPTS => {
                let delay = backoff(attempt, retry_after(&err));
                scheduled(AgentEvent::RetryScheduled {
                    attempt: attempt + 1,
                    of: MAX_ATTEMPTS,
                    delay_ms: delay.as_millis() as u32,
                    reason: err.to_string(),
                });
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = cancel.cancelled() => return Err(err),
                }
                attempt += 1;
            }
            Err(err) => return Err(err),
        }
    }
}

fn retry_after(err: &ProviderError) -> Option<Duration> {
    match err {
        ProviderError::Http { retry_after_s, .. } => retry_after_s.map(Duration::from_secs),
        _ => None,
    }
}

/// Провайдер сказал, когда возвращаться, — слушаемся. Иначе удвоение
/// с потолком: пять попыток укладываются примерно в полминуты.
fn backoff(attempt: u32, hinted: Option<Duration>) -> Duration {
    if let Some(d) = hinted {
        return d.min(Duration::from_secs(60));
    }
    Duration::from_millis(500 * 2u64.pow(attempt.min(6) - 1)).min(Duration::from_secs(30))
}
