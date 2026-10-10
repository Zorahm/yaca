//! Who is shown images, and what happens when a model turns one down.
//!
//! An image is not like other input. Once it is in the history it is in every
//! request after, so a model that cannot take it does not fail once — it fails
//! every time, and the conversation is over. Whether a model can take it is
//! often not known: the bare OpenAI shape lists ids and nothing else.
//!
//! So the images stay in the history, and what a model that cannot see is
//! sent is the history with a note where each one was. Which models those are
//! comes from the provider where it says ([`ModelInfo::supports_images`]) and
//! from experience where it does not: a request with images that is refused
//! outright is sent once more without them, and if that goes through, the
//! model is not shown images again in this session. Like the name of the
//! output cap (`provider::openai`), it is asked rather than configured.

use tokio_util::sync::CancellationToken;

use crate::error::ProviderError;
use crate::message::{has_images, without_images, Message};
use crate::provider::{Provider, ProviderRequest, ProviderStream};
use crate::usage::ModelInfo;

use super::{retry, Shared};

impl Shared {
    /// Whether `model` is shown images.
    pub(super) fn sees(&self, model: &ModelInfo) -> bool {
        model.supports_images && !self.blind.lock().unwrap().contains(&model.id)
    }

    /// The history as `model` is to be given it.
    pub(super) fn shown_to(&self, model: &ModelInfo, history: &[Message]) -> Vec<Message> {
        if self.sees(model) {
            history.to_vec()
        } else {
            without_images(history)
        }
    }

    /// Opens a stream, with the retries of §5.10 and the one above.
    pub(super) async fn open(
        &self,
        provider: &dyn Provider,
        request: ProviderRequest,
        cancel: &CancellationToken,
    ) -> Result<ProviderStream, ProviderError> {
        let scheduled = |event| self.emit(event);
        if !has_images(&request.messages) {
            return retry::open(provider, request, cancel, scheduled).await;
        }

        let refused = match retry::open(provider, request.clone(), cancel, scheduled).await {
            Err(err) if err.is_unacceptable() => err,
            other => return other,
        };

        let mut blind = request;
        blind.messages = without_images(&blind.messages);
        let model = blind.model.clone();
        match retry::open(provider, blind, cancel, scheduled).await {
            Ok(stream) => {
                self.blind.lock().unwrap().insert(model);
                Ok(stream)
            }
            // Refused either way, so the images were not why. The first
            // answer is the one about the request as it was meant.
            Err(_) => Err(refused),
        }
    }
}
