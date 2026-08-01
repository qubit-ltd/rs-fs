# Qubit FS User Guide

`qubit-fs` 0.2.0 is a provider-neutral filesystem abstraction for Rust 1.94 or
later. It provides synchronous and asynchronous application facades without a
storage backend or an async-runtime dependency.

## Purpose and audience

This guide is for application developers who need a configured filesystem but
do not want to couple filesystem operations to a provider implementation. It
explains how to use `FileSystem` and `AsyncFileSystem`, and how to make safe
decisions after an operation reports a recoverable failure.

Providers implement extension contracts in `qubit_fs::spi`. Provider discovery,
configuration, and credentials belong to `qubit-fs-registry`; the core crate
does not discover a provider or select a runtime by itself.

## Conceptual model

```text
application
  │  uses FileSystem / AsyncFileSystem and logical Path values
  ▼
qubit-fs facade ─────────────► handles, options, outcomes, typed failures
  │
  └── qubit_fs::spi ◄──────── provider implementation

qubit-fs-registry ───────────► provider discovery, configuration, credentials
```

A `FileSystem` or `AsyncFileSystem` is one configured filesystem. A provider
may yield several facades when its endpoint, bucket, root, region, or credential
profile differs. `FileSystemProperties` is an immutable snapshot of identity,
capabilities, limits, and path constraints; reading it performs no I/O.

### Names, addresses, and secrets

| Type | Role | Credential boundary |
| --- | --- | --- |
| `Path` | Validated logical name inside one configured filesystem | Not a cross-filesystem address; facade operations also validate it against that filesystem's constraints. |
| `Uri` | Canonical, secret-free address for persistence and selection | Rejects userinfo, credential-like query fields, and fragments; preserves RFC 3986 lexical distinctions. |
| `ConnectionUri` | Registry/configuration ingress | May carry connection credentials, but `Display` and `Debug` redact them. Do not log or persist the original connection text. |

`ConnectionUri` lets the registry or provider consume credentials at a controlled
boundary and create a safe canonical `Uri`. It does not permit secrets in
metadata, logs, normal URI values, or error messages.

## Install and obtain a facade

```toml
[dependencies]
qubit-fs = "0.2.0"
```

Obtain a configured `FileSystem` or `AsyncFileSystem` from provider setup or a
registry integration. The following public construction boundary is useful to
provider implementations and focused tests:

```rust,ignore
use qubit_fs::{FileSystem, Path, ReadOptions};
use qubit_fs::spi::FileSystemSpi;

fn inspect<S: FileSystemSpi + 'static>(provider: S) -> qubit_fs::FsResult<()> {
    let fs = FileSystem::from_spi(provider)?;
    let report = Path::parse("/reports/2026/summary.csv")?;
    let _metadata = fs.stat(&report)?;
    let _reader = fs.open_reader(&report, ReadOptions::default())?;
    Ok(())
}
```

Application code should remain on `FileSystem` and `AsyncFileSystem`. Provider
traits, requests, sessions, and envelopes are under `qubit_fs::spi`.

## A real workflow: publish a daily report

Suppose a job copies a completed report to a release location and then enumerates
the release directory for downstream work. The decision after failure depends on
what its typed state proves—not merely on whether an error occurred.

### Synchronous workflow

```rust,ignore
use qubit_fs::{CopyOptions, FileSystem, ListOptions, Path};

fn publish(fs: &FileSystem, source: &Path, release_dir: &Path) -> qubit_fs::FsResult<()> {
    let target = Path::parse("/releases/2026-07-30/summary.csv")?;

    match fs.copy(source, &target, CopyOptions::default()) {
        Ok(_outcome) => {}
        Err(failure) => {
            // Record failure.state() and failure.partial_stats(). Retain a
            // recovery writer, when present, until its state is resolved.
            let (error, _, _, _) = failure.into_parts();
            return Err(error);
        }
    }

    let mut entries = fs.list(release_dir, ListOptions::default())?;
    while let Some(entry) = entries.next_entry()? {
        // Process one entry and impose an application-specific bound.
        let _path = entry.path;
    }
    Ok(())
}
```

`DirectoryStream::next_entry` enumerates incrementally. This avoids an unbounded
in-memory collection, but an error can arrive after earlier entries were
processed. Make downstream work idempotent or record a checkpoint before asking
for the next entry.

For writes, `open_writer` returns a `FileWriter`; write bytes and call `commit`.
A failed commit returns `WriteFailure`, which distinguishes retryable/not-
published, published, and indeterminate facts. `write_all` preserves its writer
in typed `WriteAllFailure` when recovery is required. `rename` returns
`RenameFailure`; `copy` returns `CopyFailure` with partial statistics and, when
applicable, a recovery writer. Inspect the state before retrying, calling
`abort`, cleaning up, or reconciling source and target.

Required atomicity and other declared guarantees are checked before side effects
when the facade can determine they cannot be met. Durability is currently a
copy-only guarantee (`DurableCopy`); write, rename, and temporary persistence
outcomes do not claim durable publication. A successful outcome reports only
the guarantees it models.

### Asynchronous workflow

`AsyncFileSystem` mirrors the facade through runtime-neutral futures. Run them
on the runtime already used by the application; `qubit-fs` does not impose Tokio,
`futures-io`, or another executor.

`write_all` is also available asynchronously and returns `AsyncWriteAllFailure`
when it must retain an `AsyncFileWriter` for recovery.

```rust,ignore
use qubit_fs::{AsyncFileSystem, CopyOptions, Path};

async fn publish_async(
    fs: &AsyncFileSystem,
    source: Path,
    target: Path,
) -> qubit_fs::FsResult<()> {
    let mut operation = fs.begin_copy(source, target, CopyOptions::default())?;
    match operation.execute().await {
        Ok(_outcome) => Ok(()),
        Err(failure) => {
            // Retain operation and inspect failure.state() for recovery.
            let (error, _, _) = failure.into_parts();
            Err(error)
        }
    }
}
```

`begin_copy` returns `AsyncCopyOperation` because streamed copy can retain a
recoverable async writer. Call `execute(&mut self).await` and retain the
operation until recovery is resolved. If an execution future has been polled
and then dropped before completion, the operation records an indeterminate
state; dropping an unpolled execution future leaves it ready. Explicitly await
async writer and temporary-resource cleanup when completion must be confirmed.

## Errors, diagnosis, and recovery

`FsError` contains an error kind and operation, plus available logical path,
source, target, and provider context. Start with the kind and operation, then
use a typed failure state to choose a safe next action.

| Situation | What the API says | Typical next action |
| --- | --- | --- |
| `exists` returns `Ok(false)` | `stat` explicitly returned `NotFound` | Treat the resource as absent. |
| `exists` returns `Err` | The cause was not `NotFound` | Handle it; permission, authentication, timeout, and I/O do not mean absence. |
| Copy/rename/write fails | Typed state, and for copy partial statistics plus possible writer recovery | Retry only when the state supports it; otherwise abort, clean up, or reconcile. |
| Temp persistence fails | `PersistFailureState` is `NotPublished`, `PublishedSourceRetained`, or `Indeterminate` | Retain ownership, clean up a retained source, or reconcile before republishing. |
| A guarantee is unavailable | Capability/requirement failure can be found before side effects | Change provider/options or relax the requirement. |

Temporary files and directories are facade-owned handles. `TempFile`,
`TempDirectory`, and their async counterparts expose explicit `cleanup`, `keep`,
and `persist` operations. Their states remain meaningful after recoverable
failure. Do not rely on `Drop` for an I/O operation whose completion matters.

## Troubleshooting

**No filesystem is available.** This is expected when only `qubit-fs` is present:
the crate has no backend or provider selection. Configure a provider through the
registry or provider integration, then obtain a concrete facade.

**A URI fails validation or logs show masked values.** Keep `ConnectionUri` at
configuration ingress, convert it through the controlled registry/provider path,
and store the resulting `Uri`. Canonical `Uri` values cannot contain userinfo,
credential-like query fields, or fragments.

**`exists` did not return `false`.** Only `NotFound` maps to absence. Permission,
authentication, network, timeout, and I/O errors mean that existence was not
established.

**A listing stopped partway through.** Directory enumeration is incremental, not
an atomic snapshot or preloaded vector. Preserve progress in the application and
resume or restart from a safe checkpoint.

**A copy, write, rename, or temp publish may have had side effects.** Read the
typed state first. `Published` and `Indeterminate` need reconciliation; retain
any writer or temp handle until the recovery decision is complete.

## Limits and non-goals

- The core crate does not ship local, remote, or object-storage backends.
- It does not discover providers, manage credentials, or bind to an async runtime.
- It does not promise every provider supports every operation or guarantee.
- It does not turn object keys into hierarchical paths or normalize provider
  semantics outside the public contracts.
- It does not make incremental directory enumeration complete or atomic.

## Further reading

- [README](../README.md) · [中文 README](../README.zh_CN.md)
- [中文用户指南](user_guide.zh_CN.md)
- [Architecture design (Chinese)](file_system_design.zh_CN.md)
- [API reference](https://docs.rs/qubit-fs)
