# Qubit FS User Guide

`qubit-fs` separates application-facing filesystem facades from provider-facing
contracts. The crate has no built-in backend and does not select an async runtime.
Provider discovery, credentials, and configuration belong to
[`qubit-fs-registry`](https://crates.io/crates/qubit-fs-registry).

## Application and provider boundaries

Applications create a concrete facade from a provider implementation and use logical
`Path` values for every operation. Provider traits, requests, sessions, and envelopes are
available only from `qubit_fs::spi`.

```rust,ignore
use qubit_fs::{FileSystem, Path, ReadOptions};
use qubit_fs::spi::FileSystemSpi;

fn inspect<S: FileSystemSpi + 'static>(provider: S) -> qubit_fs::FsResult<()> {
    let filesystem = FileSystem::from_spi(provider)?;
    let path = Path::parse("/reports/2026/summary.csv")?;
    let _metadata = filesystem.stat(&path)?;
    let _reader = filesystem.open_reader(&path, ReadOptions::default())?;
    Ok(())
}
```

`FileSystemProperties` is an immutable construction-time snapshot containing provider
identity, capabilities, limits, and path constraints. `stat`, list, open, create, delete,
copy, rename, and temporary-resource operations may perform I/O.

## Paths and URIs

`Path` represents a validated logical resource name. `Path::parse` applies hierarchical
validation; `Path::parse_literal` retains provider-specific spelling when the configured
`PathSemantics` permits it. `RelativePath` and `PathComponent` build safe descendants without
allowing a parent escape.

`Uri` and `ConnectionUri` retain RFC 3986 lexical structure while rejecting fragments,
userinfo, and credential-bearing query fields. They are transport/configuration values, not a
replacement for a provider's logical `Path` validation. `UserMetadata` likewise rejects
credential-like keys and does not show values in `Debug` output.

## Synchronous I/O

Open a reader or writer directly from `FileSystem`. Handles retain the provider-opened identity
in `OpenedFileInfo`; use `info()` to inspect it without another provider call.

```rust,ignore
use qubit_fs::{FileSystem, Path, WriteOptions};
use qubit_io::Output;

fn replace(fs: &FileSystem, path: &Path, bytes: &[u8]) -> qubit_fs::FsResult<()> {
    let mut writer = fs.open_writer(path, WriteOptions::default())?;
    writer.write_fully(bytes).map_err(|error| {
        qubit_fs::FsError::from_io(error, qubit_fs::FsOperation::Write)
    })?;
    writer.commit().map_err(|failure| failure.into_error())?;
    Ok(())
}
```

`FileWriter::commit` returns a typed `WriteFailure`. Its state distinguishes retryable,
not-published, published, and indeterminate outcomes; retain the writer and call `abort` when
cleanup confirmation matters. `DirectoryStream::next_entry` is incremental, so applications
must impose their own collection limit.

`FileSystem::copy` and `FileSystem::rename` return typed `CopyFailure` and `RenameFailure`.
Required atomicity or durability is checked before side effects and successful outcomes report
the achieved guarantees rather than hiding provider downgrades.

## Asynchronous I/O

`AsyncFileSystem` mirrors the facade operations with runtime-neutral futures. Its provider
contract is `spi::AsyncFileSystemSpi`; callers await facade methods using their chosen runtime.

```rust,ignore
use qubit_fs::{AsyncFileSystem, Path, ReadOptions};

async fn inspect(fs: &AsyncFileSystem, path: &Path) -> qubit_fs::FsResult<()> {
    let _metadata = fs.stat(path).await?;
    let _reader = fs.open_reader(path, ReadOptions::default()).await?;
    Ok(())
}
```

`AsyncFileSystem::begin_copy` creates an `AsyncCopyOperation`. Dropping an execution future
after it has been polled records an indeterminate operation state without starting extra cleanup
I/O. Async writer and temporary-handle cleanup should be awaited explicitly whenever completion
must be confirmed.

## Temporary resources

`create_temp_file` and `create_temp_directory` return facade-owned handles. `TempFile`,
`TempDirectory`, `AsyncTempFile`, and `AsyncTempDirectory` expose explicit `cleanup`, `keep`,
and `persist` lifecycle operations. A persist failure preserves `NotPublished`,
`PublishedSourceRetained`, or `Indeterminate` so callers can retry, clean up, or reconcile.

## Errors and guarantees

`FsError` includes an error kind, operation, and available logical source/target context.
`exists` returns `false` only for an explicit not-found result; permission, authentication,
network, and timeout failures remain errors. Capability checks are local preflight whenever the
facade can determine that a requested guarantee cannot be met.
