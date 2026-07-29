// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Tests for immutable filesystem property snapshots.

use qubit_fs::{
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimit,
    FileSystemLimits,
    FileSystemProperties,
    Path,
    PathConstraints,
    PathForm,
    PathSemantics,
};

/// Builds the smallest valid properties snapshot for validation tests.
fn test_properties(path_semantics: PathSemantics) -> FileSystemProperties {
    let info = FileSystemInfo::new(
        FileSystemId::new("test-fs").expect("id should parse"),
        "test-provider",
        path_semantics,
    );
    FileSystemProperties::new(
        info,
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        PathConstraints::either(),
    )
    .expect("properties should validate")
}

/// Verifies nonsensical zero-valued provider limits are rejected at
/// construction.
#[test]
fn test_file_system_properties_rejects_invalid_limit_value() {
    let info = FileSystemInfo::new(
        FileSystemId::new("test-fs").expect("id should parse"),
        "test-provider",
        PathSemantics::Hierarchical,
    );
    let limits = FileSystemLimits::unknown()
        .with_max_path_text_bytes(FileSystemLimit::Maximum(0));
    assert!(
        FileSystemProperties::new(
            info,
            FileSystemCapabilities::new(),
            limits,
            PathConstraints::either(),
        )
        .is_err()
    );
}

/// Verifies snapshots expose their validated immutable values.
#[test]
fn test_file_system_properties_exposes_immutable_values() {
    let properties = test_properties(PathSemantics::Hierarchical);
    assert_eq!(properties.path_constraints().form(), PathForm::Either);
    assert_eq!(properties.info().provider_id(), "test-provider");
}

/// Verifies absolute-only constraints reject relative logical paths.
#[test]
fn test_path_constraints_validate_path_form() {
    let constraints = PathConstraints::absolute();
    let relative = Path::parse("child").expect("relative path should parse");
    assert!(constraints.validate(&relative).is_err());
}

/// Verifies a safe facade stream fallback is exposed as an effective copy
/// capability when the provider supplies both required byte primitives.
#[test]
fn test_file_system_properties_derives_copy_from_read_and_write() {
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("effective-copy").expect("id should parse"),
            "effective-provider",
            PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new()
            .with(FileSystemCapability::Read)
            .with(FileSystemCapability::Write),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
    )
    .expect("properties should validate");
    assert!(
        properties
            .capabilities()
            .contains(FileSystemCapability::Copy)
    );
}
