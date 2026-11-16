//! Execution bindings and owned-operation contracts.
mod binding;
mod effect_observer;
mod failure;
mod prepared_adapter;
mod request;
pub use binding::FsExecutionBinding;
pub use effect_observer::AsyncFsMutationOperation;
pub use effect_observer::FsEffectCapacityError;
pub use effect_observer::FsEffectFact;
pub use effect_observer::FsEffectObservationError;
pub use effect_observer::FsEffectObserver;
pub use effect_observer::FsEffectReservation;
pub use effect_observer::FsEffectSlotPool;
pub use effect_observer::FsEffectSnapshot;
pub use effect_observer::FsEffectWait;
pub use effect_observer::FsMutationOperation;
pub use failure::FsOperationFailure;
pub use failure::FsOperationFailureKind;
pub use failure::FsPrepareFailure;
pub use failure::FsPrepareFailureKind;
pub use failure::FsSubmitFailure;
pub use prepared_adapter::PreparedFsCopyAdapter;
pub use prepared_adapter::PreparedFsReadAdapter;
pub use prepared_adapter::PreparedFsRenameAdapter;
pub use prepared_adapter::PreparedFsTreePublishAdapter;
pub use prepared_adapter::PreparedFsWriteAdapter;
pub use request::FsOwnedCopyRequest;
pub use request::FsOwnedReadRequest;
pub use request::FsOwnedRenameRequest;
pub use request::FsOwnedTreePublishRequest;
pub use request::FsOwnedWriteRequest;
/// Synchronous filesystem operation managed by `qubit-operation`.
pub type FsOperation<T, E = FsOperationFailure> = qubit_operation::OperationHandle<T, E>;
/// Asynchronous filesystem operation managed by `qubit-operation`.
pub type AsyncFsOperation<T, E = FsOperationFailure> = qubit_operation::AsyncOperationHandle<T, E>;
