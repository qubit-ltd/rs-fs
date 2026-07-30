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

/// Verifies relative-only and either-form constraints accept exactly their
/// documented path forms without performing I/O.
#[test]
fn test_path_constraints_accept_matching_forms() {
    let absolute = Path::parse("/child").expect("absolute path should parse");
    let relative = Path::parse("child").expect("relative path should parse");

    assert!(PathConstraints::relative().validate(&relative).is_ok());
    assert!(PathConstraints::relative().validate(&absolute).is_err());
    assert!(PathConstraints::either().validate(&relative).is_ok());
    assert!(PathConstraints::either().validate(&absolute).is_ok());
}

/// Verifies stream fallback does not overstate a provider copy capability.
#[test]
fn test_file_system_properties_does_not_derive_copy_from_read_and_write() {
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
        !properties
            .capabilities()
            .contains(FileSystemCapability::Copy)
    );
}

/// Verifies snapshots expose their stored limits and capabilities unchanged
/// after construction when no derived capability is applicable.
#[test]
fn test_file_system_properties_exposes_limits_and_capabilities() {
    let limits = FileSystemLimits::unknown()
        .with_max_write_bytes(FileSystemLimit::Maximum(128));
    let capabilities =
        FileSystemCapabilities::new().with(FileSystemCapability::Read);
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("stored-properties").expect("id should parse"),
            "stored-provider",
            PathSemantics::Hierarchical,
        ),
        capabilities,
        limits,
        PathConstraints::relative(),
    )
    .expect("properties should validate");

    assert_eq!(capabilities, properties.capabilities());
    assert_eq!(&limits, properties.limits());
    assert_eq!(PathForm::Relative, properties.path_constraints().form());
}

/// Verifies internally inconsistent capability and literal-path configuration
/// snapshots are rejected at the facade boundary.
#[test]
fn test_file_system_properties_rejects_invalid_capabilities_and_constraints() {
    let info = FileSystemInfo::new(
        FileSystemId::new("invalid-properties").expect("id should parse"),
        "provider",
        PathSemantics::Hierarchical,
    );
    assert!(
        FileSystemProperties::new(
            info,
            FileSystemCapabilities::new()
                .with(FileSystemCapability::AtomicRename),
            FileSystemLimits::unknown(),
            PathConstraints::either(),
        )
        .is_err()
    );

    let literal_info = FileSystemInfo::new(
        FileSystemId::new("literal-properties").expect("id should parse"),
        "provider",
        PathSemantics::ObjectKey,
    );
    assert!(
        FileSystemProperties::new(
            literal_info,
            FileSystemCapabilities::new(),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
        )
        .is_err()
    );
}

/// Verifies every declared filesystem limit can be configured and read back.
#[test]
fn test_file_system_limits_configure_all_dimensions() {
    let limits = FileSystemLimits::unknown()
        .with_max_path_text_bytes(FileSystemLimit::Maximum(20))
        .with_max_component_text_bytes(FileSystemLimit::Maximum(8))
        .with_max_read_range_bytes(FileSystemLimit::Maximum(32))
        .with_max_write_bytes(FileSystemLimit::Maximum(64))
        .with_max_list_page_entries(FileSystemLimit::Maximum(10));

    assert_eq!(FileSystemLimit::Maximum(20), limits.max_path_text_bytes());
    assert_eq!(
        FileSystemLimit::Maximum(8),
        limits.max_component_text_bytes()
    );
    assert_eq!(FileSystemLimit::Maximum(32), limits.max_read_range_bytes());
    assert_eq!(FileSystemLimit::Maximum(64), limits.max_write_bytes());
    assert_eq!(FileSystemLimit::Maximum(10), limits.max_list_page_entries());
}

/// Verifies provider list-page limits clamp finite requests while preserving
/// missing and non-finite hints.
#[test]
fn test_file_system_limits_clamp_list_page_size() {
    let limited = FileSystemLimits::unknown()
        .with_max_list_page_entries(FileSystemLimit::Maximum(10));
    assert_eq!(None, limited.clamp_list_page_size(None));
    assert_eq!(Some(4), limited.clamp_list_page_size(Some(4)));
    assert_eq!(Some(10), limited.clamp_list_page_size(Some(20)));

    for limit in [
        FileSystemLimit::Unknown,
        FileSystemLimit::NotApplicable,
        FileSystemLimit::Unbounded,
    ] {
        let limits =
            FileSystemLimits::unknown().with_max_list_page_entries(limit);
        assert_eq!(Some(20), limits.clamp_list_page_size(Some(20)));
    }
}

/// Verifies path, read, and write preflight checks reject only requests above
/// their corresponding finite provider limits.
#[test]
fn test_file_system_limits_validate_path_read_and_write_boundaries() {
    let limits = FileSystemLimits::unknown()
        .with_max_path_text_bytes(FileSystemLimit::Maximum(10))
        .with_max_component_text_bytes(FileSystemLimit::Maximum(4))
        .with_max_read_range_bytes(FileSystemLimit::Maximum(5))
        .with_max_write_bytes(FileSystemLimit::Maximum(6));
    let short_path = Path::parse("a/bbbb").expect("path should parse");
    let long_component = Path::parse("a/ccccc").expect("path should parse");
    let long_path = Path::parse("abcdefghijk").expect("path should parse");

    assert!(
        limits
            .validate_path(
                &short_path,
                PathSemantics::Hierarchical,
                qubit_fs::FsOperation::Stat,
            )
            .is_ok()
    );
    assert!(
        limits
            .validate_path(
                &long_component,
                PathSemantics::Hierarchical,
                qubit_fs::FsOperation::Stat,
            )
            .is_err()
    );
    assert!(
        limits
            .validate_path(
                &long_component,
                PathSemantics::ObjectKey,
                qubit_fs::FsOperation::Stat,
            )
            .is_ok()
    );
    assert!(
        limits
            .validate_path(
                &long_path,
                PathSemantics::ObjectKey,
                qubit_fs::FsOperation::Stat,
            )
            .is_err()
    );

    assert!(limits.validate_read_range(&short_path, None).is_ok());
    assert!(limits.validate_read_range(&short_path, Some(5)).is_ok());
    assert!(limits.validate_read_range(&short_path, Some(6)).is_err());
    assert!(limits.validate_write_size(&short_path, 6).is_ok());
    assert!(limits.validate_write_size(&short_path, 7).is_err());
}
