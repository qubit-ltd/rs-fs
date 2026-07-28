//! Tests for immutable filesystem property snapshots.

use qubit_fs::{
    FileSystemCapabilities, FileSystemId, FileSystemInfo, FileSystemLimit, FileSystemLimits,
    FileSystemProperties, Path, PathConstraints, PathForm, PathSemantics,
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

/// Verifies nonsensical zero-valued provider limits are rejected at construction.
#[test]
fn test_file_system_properties_rejects_invalid_limit_value() {
    let info = FileSystemInfo::new(
        FileSystemId::new("test-fs").expect("id should parse"),
        "test-provider",
        PathSemantics::Hierarchical,
    );
    let limits = FileSystemLimits::unknown().with_max_path_text_bytes(FileSystemLimit::Maximum(0));
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
