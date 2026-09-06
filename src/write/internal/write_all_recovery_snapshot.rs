//! Recovery snapshot for async whole-file writes.
use crate::write::WriteFailureState;
#[derive(Clone, Copy)]
pub(crate) struct WriteAllRecoverySnapshot {
    pub(crate) state: WriteFailureState,
    pub(crate) written_bytes: u64,
}
impl WriteAllRecoverySnapshot {
    pub(crate) const fn new() -> Self {
        Self {
            state: WriteFailureState::NotPublished,
            written_bytes: 0,
        }
    }
}
