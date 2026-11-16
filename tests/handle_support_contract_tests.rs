// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::Path;
use qubit_fs::copy::CopyOptions;
use qubit_fs::directory::CreateDirectoryOptions;
use qubit_fs::directory::DeleteOptions;
use qubit_fs::directory::ListOptions;
use qubit_fs::directory::ListScope;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemCapabilitySupport;
use qubit_fs::read::ReadOptions;
use qubit_fs::temp::TempOptions;
use qubit_fs::write::WriteOptions;
use qubit_io::Input;
use qubit_io::Output;

mod handle_support;

use crate::handle_support::implementation_tests::filesystem;
use crate::handle_support::implementation_tests::invalid_temp_cleanup_filesystem;
use crate::handle_support::implementation_tests::invalid_temp_path_filesystem;
use crate::handle_support::implementation_tests::provider_open_failure_filesystem;
use crate::handle_support::implementation_tests::wrong_temp_kind_filesystem;

#[test]
fn test_handle_support_constructs_file_system() {
    let (file_system, _, _) = filesystem(false, Vec::new());

    assert_eq!("handles-test", file_system.properties().info().id().as_str());
}

/// Exercises synchronous facade dispatch for directory creation and both
/// deletion primitives against the recording provider.
#[test]
fn test_handle_support_dispatches_directory_and_delete_operations() {
    let (file_system, _, _) = filesystem(false, Vec::new());
    let path = Path::parse("/target").expect("test path should parse");

    assert!(
        !file_system
            .create_directory(&path, CreateDirectoryOptions::default())
            .expect("directory creation should succeed")
            .already_existed()
    );
    assert!(
        !file_system
            .delete_file(&path, DeleteOptions::default())
            .expect("file deletion should succeed")
            .already_missing()
    );
    assert!(
        !file_system
            .delete_directory(&path, DeleteOptions::default())
            .expect("directory deletion should succeed")
            .already_missing()
    );
}

/// Exercises each synchronous facade entry point on the successful provider
/// path so that its public validation and handle-binding contracts remain
/// covered together.
#[test]
fn test_handle_support_dispatches_successful_facade_operations() {
    let (file_system, _, _) = filesystem(false, Vec::new());
    let source = Path::parse("/source").expect("test path should parse");
    let target = Path::parse("/target").expect("test path should parse");

    assert!(file_system.exists(&source).expect("stat should succeed"));
    let mut directory = file_system
        .list(&ListScope::Path((source).clone()), ListOptions::default())
        .expect("list should succeed");
    assert!(directory.next_entry().expect("stream should succeed").is_none());

    let mut reader = file_system
        .open_reader(&source, ReadOptions::default())
        .expect("reader should open");
    let mut bytes = [0_u8; 5];
    assert_eq!(
        5,
        Input::read(&mut reader, &mut bytes).expect("reader should transfer bytes")
    );

    let mut writer = file_system
        .open_writer(&target, WriteOptions::default())
        .expect("writer should open");
    Output::write_fully(&mut writer, b"bytes").expect("writer should accept bytes");
    writer.commit().expect("writer should commit");

    let mut temporary_file = file_system
        .create_temp_file(TempOptions::default())
        .expect("temporary file should open");
    temporary_file.keep().expect("temporary file should be kept");
    let mut temporary_directory = file_system
        .create_temp_directory(TempOptions::default())
        .expect("temporary directory should open");
    temporary_directory.keep().expect("temporary directory should be kept");
}

/// Invokes the synchronous SPI's optional default copy method and completes
/// the facade-owned stream fallback.
#[test]
fn test_handle_support_uses_default_spi_copy_decline() {
    let (file_system, _, _) = filesystem(false, Vec::new());
    assert_eq!(
        FileSystemCapabilitySupport::Guaranteed,
        file_system
            .properties()
            .capabilities()
            .support(FileSystemCapability::Copy),
        "the fixture must reach the default TryCopy implementation",
    );
    let outcome = file_system
        .copy(
            &Path::parse("/source").expect("test path should parse"),
            &Path::parse("/target").expect("test path should parse"),
            CopyOptions::default(),
        )
        .expect("fallback should use reader and writer capabilities");
    assert!(outcome.used_fallback());
}

/// Ensures provider open failures are enriched by each synchronous facade
/// entry point after its local validation has completed.
#[test]
fn test_handle_support_enriches_open_and_temp_provider_failures() {
    let file_system = provider_open_failure_filesystem();
    let path = Path::parse("/target").expect("test path should parse");
    let list = file_system
        .list(&ListScope::Path(path.clone()), ListOptions::default())
        .expect_err("list failure");
    let writer = file_system
        .open_writer(&path, WriteOptions::default())
        .expect_err("writer failure");
    let reader = file_system
        .open_reader(&path, ReadOptions::default())
        .expect_err("reader failure");
    let temp_file = file_system
        .create_temp_file(TempOptions::default())
        .expect_err("temp failure");
    let temp_directory = file_system
        .create_temp_directory(TempOptions::default())
        .expect_err("temp failure");
    for error in [
        &list,
        writer.error(),
        &reader,
        temp_file.error(),
        temp_directory.error(),
    ] {
        assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
        assert_eq!(Some("handles-test"), error.provider());
    }
    assert!(writer.recovery().is_none());
    assert!(temp_file.recovery().is_none());
    assert!(temp_directory.recovery().is_none());
}

/// Verifies pathless temporary provider failures do not receive a fabricated
/// hierarchical root path.
#[test]
fn test_handle_support_keeps_pathless_temp_errors_pathless() {
    let file_system = provider_open_failure_filesystem();
    let file_error = file_system
        .create_temp_file(TempOptions::default())
        .expect_err("temporary-file provider failure should propagate");
    let directory_error = file_system
        .create_temp_directory(TempOptions::default())
        .expect_err("temporary-directory provider failure should propagate");
    for error in [file_error, directory_error] {
        assert_eq!(FsOperation::CreateTemp, error.error().operation());
        assert_eq!(None, error.error().path());
        assert_eq!(Some("handles-test"), error.error().provider());
    }
}

/// Verifies temporary creation rejects an invalid parent before provider I/O.
#[test]
fn test_handle_support_validates_temp_parent_before_provider_call() {
    let file_system = provider_open_failure_filesystem();
    let parent = Path::parse("relative").expect("relative test path should parse");
    let file_error = file_system
        .create_temp_file(TempOptions::default().with_parent(Some(parent.clone())))
        .expect_err("invalid temporary-file parent must fail in the facade");
    assert_eq!(FsErrorKind::InvalidPath, file_error.error().kind());
    assert_eq!(FsOperation::CreateTemp, file_error.error().operation());
    assert_eq!(Some(&parent), file_error.error().path());

    let directory_error = file_system
        .create_temp_directory(TempOptions::default().with_parent(Some(parent.clone())))
        .expect_err("invalid temporary-directory parent must fail in the facade");
    assert_eq!(FsErrorKind::InvalidPath, directory_error.error().kind());
    assert_eq!(FsOperation::CreateTemp, directory_error.error().operation());
    assert_eq!(Some(&parent), directory_error.error().path());
}

/// Rejects invalid temporary identities even when the provider also fails to
/// clean up the invalid resource.
#[test]
fn test_handle_support_rejects_invalid_temp_identities_with_cleanup_failure() {
    let file_system = invalid_temp_cleanup_filesystem();
    for mut error in [
        file_system
            .create_temp_file(TempOptions::default())
            .expect_err("foreign temporary file identity must be rejected"),
        file_system
            .create_temp_directory(TempOptions::default())
            .expect_err("foreign temporary directory identity must be rejected"),
    ] {
        assert_eq!(FsErrorKind::ProviderContractViolation, error.error().kind());
        assert_eq!(FsOperation::ValidateProviderOutcome, error.error().operation());
        let cleanup = error
            .recovery_mut()
            .expect("isolated session")
            .cleanup()
            .expect_err("cleanup injection");
        assert!(cleanup.to_string().contains("injected temporary cleanup failure"));
        assert_eq!(FsErrorKind::ProviderContractViolation, error.error().kind());
    }
}

/// Rejects invalid temporary paths while retaining explicit cleanup for both
/// temporary resource kinds.
#[test]
fn test_handle_support_rejects_invalid_temp_paths_after_cleanup() {
    for error in [
        invalid_temp_path_filesystem()
            .create_temp_file(TempOptions::default())
            .expect_err("relative temporary file path must be rejected"),
        invalid_temp_path_filesystem()
            .create_temp_directory(TempOptions::default())
            .expect_err("relative temporary directory path must be rejected"),
    ] {
        assert_eq!(FsErrorKind::ProviderContractViolation, error.error().kind());
    }
}

/// Rejects a temporary-file envelope whose metadata claims a directory.
#[test]
fn test_handle_support_rejects_wrong_temp_kind() {
    let error = wrong_temp_kind_filesystem()
        .create_temp_file(TempOptions::default())
        .expect_err("temporary-file kind must be validated");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.error().kind());
}
