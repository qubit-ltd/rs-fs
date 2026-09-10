# Qubit FS User Guide

[中文指南](user_guide.zh_CN.md) · [README](../README.md)

This guide covers `qubit-fs` 0.7 for Rust 1.94 and later. It is for applications
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
`qubit-fs = { version = "0.7", features = ["async"] }` for asynchronous APIs.
The application chooses its executor; the library does not require Tokio.

For the runnable local example, use:

```toml
[dependencies]
qubit-fs = "0.7"
qubit-fs-local = "0.8"
tempfile = "3"
```

## Listing scopes and bounded reads

Pass `ListScope::Path(path)` to list a hierarchical directory or a raw flat-key
prefix. Use `ListScope::Namespace` to list the entire configured flat namespace;
it is rejected for hierarchical filesystems, whose root is `ListScope::Path(Path::root())`.
`Path` still rejects empty strings. Namespace does not permit access beyond the
configured filesystem, and cannot be used to open, stat, or write a resource.

For flat keys, `LiteralPrefix` is relative to the selected scope. A root `folder/`
and filter `a` match `folder/a` and `folder/ab`. A root `folder` also matches
`folderish`; no separator is inserted and no key text is normalized. Namespace
filters match complete logical keys. The combined root/filter text is checked
against the provider's path-text limit before opening a stream.

Listing deadlines start when the stream is constructed and are checked before
and after each provider call. A successful entry or EOF arriving at the deadline
is rejected; a real provider failure retains its category and source. Checks are
cooperative and cannot interrupt a permanently pending provider future. Creating
and dropping an unpolled next-entry future does not change stream state.

`read_prefix` opens once and consumes at most the requested prefix without an
extra stat. It inserts or narrows a provider range only for **Guaranteed**
`RangeRead`, no checksum request, a positive prefix length, and representable
provider limits. Original options are validated first. Conditional or unsupported
range capabilities still permit sequential prefix reads. BestEffort checksum
preserves the original request; Required checksum is rejected with
`RequirementNotMet` because a prefix cannot prove complete checksum validation.
Use a complete `read_all` when that guarantee is needed. Return and consumption
bounds do not promise an identical bound on provider network prefetch.


## Publish and read a report

Put the following in `src/main.rs` and run `cargo run`. The example isolates its
files in a temporary rooted filesystem, writes a report, reads at most 1024 bytes,
and prints `report ready`. `tempfile` is only a convenience for this demo's root.
In an application, construct the provider with its configured existing root and
choose explicit resource budgets for the workload.

<!-- example: quick-start -->
```rust
use qubit_fs::Path;
use qubit_fs::directory::ListScope;
use qubit_fs::read::ReadOptions;
use qubit_fs::write::WriteOptions;
use qubit_fs_local::LocalFileSystems;
use qubit_fs_local::LocalResourcePolicy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let policy = LocalResourcePolicy::standard();
    let filesystem = LocalFileSystems::rooted(directory.path(), policy)?;
    let path = Path::parse("/report.txt")?;
    filesystem.write_all(&path, b"report ready", WriteOptions::default())?;
    let bytes = filesystem.read_all(&path, ReadOptions::default(), 1024)?;
    assert_eq!(b"report ready", bytes.as_slice());
    let scope = ListScope::Path(Path::root());
    let mut entries = filesystem.list(&scope, Default::default())?;
    assert_eq!(entries.next_entry()?.expect("published report").path, path);
    assert!(entries.next_entry()?.is_none());
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
use qubit_fs::write::WriterRecovery;

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
            let cleanup_error = match failure.recovery_mut() {
                Some(WriterRecovery::Opened(writer)) => writer.abort().err(),
                Some(WriterRecovery::Rejected(writer)) => writer.abort().err(),
                None => None,
            };
            // Preserve the publication facts and writer even when cleanup fails.
            Err(CopyRecovery { failure, cleanup_error })
        }
    }
}
```

After success, enumerate the release path with `filesystem.list(&ListScope::Path(path.clone()),
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
use qubit_fs::directory::ListFilter;
use qubit_fs::directory::ListOptions;
use qubit_fs::directory::ListScope;
use qubit_fs::error::FsError;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::write::AsyncWriteAllOperation;
use qubit_fs::write::AsyncWriterRecovery;
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
    let cleanup_error = match operation.recovery() {
        Some(AsyncWriterRecovery::Opened(writer)) => writer.abort_async().await.err(),
        Some(AsyncWriterRecovery::Rejected(writer)) => writer.abort_async().await.err(),
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
`cleanup`, `keep`, and `persist` methods report what happened. In 0.7, the
recovery snapshot has two separate axes: the retained target-publication fact
and the source's current qualification.

| `PersistFailureState` | Retained target fact | Source qualification | Safe next step |
| --- | --- | --- | --- |
| `NotPublished` | Not published | Owned | Correct the target and retry, or clean up |
| `NotPublishedSourceIndeterminate` | Not published | Indeterminate | Read-only reconciliation; do not delete a replacement |
| `NotPublishedSourceCleanupRequired` | Not published | Cleanup required | Retry cleanup only |
| `NotPublishedSourceReleased` | Not published | Released | No source operation remains |
| `PublishedSourceRetained` | Published | Cleanup required | Keep the target; clean the retained source |
| `PublishedSourceIndeterminate` | Published | Indeterminate | Keep the target fact and reconcile source read-only |
| `PublishedSourceReleased` | Published | Released | Do not publish again |
| `Indeterminate` | Indeterminate | Indeterminate | Reconcile without automatic retry |

`PersistFailure::publication_target()` is the last positively confirmed target,
not merely the target named by the failing call. A later invalid retry is rejected
before provider I/O, while its facade failure may retain an earlier
`PublishedSource*` state and target. That snapshot does not mean the retry
published again. Cleanup errors and asynchronous cancellation preserve the same
target history. Once source qualification is indeterminate, a later path or
option error cannot make it owned again. Inspect `failure.state()`,
`failure.publication_target()`, and the handle's `TempResourceState` before
choosing retry, cleanup, or read-only reconciliation.

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

## Opening failures and recovery in 0.6

`open_writer`, `create_temp_file`, and `create_temp_directory` return
`OpenFailure<R>` in both facades. `Preflight` and `ProviderOpen` failures have no
recovery session. `OutcomeValidation` means a provider returned a session with an
invalid identity: the error owns a `RejectedWriter`, `RejectedAsyncWriter`,
`RejectedTempResource`, or `RejectedAsyncTempResource`. These handles expose only
explicit abort/cleanup, never writing, commit, keep, persist, or a raw session.
Keep the error or transfer its session with `take_recovery`; converting it into
an ordinary `FsError` would lose recovery responsibility and is not provided.

Cleanup errors and polled-future cancellation retain the session and record
`RecoveryCleanupState::Indeterminate`; an unpolled cleanup future changes nothing.
A confirmed cleanup records `Completed`; another cleanup returns `InvalidState`
without provider I/O. An indeterminate abort is not confirmation. Dropping a
rejected handle does not initiate cleanup or `cancel_on_drop`. Cleanup uses the
session's actual ownership, not an unvalidated diagnostic path. Public cleanup
context contains only the configured provider and known request path; a temporary
request without a parent has no invented path. Preserve the original failure and
any cleanup error together.

Whole-write and copy recovery uses `WriterRecovery` / `AsyncWriterRecovery`:
match `Opened` for a validated writer and `Rejected` for cleanup-only authority.
Use `recovery`, `recovery_mut` where available, and `take_recovery` instead of the
removed writer-only accessors. Synchronous `WriteAllFailure::state()` and
`written_bytes()` capture immutable failure facts, including short writes and
exact commit states; abort or transferring recovery never changes those facts.
The failure's `into_parts` returns error, state, confirmed bytes, and recovery.
A copy collision during opening can count as Skip only when the provider failed with proven
unchanged effects and no rejected session. Invalid opened identities remain
indeterminate failures.

The core cannot retain a session a provider has not returned. Providers remain
responsible for resources created internally during failed or cancelled opening.

## Read windows and allocation limits

`ReadOptions::validate()` rejects explicit offset + length overflow as
`InvalidOptions` before capability checks, prefix optimization, or provider I/O.
An omitted offset means zero. A zero-length request still opens or checks the
resource; missing resources and permission failures remain errors. Windows at or
beyond EOF are empty, and windows crossing EOF return the available suffix.
Returned metadata describes the full resource, not the window or a snapshot.

Both sync and async `read_all` and `read_prefix` use fallible geometric buffer
growth. Metadata is a hint, not an allocation instruction: an enormous hint does
not reserve the whole object. Reservation failures return `ResourceLimitExceeded`
with the allocation error as source. `read_all` may consume one extra byte to
prove the limit was exceeded; `read_prefix` never probes past its prefix limit.
These are returned-length and consumption bounds, not process RSS bounds or
limits on provider/network prefetch. The local provider advertises conditional
range support, so automatic prefix narrowing still requires a provider advertising
`RangeRead` as Guaranteed.
