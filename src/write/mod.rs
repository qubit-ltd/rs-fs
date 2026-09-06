// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! File write options, handles, outcomes, and facade operation objects.

#[cfg(feature = "async")]
mod async_file_writer;
#[cfg(feature = "async")]
mod async_write_all_failure;
#[cfg(feature = "async")]
mod async_write_all_operation;
#[cfg(feature = "async")]
mod async_write_all_operation_failure;
#[cfg(feature = "async")]
mod async_write_all_operation_state;
mod file_writer;
#[cfg(feature = "async")]
mod internal;
mod write_abort_outcome;
mod write_all_failure;
mod write_disposition;
mod write_failure;
mod write_failure_state;
mod write_operation;
mod write_options;
mod write_precondition;
mod writer_state;

#[cfg(feature = "async")]
pub use async_file_writer::AsyncFileWriter;
#[cfg(feature = "async")]
pub use async_write_all_failure::AsyncWriteAllFailure;
#[cfg(feature = "async")]
pub use async_write_all_operation::AsyncWriteAllOperation;
#[cfg(feature = "async")]
pub use async_write_all_operation_failure::AsyncWriteAllOperationFailure;
#[cfg(feature = "async")]
pub use async_write_all_operation_state::AsyncWriteAllOperationState;
pub use file_writer::FileWriter;
pub use write_abort_outcome::WriteAbortOutcome;
pub use write_all_failure::WriteAllFailure;
pub use write_disposition::WriteDisposition;
pub use write_failure::WriteFailure;
pub use write_failure_state::WriteFailureState;
pub(crate) use write_operation::WriteOperation;
pub use write_options::WriteOptions;
pub use write_precondition::WritePrecondition;
pub use writer_state::WriterState;
