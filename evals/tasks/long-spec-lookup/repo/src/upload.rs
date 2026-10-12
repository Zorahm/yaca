/// How many attempts an object gets before the worker gives up on it.
pub const MAX_ATTEMPTS: u32 = 3;

/// Whether another attempt is allowed after `attempt` attempts have failed.
pub fn may_retry(attempt: u32) -> bool {
    attempt < MAX_ATTEMPTS
}
