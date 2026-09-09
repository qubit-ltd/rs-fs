# Qubit FS User Guide

[中文指南](user_guide.zh_CN.md) · [README](../README.md)

This guide covers `qubit-fs` 0.4 for Rust 1.94 and later. It is for applications
that publish reports through a configured filesystem and need to retain recovery
facts when a write, copy, or cancellation does not complete normally.

## Model and setup

`FileSystem` and `AsyncFileSystem` are concrete application facades. Providers
implement `qubit_fs::spi`; registry discovery, configuration, and credentials
belong to `qubit-fs-registry`. The core has no backend and no async runtime.
`qubit-fs-local` supplies a synchronous host or rooted local filesystem.

`FileSystemProperties` is an immutable snapshot of identity, effective
capabilities, limits, path constraints, and symlink policy. Reading it performs
no I/O. Successful registry resolution validates static constraints; it does not
prove that a resource exists or that future I/O will succeed.

| Concept | Meaning |
| --- | --- |
| `Path` | Logical name in one configured filesystem; hierarchical and object-key namespaces have distinct semantics. |
| `Uri` | Secret-free canonical location interpreted with filesystem context; username-only authority is allowed, passwords, sensitive query fields and fragments are rejected. |
| `ConnectionUri` | Configuration ingress that can retain credentials; `Display` and `Debug` redact them. |
| Publication | Whether the requested target data became visible. |
| Cleanup | Whether staging/source resources have been released; this does not prove target rollback. |
| Recovery handle | Ownership of a session still needing a decision; its absence does not prove no side effects. |

The default feature set is empty and provides synchronous APIs. Enable
`qubit-fs = { version = "0.4", features = ["async"] }` for asynchronous APIs.
The application chooses its executor; the library does not require Tokio.

For the runnable local example, use:

```toml
[dependencies]
qubit-fs = "0.4"
qubit-fs-local = "0.6"
tempfile = "3"
```

## Publish and read a report

Put the following in `src/main.rs` and run `cargo run`. The example isolates its
files in a temporary rooted filesystem, writes a report, reads at most 1024 bytes,
and prints `report ready`. `tempfile` is only a convenience for this demo's root.
In an application, construct the provider with its configured existing root and
choose explicit resource budgets for the workload.

<!-- example: quick-start -->
```rust
use std::time::Duration;

use qubit_fs::Path;
use qubit_fs::read::ReadOptions;
use qubit_fs::write::WriteOptions;
use qubit_fs_local::LocalCopyResourceLimits;
use qubit_fs_local::LocalDeleteResourceLimits;
use qubit_fs_local::LocalFileSystems;
use qubit_fs_local::LocalListResourceLimits;
use qubit_fs_local::LocalResourcePolicy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let list_budget = LocalListResourceLimits::new(8, 64, 4 * 1024, 4, Duration::from_secs(5))?;
    let copy_budget = LocalCopyResourceLimits::new(8, 64, 1024 * 1024, 4, Duration::from_secs(5))?;
    let delete_budget = LocalDeleteResourceLimits::new(8, 64, 4 * 1024, Duration::from_secs(5));
    let policy = LocalResourcePolicy::bounded(list_budget, copy_budget, delete_budget);
    let filesystem = LocalFileSystems::rooted(directory.path(), policy)?;
    let path = Path::parse("/report.txt")?;
    filesystem.write_all(&path, b"report ready", WriteOptions::default())?;
    let bytes = filesystem.read_all(&path, ReadOptions::default(), 1024)?;
    assert_eq!(b"report ready", bytes.as_slice());
    println!("{}", String::from_utf8(bytes)?);
    Ok(())
}
```

Logical `/report.txt` is inside that root; it is not the host path `/report.txt`.
Use `host_path_to_logical` from `qubit-fs-local` when converting a native host
path. Do not feed arbitrary OS path text through logical path parsing.

For larger data, use `open_reader` or `open_writer` and bounded chunks.
`read_all` requires a byte limit and applies it to the selected range, rather
than the entire metadata length. `read_prefix` bounds the returned prefix.
Commit a writer explicitly; flushing alone does not confirm publication.

## Copy and retain recovery responsibility

A completed report can be copied using the following application helper. On
failure it returns the original `CopyFailure`, its publication state, partial
statistics, and any retained writer. An abort failure is stored separately.
The caller must keep the returned recovery object until it decides how to
reconcile or finish cleanup. The helper does not automatically repeat the copy.

<!-- example: sync-recovery -->
```rust
use qubit_fs::FileSystem;
use qubit_fs::Path;
use qubit_fs::copy::CopyFailure;
use qubit_fs::copy::CopyOptions;
use qubit_fs::copy::CopyOutcome;
use qubit_fs::error::FsError;

#[derive(Debug)]
pub struct CopyRecovery {
    pub failure: CopyFailure,
    pub cleanup_error: Option<FsError>,
}

pub fn copy_report(filesystem: &FileSystem, source: &Path, target: &Path) -> Result<CopyOutcome, CopyRecovery> {
    match filesystem.copy(source, target, CopyOptions::default()) {
        Ok(outcome) => Ok(outcome),
        Err(mut failure) => {
            // Abort handles the retained session; it does not promise target rollback.
            let cleanup_error = match failure.writer_mut() {
                Some(writer) => writer.abort().err(),
                None => None,
            };
            // Preserve the publication facts and writer even when cleanup fails.
            Err(CopyRecovery { failure, cleanup_error })
        }
    }
}
```

After success, enumerate the release path with `filesystem.list(path,
ListOptions::default())` and call `next_entry()` in a bounded loop. Import
`ListOptions` from `qubit_fs::directory`. Entries arrive incrementally and errors
can occur after earlier entries were processed. Record progress before fetching
the next entry; a listing is not an atomic snapshot.

Use `ListFilter::Subtree` for hierarchical subtrees. For flat object keys,
`ListOptions::object_keys().with_filter(ListFilter::LiteralPrefix(...))` performs
raw prefix matching without decoding or normalizing keys. A nonempty root is
required; providers can reject requests they cannot represent faithfully.

## Async writing and cancellation

`begin_write_all` consumes a `Vec<u8>` and returns an operation that owns a
filesystem clone, path, options, and payload. Borrowed input must be explicitly
copied with `to_vec()`. For streaming without a whole-file allocation, manage an
`AsyncFileWriter` directly. The removed asynchronous `write_all` has no wrapper.

The following complete helper takes an application cancellation future. Keep
this helper running through recovery; cancel the write through its `cancel`
argument, rather than dropping the entire helper during cleanup. The operation
lives outside the scope containing the execution future. Successful execution
wins if both execution and cancellation are ready on the same poll.

<!-- example: async-recovery -->
```rust
use std::future::Future;
use std::future::poll_fn;
use std::pin::pin;
use std::task::Poll;

use qubit_fs::AsyncFileSystem;
use qubit_fs::Path;
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
```

An explicit error is in `primary`; cancellation has `primary == None` and leaves
its state in `operation`. A failed abort leaves both `cleanup_error` and the
writer in the returned recovery object. Inspect `operation.filesystem()` and
`operation.path()` when no writer was returned. Neither a missing writer nor
one `stat` returning `NotFound` establishes that pending remote work cannot
publish later. Reconciliation depends on the provider's request identity and
completion guarantees.

| Execution event | Retained facts |
| --- | --- |
| Drop an unpolled execute future | `Ready`; no provider call; payload retained. |
| Cancel after execution starts | `Failed(Indeterminate)`; confirmed byte count and any opened writer remain. |
| Success | `Completed`; full confirmed byte count; committed writer and payload released. |
| Explicit failure | Typed publication state, confirmed byte count, and relevant recovery writer. |
| Execute again | `InvalidState`; no new provider call; historical state and count unchanged. |
| Take recovery writer | Responsibility transfers; later writer recovery does not rewrite the operation snapshot. |

`begin_copy` follows the same ownership principle for streamed async copy.
Retain its operation across cancellation. `Drop` does not start an executor or
perform required asynchronous cleanup; await cleanup when confirmation matters.

## Guarantees and error decisions

Writer opening only proves no side effects when the provider explicitly reports
`FsEffectState::Unchanged` without an indeterminate error. Missing effect,
`Applied`, `PartiallyApplied`, or indeterminate evidence becomes `Indeterminate`
for the whole write/copy. Applied opening is not completed file publication.
`AlreadyExists + Skip` is successful only with explicit unchanged evidence.
Preflight rejection before provider I/O is known not to publish.

Commit failures retain their stage-specific publication facts. `Published` is
not a reason to resend; cleanup may still be necessary. Never infer safe retry
from an error kind alone. `exists` returns false only for `NotFound`; permission,
authentication, timeout, and I/O failures remain errors.

Requests distinguish optional and required guarantees. `WriteOutcome`,
`RenameOutcome`, and `CopyOutcome` expose durability facts. `PersistOutcome`
reports publication and cleanup but has no durability field; successful temporary
persistence alone does not establish durable publication. Atomicity and durability are different; neither implies
the other. Required guarantees are checked before I/O when statically impossible
and against provider outcomes after completion. A violated guarantee after known
publication does not change that publication into an unchanged result.

The facade can perform allowlisted regular-file stream copy without native
copy, or after a provider explicitly declines without effects. It does not
retry a failed native copy. Required server-side, atomic, or durable copy,
tree mode, and a changed symlink policy cannot use that fallback.
`CopyOptions::deadline` is a cooperative cumulative budget starting at operation
construction. It is checked around stages; it is not a timer that interrupts
an arbitrary pending provider future. Provider failures remain the primary error.

Temporary files and directories retain explicit lifecycle ownership. Their
`cleanup`, `keep`, and `persist` methods report what happened. Persistence has
five failure states: `NotPublished`, `NotPublishedSourceReleased`,
`PublishedSourceRetained`, `PublishedSourceReleased`, and `Indeterminate`.
`PersistFailure::publication_target()` preserves the confirmed published target.
Do not treat released source ownership as proof that publication did not happen.

## Diagnosis and operational limits

- Missing backend: configure a provider or registry resolution; the core does not select one.
- Unsupported requirement: inspect effective capabilities and request options; changing providers or requirements is an application decision.
- Partial listing: preserve processed-entry progress and account for provider consistency; do not assume a snapshot.
- Uncertain write/copy: retain operation and errors, inspect publication facts, then reconcile without implicit retry.
- URI rejection: keep credentials at the configuration boundary. `expose_unredacted` is for controlled provider consumption, never logging or cache keys.
- Custom secret names: use an explicit redaction policy. The standard policy remains a mandatory floor; providers must remove unrecognized private credentials before creating a canonical `Uri`.
- Resource usage: `Vec` ownership makes whole-file memory cost explicit. Choose limits for reads, listings, copies and temporary resources; advertised limits are not aggregate quotas across concurrent requests.

No portable contract turns object keys into hierarchical paths, guarantees every
provider capability, or implements cross-filesystem move. Platform behavior and
rooted authority are provided by the backend.

## Further reading

- [Architecture design](file_system_design.md)
- [API reference](https://docs.rs/qubit-fs)
