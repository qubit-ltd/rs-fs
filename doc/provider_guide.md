# Provider implementation guide

[中文版本](provider_guide.zh_CN.md)

This guide targets `qubit-fs` 0.4 provider authors. It explains the smallest
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

use qubit_fs::error::{FsErrorKind, FsOperation};
use qubit_fs::metadata::{
    FileKind, FileMetadata, FileSystemCapabilities, FileSystemCapability, FileSystemId,
    FileSystemInfo, FileSystemLimits, OpenedFileInfo, SymlinkPolicy,
};
use qubit_fs::path::{PathConstraints, PathSemantics};
use qubit_fs::read::ReadOptions;
use qubit_fs::spi::{
    FileSystemSpi, OpenReaderRequest, OpenedReader, ProviderOperation, ProviderOperations,
    ProviderProperties, StatRequest, StatResponse,
};
use qubit_fs::{FileSystem, FsError, FsResult, Path};

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
- [0.4 migration guide](migration_0_4.md)
- [API reference](https://docs.rs/qubit-fs)
