// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! External contract tests for provider property snapshots.

use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemCapabilitySupport;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

/// Provider exposing only an immutable property snapshot.
struct PropertiesOnlySpi {
    properties: ProviderProperties,
}

impl PropertiesOnlySpi {
    /// Creates a provider returning `properties` from its SPI snapshot method.
    fn new(properties: ProviderProperties) -> Self {
        Self { properties }
    }
}

impl FileSystemSpi for PropertiesOnlySpi {
    /// Returns the provider property snapshot used by this test.
    fn properties(&self) -> ProviderProperties {
        self.properties.clone()
    }

    /// Rejects metadata requests because this test performs no provider I/O.
    fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Stat,
            "test provider performs no I/O",
        ))
    }
}

/// Builds a validated provider snapshot for a hierarchical test filesystem.
fn provider_properties(
    operations: ProviderOperations,
    declared_capabilities: FileSystemCapabilities,
) -> ProviderProperties {
    ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("provider-properties-test").expect("test filesystem id should be valid"),
            "provider-properties-test",
            PathSemantics::Hierarchical,
        ),
        operations,
        declared_capabilities,
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("provider properties must be valid")
}

/// Attempts to build a provider snapshot without converting validation errors
/// into test panics.
fn try_provider_properties(
    operations: ProviderOperations,
    declared_capabilities: FileSystemCapabilities,
) -> FsResult<ProviderProperties> {
    ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("invalid-provider-properties-test").expect("test filesystem id should be valid"),
            "provider-properties-test",
            PathSemantics::Hierarchical,
        ),
        operations,
        declared_capabilities,
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
}

/// Returns every provider entry point except `missing`.
fn operations_without(missing: ProviderOperation) -> ProviderOperations {
    let mut operations = ProviderOperations::new();
    for operation in [
        ProviderOperation::Stat,
        ProviderOperation::List,
        ProviderOperation::OpenReader,
        ProviderOperation::OpenWriter,
        ProviderOperation::CreateDirectory,
        ProviderOperation::DeleteFile,
        ProviderOperation::DeleteDirectory,
        ProviderOperation::TryCopy,
        ProviderOperation::Rename,
        ProviderOperation::CreateTempFile,
        ProviderOperation::CreateTempDirectory,
    ] {
        if operation != missing {
            operations = operations.with(operation);
        }
    }
    operations
}

/// Adds the capability dependencies required by the public capability value
/// object before declaring `capability` itself.
fn declared_capabilities_for(capability: FileSystemCapability) -> FileSystemCapabilities {
    let capabilities = FileSystemCapabilities::new();
    let capabilities = match capability {
        FileSystemCapability::RangeRead
        | FileSystemCapability::ConditionalRead
        | FileSystemCapability::ChecksumValidation => capabilities.with_guaranteed(FileSystemCapability::Read),
        FileSystemCapability::Append | FileSystemCapability::ConditionalWrite | FileSystemCapability::AtomicReplace => {
            capabilities.with_guaranteed(FileSystemCapability::Write)
        }
        FileSystemCapability::RecursiveDelete | FileSystemCapability::ConditionalDelete => {
            capabilities.with_guaranteed(FileSystemCapability::Delete)
        }
        FileSystemCapability::AtomicRename | FileSystemCapability::DurableRename => {
            capabilities.with_guaranteed(FileSystemCapability::Rename)
        }
        FileSystemCapability::ServerSideCopy
        | FileSystemCapability::AtomicFileCopy
        | FileSystemCapability::AtomicTreeCopy
        | FileSystemCapability::DurableFileCopy
        | FileSystemCapability::DurableTreeCopy => capabilities.with_guaranteed(FileSystemCapability::Copy),
        _ => capabilities,
    };
    capabilities.with_guaranteed(capability)
}

/// Verifies the facade derives conditional copy without advertising native
/// provider copy dispatch.
#[test]
fn test_facade_derives_conditional_copy_without_native_try_copy() {
    let operations = ProviderOperations::new()
        .with(ProviderOperation::Stat)
        .with(ProviderOperation::OpenReader)
        .with(ProviderOperation::OpenWriter);
    let declared = FileSystemCapabilities::new()
        .with_guaranteed(FileSystemCapability::Read)
        .with_guaranteed(FileSystemCapability::Write);
    let provider = provider_properties(operations, declared);
    let filesystem =
        FileSystem::from_spi(PropertiesOnlySpi::new(provider.clone())).expect("provider properties must be valid");

    assert!(!provider.operations().supports(ProviderOperation::TryCopy));
    assert_eq!(
        filesystem
            .properties()
            .capabilities()
            .support(FileSystemCapability::Copy),
        FileSystemCapabilitySupport::Conditional,
    );
}

/// Verifies every declared capability group rejects a snapshot missing its
/// mapped provider entry point, including both multi-operation contracts.
#[test]
fn test_provider_properties_rejects_missing_capability_operations() {
    let cases = [
        (FileSystemCapability::List, ProviderOperation::List),
        (FileSystemCapability::Read, ProviderOperation::OpenReader),
        (FileSystemCapability::RangeRead, ProviderOperation::OpenReader),
        (FileSystemCapability::ConditionalRead, ProviderOperation::OpenReader),
        (FileSystemCapability::ChecksumValidation, ProviderOperation::OpenReader),
        (FileSystemCapability::Write, ProviderOperation::OpenWriter),
        (FileSystemCapability::Append, ProviderOperation::OpenWriter),
        (FileSystemCapability::ConditionalWrite, ProviderOperation::OpenWriter),
        (FileSystemCapability::AtomicReplace, ProviderOperation::OpenWriter),
        (
            FileSystemCapability::CreateDirectory,
            ProviderOperation::CreateDirectory,
        ),
        (FileSystemCapability::EmptyDirectory, ProviderOperation::CreateDirectory),
        (FileSystemCapability::Delete, ProviderOperation::DeleteFile),
        (FileSystemCapability::Delete, ProviderOperation::DeleteDirectory),
        (
            FileSystemCapability::RecursiveDelete,
            ProviderOperation::DeleteDirectory,
        ),
        (FileSystemCapability::ConditionalDelete, ProviderOperation::DeleteFile),
        (
            FileSystemCapability::ConditionalDelete,
            ProviderOperation::DeleteDirectory,
        ),
        (FileSystemCapability::Rename, ProviderOperation::Rename),
        (FileSystemCapability::AtomicRename, ProviderOperation::Rename),
        (FileSystemCapability::DurableRename, ProviderOperation::Rename),
        (FileSystemCapability::TempFile, ProviderOperation::CreateTempFile),
        (
            FileSystemCapability::TempDirectory,
            ProviderOperation::CreateTempDirectory,
        ),
        (
            FileSystemCapability::AtomicTempPersist,
            ProviderOperation::CreateTempFile,
        ),
        (
            FileSystemCapability::AtomicTempPersist,
            ProviderOperation::CreateTempDirectory,
        ),
        (FileSystemCapability::Copy, ProviderOperation::TryCopy),
        (FileSystemCapability::ServerSideCopy, ProviderOperation::TryCopy),
        (FileSystemCapability::AtomicFileCopy, ProviderOperation::TryCopy),
        (FileSystemCapability::AtomicTreeCopy, ProviderOperation::TryCopy),
        (FileSystemCapability::DurableFileCopy, ProviderOperation::TryCopy),
        (FileSystemCapability::DurableTreeCopy, ProviderOperation::TryCopy),
    ];

    for (capability, missing_operation) in cases {
        let error = try_provider_properties(
            operations_without(missing_operation),
            declared_capabilities_for(capability),
        )
        .expect_err("missing provider operation must reject the snapshot");
        assert_eq!(FsErrorKind::InvalidOptions, error.kind());
        assert_eq!(FsOperation::ValidateProperties, error.operation());
        assert!(error.required_capability().is_some());
        assert!(
            format!("{error}").contains(&format!("{missing_operation:?}")),
            "error must identify missing operation {missing_operation:?} for {capability:?}",
        );
    }
}

/// Verifies the first invalid declared capability and its missing operation are
/// retained as validation context.
#[test]
fn test_provider_properties_reports_first_missing_capability_operation() {
    let declared = FileSystemCapabilities::new()
        .with_guaranteed(FileSystemCapability::List)
        .with_guaranteed(FileSystemCapability::Read);
    let error = try_provider_properties(ProviderOperations::new(), declared)
        .expect_err("the first missing mapped operation must be reported");

    assert_eq!(FsOperation::ValidateProperties, error.operation());
    assert_eq!(Some(FileSystemCapability::List), error.required_capability(),);
    assert!(format!("{error}").contains("List"));
}

/// Verifies symbolic-link policy guarantees do not require a dedicated
/// provider operation entry point.
#[test]
fn test_provider_properties_accepts_symlink_without_operation() {
    let provider = try_provider_properties(
        ProviderOperations::new(),
        FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Symlink),
    )
    .expect("symlink policy guarantee must not require an entry point");

    assert_eq!(
        FileSystemCapabilitySupport::Guaranteed,
        provider.declared_capabilities().support(FileSystemCapability::Symlink),
    );
}
