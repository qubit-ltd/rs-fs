//! State for an owning asynchronous whole-file write.
use crate::write::WriteFailureState;
/// Lifecycle state of an owning asynchronous whole-file write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsyncWriteAllOperationState {
    /// The operation has not been polled.
    Ready,
    /// The provider call is in progress or was cancelled while pending.
    Running,
    /// The target was published successfully.
    Completed,
    /// The operation stopped with the paired publication state.
    Failed(WriteFailureState),
}
