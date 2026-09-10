# Provider implementation guide

[中文版本](provider_guide.zh_CN.md)

This guide targets `qubit-fs` 0.6 provider authors. It explains the smallest
provider that can be trusted by the `FileSystem` facade, and the checks to run
before publishing an adapter. It is not a backend tutorial, a credential
manager, or a promise that every backend supports every operation.

## 1. Facade and SPI trust boundary

Applications call `FileSystem` (or `AsyncFileSystem`); providers implement
`FileSystemSpi` under `qubit_fs::spi`. `ProviderProperties` is the provider's
declaration. The facade validates that declaration and exposes the resulting
`FileSystemProperties` view to applications. Keep the declaration immutable and
return a clone of the cached value from `properties()`.

## 2. Cache `ProviderProperties`

Construct and validate `ProviderProperties` once during provider construction.
Do not reconstruct it on every call or use `expect` to defer validation. The
`provider-minimal` fixture below demonstrates this ownership model.

## 3. Declare only what is implemented

`ProviderOperations` lists operations the provider actually implements;
`FileSystemCapabilities` describes stronger semantic guarantees. A provider
must not advertise an operation or capability merely because its backend has a
similar primitive. Start with the minimum declaration and add a contract test
for every addition.

## 4. Capability dependency strength

Capabilities have `Conditional` and `Guaranteed` support strengths. A
`Conditional` derived capability requires its base capability to be supported;
a `Guaranteed` derived capability requires the base capability to be
`Guaranteed`. The facade rejects inconsistent declarations before use.

## 5. A complete minimal provider

The following provider exposes one read-only health resource. This exact code
is compiled by the documentation fixture.

<!-- example: provider-minimal -->
```rust
use std::io::Cursor;

use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::read::ReadOptions;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

pub struct HealthProvider {
    properties: ProviderProperties,
}

impl HealthProvider {
    pub fn new() -> FsResult<Self> {
        let id = FileSystemId::new("docs-health")?;
        let properties = ProviderProperties::new(
            FileSystemInfo::new(id, "docs-health", PathSemantics::Hierarchical),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )?;
        Ok(Self { properties })
    }

    fn validate_path(path: &Path, operation: FsOperation) -> FsResult<()> {
        if path.as_str() == "/health" {
            Ok(())
        } else {
            Err(FsError::new(
                FsErrorKind::NotFound,
                operation,
                "health resource not found",
            )
            .with_path(path.clone()))
        }
    }
}

impl FileSystemSpi for HealthProvider {
    fn properties(&self) -> ProviderProperties {
        self.properties.clone()
    }

    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Self::validate_path(request.path(), FsOperation::Stat)?;
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(2)),
        ))
    }

    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Self::validate_path(request.path(), FsOperation::OpenReader)?;
        Ok(OpenedReader::new(
            OpenedFileInfo::new(self.properties.info().id().clone(), request.path().clone()),
            Box::new(Cursor::new(b"ok".to_vec())),
        ))
    }
}

pub fn read_health() -> FsResult<Vec<u8>> {
    let filesystem = FileSystem::from_spi(HealthProvider::new()?)?;
    filesystem.read_all(&Path::parse("/health")?, ReadOptions::default(), 16)
}
```

## 6. Errors and context

Return `FsError` with the operation and path that failed. The facade preserves
the operation's `FsEffectState` (for example, whether a publication may have
become visible), so callers can choose recovery without guessing. Include the
target or temporary path whenever the provider knows it.

## 7. Publication and cleanup

Writer and temporary-resource implementations must report publication,
cleanup, and recovery failures separately. A failed cleanup is not proof that
the target is absent. Follow the `CopyOutcome` and write recovery contracts;
retain the primary failure and any cleanup error for observability.

## 8. Async cancellation

An async operation must own its operation state outside the cancellation domain
and retain the operation until cleanup and recovery decisions are complete. Do
not let cancellation discard a writer, temporary path, or publication result.

## 9. Use `qubit-fs-testkit`

Adapters should run the shared contract suite from `qubit-fs-testkit` and add
backend-specific tests for limits, capabilities, path rules, and recovery.
Run the fixture and all-features tests before publishing.

## 10. Common contract violations

When tests fail, first check that operations and capabilities are declared only
when implemented, that capability dependency strengths are valid, that stat
metadata agrees with operation results, and that every error has path and
operation context. A `NotFound` result may make `exists` false; permission,
authentication, timeout, and I/O errors must remain errors.

## 11. Release checklist

- Construct and validate one cached `ProviderProperties` snapshot.
- Add only implemented operations and justified capability strengths.
- Run the shared testkit contract suite and provider-specific recovery tests.
- Run `cargo test --locked --all-features`, doctests, clippy, rustdoc, and the
  documentation checker.
- Review publication, cleanup, cancellation, and error-state behavior.

## 12. Further reading

- [User guide](user_guide.md) · [中文用户指南](user_guide.zh_CN.md)
- [File-system design](file_system_design.md) ·
  [中文设计文档](file_system_design.zh_CN.md)
- [API reference](https://docs.rs/qubit-fs)

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
