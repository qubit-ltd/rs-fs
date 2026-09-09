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
use qubit_fs::write::WriteOptions;

#[derive(Debug)]
pub struct WriteRecovery {
    pub operation: AsyncWriteAllOperation,
    pub primary: Option<AsyncWriteAllOperationFailure>,
    pub cleanup_error: Option<FsError>,
}

#[derive(Debug)]
pub enum WriteReportError {
    Preflight(AsyncWriteAllOperationFailure),
    Recovery(Box<WriteRecovery>),
}

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
    let cleanup_error = match operation.recovery_writer() {
        Some(writer) => writer.abort_async().await.err(),
        None => None,
    };
    // Published means do not resend. Indeterminate requires reconciliation,
    // including when no writer was returned. A missing stat result is not
    // proof that a remote request cannot finish later.
    Err(WriteReportError::Recovery(Box::new(WriteRecovery {
        operation,
        primary,
        cleanup_error,
    })))
}

/// Lists report keys across a configured flat namespace with a bounded result set.
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
