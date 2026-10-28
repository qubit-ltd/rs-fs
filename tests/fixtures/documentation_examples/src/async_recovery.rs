// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::future::Future;
use std::future::poll_fn;
use std::pin::pin;
use std::task::Poll;

use qubit_fs::AsyncFileSystem;
use qubit_fs::Path;
use qubit_fs::directory::ListFilter;
use qubit_fs::directory::ListOptions;
use qubit_fs::directory::ListScope;
use qubit_fs::error::FsError;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::write::AsyncWriteAllOperation;
use qubit_fs::write::AsyncWriteAllOperationFailure;
use qubit_fs::write::AsyncWriterRecovery;
use qubit_fs::write::WriteAbortOutcome;
use qubit_fs::write::WriteOptions;

/// Retains an asynchronous write operation when cleanup cannot finish.
///
/// This guide type keeps the original operation, primary failure, and cleanup
/// failure together so callers can reconcile publication state before retrying.
#[derive(Debug)]
pub struct WriteRecovery {
    /// The operation containing the write's publication and recovery state.
    pub operation: AsyncWriteAllOperation,
    /// The failure that triggered recovery, or `None` when cancellation did.
    pub primary: Option<AsyncWriteAllOperationFailure>,
    /// An error returned while aborting a retained writer, if any.
    pub cleanup_error: Option<FsError>,
    /// The provider's confirmed result after cleanup, if cleanup succeeded.
    pub cleanup_outcome: Option<WriteAbortOutcome>,
}

/// Describes whether report writing failed before execution or needs recovery.
#[derive(Debug)]
pub enum WriteReportError {
    /// The operation could not be prepared before asynchronous execution.
    Preflight(AsyncWriteAllOperationFailure),
    /// The operation and any cleanup failure needed for reconciliation.
    Recovery(Box<WriteRecovery>),
}

/// Writes report bytes and retains ambiguous outcomes for caller reconciliation.
///
/// Cancellation applies only to execution. Once execution completes or is
/// cancelled, any retained writer is aborted, and the operation state is
/// returned when the result cannot safely be treated as published.
pub async fn write_report(
    filesystem: &AsyncFileSystem,
    path: Path,
    bytes: Vec<u8>,
    cancel: impl Future<Output = ()>,
) -> Result<WriteOutcome, WriteReportError> {
    let mut operation = filesystem
        .begin_write_all(path, bytes, WriteOptions::default())
        .map_err(WriteReportError::Preflight)?;
    // Only the execute future enters the cancellation scope.
    let result = {
        let mut execution = pin!(operation.execute());
        let mut cancellation = pin!(cancel);
        poll_fn(|context| {
            if let Poll::Ready(result) = execution.as_mut().poll(context) {
                return Poll::Ready(Some(result));
            }
            if cancellation.as_mut().poll(context).is_ready() {
                return Poll::Ready(None);
            }
            Poll::Pending
        })
        .await
    };
    let primary = match result {
        Some(Ok(outcome)) => return Ok(outcome),
        Some(Err(failure)) => Some(failure),
        None => None, // The operation now records cancellation facts.
    };
    let cleanup = match operation.recovery() {
        Some(AsyncWriterRecovery::Opened(writer)) => Some(writer.abort_async().await),
        Some(AsyncWriterRecovery::Rejected(writer)) => Some(writer.abort_async().await),
        None => None,
    };
    let (cleanup_outcome, cleanup_error) = match cleanup {
        Some(Ok(outcome)) => (Some(outcome), None),
        Some(Err(error)) => (None, Some(error)),
        None => (None, None),
    };
    // Published means do not resend. Indeterminate requires reconciliation,
    // including when no writer was returned. A missing stat result is not
    // proof that a remote request cannot finish later.
    Err(WriteReportError::Recovery(Box::new(WriteRecovery {
        operation,
        primary,
        cleanup_error,
        cleanup_outcome,
    })))
}

/// Lists report keys across a configured flat namespace with a bounded result
/// set.
///
/// This guide example uses a literal prefix and caps the returned entries to
/// keep listing work bounded.
pub async fn list_reports(filesystem: &AsyncFileSystem) -> Result<Vec<Path>, FsError> {
    let scope = ListScope::Namespace;
    let options = ListOptions::object_keys()
        .with_filter(Some(ListFilter::LiteralPrefix("reports/".to_owned())))
        .with_max_entries(Some(1000));
    let mut stream = filesystem.list(&scope, options).await?;
    let mut paths = Vec::new();
    while let Some(entry) = stream.next_entry_async().await? {
        paths.push(entry.path);
    }
    Ok(paths)
}
