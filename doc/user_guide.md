# Qubit FS User Guide

`qubit-fs` is the common contract between application code and filesystem
providers. It is intended for local filesystems, object stores, cloud drives,
remote protocols, distributed filesystems, in-memory implementations, and
provider-specific storage services.

The core crate contains no concrete backend. Adding a backend does not require
adding an enum variant or changing application code; the application registers
the provider it wants to assemble.

## Installation

```toml
[dependencies]
qubit-fs = "0.2"
```

File byte streams use the traits from `qubit-io`. Provider implementations also
use `qubit-spi` and commonly use `qubit-metadata`.

## The Object Model

The filesystem traits form this hierarchy:

```rust
use qubit_fs::{
    AsyncFileSystem,
    FileSystem,
    FileSystemProperties,
};
```

- `FileSystemProperties` contains `info()` and `capabilities()`. Both values are
  stable construction-time snapshots and getters must never perform I/O.
- `FileSystem: FileSystemProperties` contains synchronous operations.
- `AsyncFileSystem: FileSystemProperties` contains runtime-neutral async
  operations whose names end in `_async`.

A backend type may implement either operational trait or both. Sync and async
registries are separate because a provider may support only one mode.

`stat()` and `stat_async()` are live operations and may perform remote I/O.
They are intentionally distinct from `info()` and from the optional metadata
snapshot attached to an opened handle.

## URI and Provider-Local Paths

### `FsUri`

`FsUri` is the transport and provider selection representation. It preserves
information that must not be interpreted by the core layer:

- the raw percent-encoded path, including encoded separators such as `%2F`;
- ordered query pairs and duplicate query keys;
- whether the hierarchical `//` form was present, including the difference
  between `file:/tmp/a` and `file:///tmp/a`;
- an optional authority.

Fragments, malformed percent encoding, control characters, passwords, and
credential-like query keys are rejected, including signed-URL fields such as
`x-amz-signature`. Query values are non-secret provider options, not a
credential channel. Raw path text is RFC 3986 encoded syntax:
non-ASCII, whitespace, `?`, and `#` must be percent encoded. Component builders
also reject authority/path combinations that would serialize ambiguously.

```rust
use qubit_fs::FsUri;

let uri = FsUri::parse(
    "object://bucket/a%2Fb?tag=first&tag=second",
)?;

assert_eq!("object", uri.scheme().as_str());
assert_eq!("/a%2Fb", uri.path().as_encoded());
assert_eq!(vec!["first", "second"], uri.query().get_all("tag"));
# Ok::<(), qubit_fs::FsError>(())
```

The core never decodes an `FsUriPath` into an `FsPath`. The selected provider
owns that conversion because encoded separators, dot segments, object keys,
drive identifiers, and authority semantics vary between backends.

### `FsPath`

`FsPath` is a path already interpreted inside one configured filesystem.
`parse()` uses normalized hierarchical semantics; `parse_literal()` preserves
provider-specific object-key text.

```rust
use qubit_fs::{
    FsName,
    FsPath,
    RelativeFsPath,
};

let base = FsPath::parse("/work")?;
let child = base.child(&FsName::parse("result.bin")?);
let nested = base.join_relative(
    &RelativeFsPath::parse("2026/july/report.csv")?,
);

assert_eq!("/work/result.bin", child.as_str());
assert_eq!("/work/2026/july/report.csv", nested.as_str());
# Ok::<(), qubit_fs::FsError>(())
```

`FsName` and `RelativeFsPath` cannot represent absolute paths or parent escape.
APIs such as `TempDir::child()` use these types to make lexical escape
unrepresentable.

Do not use `std::path::Path` as a cross-provider model. A local provider may
convert to it internally after applying its own platform and sandbox rules.

## Registry and Complete Configuration

Create and populate registries during application assembly. Registry clones
share registrations and default provider selection.

```rust
use qubit_fs::{
    FileSystemRegistry,
    FsResult,
};

fn configure() -> FsResult<FileSystemRegistry> {
    let registry = FileSystemRegistry::default();
    // Backend crates register their self-described providers here.
    // qubit_fs_local::register(&registry)?;
    // qubit_fs_object::register(&registry)?;
    Ok(registry)
}
```

`FileSystemConfig` is the complete provider input:

- a required, secret-free `FsUri`;
- an optional explicit `ProviderSelection`;
- validated `NonSensitiveMetadata`-backed options;
- an optional `CredentialRef`.

`NonSensitiveMetadata` is the common boundary for config options,
filesystem/file metadata, write and directory metadata, and operation
diagnostics. Validation is key-based and recursive over top-level fields,
string maps, and JSON objects, including objects inside arrays. Its `Debug`
implementation prints keys only. Scalar contents cannot be classified
reliably, so providers must use non-sensitive values under ordinary keys and
place every secret behind `CredentialRef`.

```rust
use qubit_fs::{
    CredentialRef,
    FileSystemConfig,
    FileSystemRegistry,
    FsResult,
    FsUri,
};

fn open_configured(
    registry: &FileSystemRegistry,
) -> FsResult<()> {
    let config = FileSystemConfig::new(FsUri::parse(
        "object://bucket/reports/a.csv?region=cn-east-1",
    )?)
    .with_credentials(CredentialRef::Profile("reporting".into()));

    let resource = registry.resource(&config)?;
    println!("{}", resource.path());
    Ok(())
}
```

When selection is absent, the registry selects by URI scheme. The provider
receives the complete configuration and returns a `FileSystemResolution`
containing:

- the configured filesystem object;
- its decoded `FsPath`;
- a canonical credential-free `FsUri`.

The registry turns that result into `FileResource` or `AsyncFileResource`.
Convenience methods `resource_uri()` and `resource_uri_async()` construct an
empty-options configuration.

## File Identity and Metadata

`FileReader`, `FileWriter`, `AsyncFileReader`, and `AsyncFileWriter` are file
handles, not aliases for arbitrary byte streams. A provider explicitly creates
one from an already-open stream and an `OpenedFileInfo`.

Every opened handle has a fixed `FileLocation`:

- configured `FileSystemId`;
- provider-local path captured at open time;
- optional canonical, secret-free URI.

`OpenedFileInfo::metadata()` is an optional open-time snapshot. Providers should
populate it only when metadata was already obtained as part of opening; they
should not issue an extra remote `stat` just to fill the field. Use
`FileSystem::stat()` or `AsyncFileSystem::stat_async()` for a current view.
The extensible `FileMetadata::user_metadata` and `provider_metadata` fields are
`NonSensitiveMetadata`, so providers must construct them through validated
conversion or the supplied builder methods.

## Synchronous Reading

For bounded resources, `FileSystemExt::read_all()` and
`FileResource::read_all()` are convenient. For streaming reads, `FileReader`
implements `qubit_io::Input<Item = u8>`.

```rust
use qubit_fs::{
    FileResource,
    FsError,
    FsOperation,
    FsResult,
    ReadOptions,
};
use qubit_io::Input;

fn read_prefix(resource: &FileResource) -> FsResult<Vec<u8>> {
    let mut reader = resource.open_reader(&ReadOptions::default())?;
    let mut buffer = [0_u8; 4096];
    let count = reader.read(&mut buffer).map_err(|error| {
        FsError::from_io(error, FsOperation::Read)
            .with_path(resource.path().clone())
    })?;
    Ok(buffer[..count].to_vec())
}
```

`ReadOptions` expresses byte range, version conditions, and checksum policy.
When a requested option is mandatory and unsupported, a provider must return an
explicit filesystem error rather than silently ignore it.

## Synchronous Writing

`FileWriter` implements `qubit_io::Output<Item = u8>` and is also a provider
publication session. Write bytes, then explicitly call `commit()` or `abort()`.

```rust
use qubit_fs::{
    AtomicityRequirement,
    FileResource,
    FsError,
    FsOperation,
    FsResult,
    WriteDisposition,
    WriteOptions,
    WriteOutcome,
};
use qubit_io::Output;

fn replace(
    resource: &FileResource,
    bytes: &[u8],
) -> FsResult<WriteOutcome> {
    let options = WriteOptions {
        create_parent: true,
        disposition: WriteDisposition::CreateOrReplace,
        atomicity: AtomicityRequirement::Required,
        ..WriteOptions::default()
    };
    let mut writer = resource.open_writer(&options)?;
    if let Err(error) = writer.write_fully(bytes) {
        let _ = writer.abort();
        return Err(FsError::from_io(error, FsOperation::Write)
            .with_path(resource.path().clone()));
    }
    writer.commit()
}
```

`commit(&mut self)` deliberately does not consume the handle. A definite
failure keeps the writer open for retry or abort. If the provider cannot
determine whether publication occurred, it returns `FsErrorKind::Indeterminate`
and the writer enters `WriterState::Indeterminate`; the session is still
retained for explicit recovery.

Dropping an open synchronous writer attempts best-effort abort. An indeterminate
writer is never aborted automatically because publication or cleanup may have
already occurred. Drop-time cleanup cannot report failure, so callers needing
confirmation must call `abort()` themselves.

## Asynchronous Operations

Async filesystem operations return `FsFuture` and do not bind the core crate to
Tokio, `futures-io`, or another executor. Opening is asynchronous because
authentication, network connection, request negotiation, or multipart setup may
be required. A successful open returns an already-initialized async handle.

```rust
use qubit_fs::{
    AsyncFileResource,
    FsError,
    FsOperation,
    FsResult,
    ReadOptions,
};
use qubit_io::AsyncInputExt;

async fn read_prefix(
    resource: &AsyncFileResource,
) -> FsResult<Vec<u8>> {
    let mut reader =
        resource.open_reader_async(ReadOptions::default()).await?;
    let mut buffer = [0_u8; 4096];
    let count = reader.read_async(&mut buffer).await.map_err(|error| {
        FsError::from_io(error, FsOperation::Read)
            .with_path(resource.path().clone())
    })?;
    Ok(buffer[..count].to_vec())
}
```

Async writers implement `AsyncOutput<Item = u8>`:

```rust
use qubit_fs::{
    AsyncFileResource,
    FsError,
    FsOperation,
    FsResult,
    WriteOptions,
    WriteOutcome,
};
use qubit_io::AsyncOutputExt;

async fn write(
    resource: &AsyncFileResource,
    bytes: &[u8],
) -> FsResult<WriteOutcome> {
    let mut writer =
        resource.open_writer_async(WriteOptions::default()).await?;
    if let Err(error) = writer.write_fully_async(bytes).await {
        let _ = writer.abort_async().await;
        return Err(FsError::from_io(error, FsOperation::Write)
            .with_path(resource.path().clone()));
    }
    writer.commit_async().await
}
```

Async drop cannot wait for remote cleanup. `AsyncFileWriteSession::cancel_on_drop`
may perform only nonblocking local cancellation for a definitely open writer.
Once a commit or abort future has been polled, dropping it before completion
changes the writer to `Indeterminate`; dropping an unpolled future leaves the
state unchanged. No automatic cancellation is attempted for an indeterminate
writer. Await `abort_async()` whenever confirmed cleanup matters.

For bounded resources, `AsyncFileSystemExt::read_all_async()` and
`write_all_async()` provide the asynchronous counterparts of the synchronous
whole-resource helpers. `AsyncFileResource` exposes the same conveniences.

## Listing and Bound Resources

`DirectoryStream::next_entry()` and
`AsyncDirectoryStream::next_entry_async()` support incremental provider paging.
`DirectoryStreamExt::collect_entries()` and
`AsyncDirectoryStreamExt::collect_entries_async()` are suitable when the
complete listing intentionally fits memory.

`FileResource` and `AsyncFileResource` keep a decoded path bound to its owning
filesystem and expose stat, exists, list, open, create, delete, rename, and copy
conveniences. Keeping `FsPath` itself free of backend state makes it cheap,
comparable, and testable.

## Capabilities, Requirements, and Outcomes

Capabilities are typed guarantees:

```rust
use qubit_fs::{
    FileSystem,
    FileSystemCapability,
};

fn supports_atomic_rename(fs: &dyn FileSystem) -> bool {
    fs.capabilities().contains(FileSystemCapability::AtomicRename)
}
```

Capability snapshots describe what the configured filesystem guarantees, not
what a generic provider might sometimes attempt. `FileSystemLimits` carries
stable configured limits.

`AtomicityRequirement` has three meanings:

- `Required`: success must satisfy atomicity; unsupported guarantees fail
  before side effects;
- `Preferred`: try an atomic method, but a non-atomic success is allowed and
  must be reported;
- `NotRequired`: atomicity is not requested, though a provider may still use it.

Successful `WriteOutcome`, `RenameOutcome`, `CopyOutcome`, and `PersistOutcome`
report `AchievedAtomicity` and the actual method. A requested policy and a
completed outcome are intentionally different types.

Providers validate required guarantees before mutation. `ReadOptions`,
`WriteOptions`, `DeleteOptions`, `RenameOptions`, `CopyOptions`, and
`PersistOptions` expose `validate_against()` preflight. Missing range,
conditional, checksum, append, atomic-publication, recursive-delete, and
server-side-copy guarantees report the matching typed capability.
`WriteOptions::validate()` also rejects intrinsically contradictory requests,
such as `CreateNew` combined with `IfMatch`.

## Errors

`FsError` carries:

- a provider-neutral `FsErrorKind`;
- the `FsOperation`;
- optional source and target paths;
- optional provider identity;
- an optional required capability;
- the original source error.

Open, metadata, lifecycle, and namespace operations return `FsError`. Byte
transfer methods follow `qubit-io` and return `std::io::Error`.
`FsError::into_io_error()` and `FsError::from_io()` cross that boundary while
preserving the underlying source where possible.

`exists()` returns `Ok(false)` only for a confirmed `NotFound`. Authentication,
permission, timeout, and connectivity failures remain errors.

## Temporary Resources

There is no universal default temporary directory. A local filesystem, object
store, cloud drive, and distributed filesystem do not share a safe namespace,
lifecycle, or publication strategy. Providers therefore opt in by implementing
`create_temp_file` / `create_temp_dir` or their async counterparts and advertise
the corresponding capability.

Temporary handles own cleanup responsibility and expose:

- `cleanup` to delete the source and confirm cleanup;
- `keep` to transfer responsibility to the caller;
- `persist` to publish to a final path without consuming the handle.

`TempDir::child()` accepts `FsName`, and `descendant()` accepts
`RelativeFsPath`, preventing absolute-path replacement and lexical parent
escape.

Persist failure is typed:

| State | Meaning | Caller action |
| --- | --- | --- |
| `NotPublished` | target was not published; handle still owns source | retry with corrected input or clean up |
| `PublishedSourceRetained` | target is published; source still needs cleanup | do not republish; clean source |
| `Indeterminate` | provider cannot confirm final source/target state | reconcile externally; no automatic cleanup or retry |

Because `persist(&mut self, ...)` retains the handle, no temporary session is
lost on failure. Sync drop performs best-effort cleanup only for definite owned
states. Once an async cleanup, keep, or persist future has been polled, dropping
it before completion leaves the handle indeterminate and disables automatic
cancellation; an unpolled future has no lifecycle effect. Confirmed remote
cleanup must be awaited explicitly.

## Implementing a Provider

A synchronous provider is a self-described
`qubit_spi::ProviderDefinition<FileSystemSpec>` whose output is
`FileSystemResolution<dyn FileSystem>`. An asynchronous provider implements
`AsyncFileSystemProvider` independently.

A provider should:

1. validate the complete `FileSystemConfig`;
2. resolve `CredentialRef` through an external secret source;
3. decode the raw URI path according to provider semantics;
4. construct immutable `FileSystemInfo` and `FileSystemCapabilities`;
5. return the configured filesystem, decoded `FsPath`, and safe canonical URI;
6. create explicit file and lifecycle handles with stable identity;
7. reject unsupported guarantees before side effects;
8. report actual publication methods, atomicity, and partial progress;
9. attach operation, path, provider, capability, and source context to errors.

`FileSystemInfo::with_provider_metadata()` rejects credential-like keys at the
top level and recursively inside string maps and JSON objects because the
snapshot is debug-visible. The same invariant is enforced by
`NonSensitiveMetadata` for file metadata, options, and outcome diagnostics;
automatic `Debug` formatting exposes keys but not values. Resolve all secret
values outside the core model. `FsError` messages must likewise be scrubbed;
its `Debug` and `Display` implementations never expand the retained source
error. Explicit callers can still inspect that source through
`Error::source()`.

Default trait methods return `UnsupportedCapability` for optional operations.
Providers should override only the operations they genuinely support.
