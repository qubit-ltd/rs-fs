//! Execution bindings and owned-operation contracts.
mod binding;
mod effect_observer;
mod failure;
mod prepared_adapter;
mod request;
pub use binding::FsExecutionBinding;
pub use effect_observer::{
    AsyncFsMutationOperation, FsEffectCapacityError, FsEffectFact, FsEffectObservationError,
    FsEffectObserver, FsEffectReservation, FsEffectSlotPool, FsEffectSnapshot, FsEffectWait,
    FsMutationOperation,
};
pub use failure::{
    FsOperationFailure, FsOperationFailureKind, FsPrepareFailure, FsPrepareFailureKind,
    FsSubmitFailure,
};
pub use prepared_adapter::{
    PreparedFsCopyAdapter, PreparedFsReadAdapter, PreparedFsRenameAdapter,
    PreparedFsTreePublishAdapter, PreparedFsWriteAdapter,
};
pub use request::{
    FsOwnedCopyRequest, FsOwnedReadRequest, FsOwnedRenameRequest, FsOwnedTreePublishRequest,
    FsOwnedWriteRequest,
};
/// Synchronous filesystem operation managed by `qubit-operation`.
pub type FsOperation<T, E = FsOperationFailure> = qubit_operation::OperationHandle<T, E>;
/// Asynchronous filesystem operation managed by `qubit-operation`.
pub type AsyncFsOperation<T, E = FsOperationFailure> = qubit_operation::AsyncOperationHandle<T, E>;
