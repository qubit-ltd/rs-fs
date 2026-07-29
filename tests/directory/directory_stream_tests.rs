// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#[test]
fn test_directory_entry_path_can_be_compared_with_requested_root() {
    let entry = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/outside").expect("entry should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions::default(),
        )
        .expect("stream should open");
    let error = stream.next_entry().expect_err("outside entry must fail");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        error.kind()
    );
}

/// Verifies providers cannot silently ignore the requested lexical prefix.
#[test]
fn test_directory_stream_rejects_entry_outside_requested_prefix() {
    let entry = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/root/other").expect("entry should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions {
                prefix: Some("nested".to_owned()),
                ..qubit_fs::ListOptions::default()
            },
        )
        .expect("stream should open");
    let error = stream
        .next_entry()
        .expect_err("provider must honor the requested prefix");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        error.kind()
    );
}

/// Verifies a nested prefix is evaluated before direct-child filtering.
#[test]
fn test_directory_stream_accepts_nested_prefix_without_recursive_option() {
    let entry = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/root/nested/item").expect("entry should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions {
                prefix: Some("nested/item".to_owned()),
                ..qubit_fs::ListOptions::default()
            },
        )
        .expect("stream should open");

    let entry = stream
        .next_entry()
        .expect("nested prefix must be accepted")
        .expect("matching entry must be returned");
    assert_eq!(
        qubit_fs::Path::parse("/root/nested/item").expect("path should parse"),
        entry.path
    );
}

/// Rejects a nested entry from a direct-child listing and keeps formatting
/// opaque to the provider session implementation.
#[test]
fn test_directory_stream_rejects_nested_entry_for_direct_listing() {
    let entry = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/root/nested/item")
            .expect("entry path should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions::default(),
        )
        .expect("stream should open");
    let error = stream
        .next_entry()
        .expect_err("direct listing must reject nested provider entries");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        error.kind()
    );
    assert!(format!("{stream:?}").contains("DirectoryStream"));
}

/// Enforces requested entry metadata and accepts a descendant of an explicit
/// lexical prefix.
#[test]
fn test_directory_stream_validates_metadata_and_prefix_descendants() {
    let missing_metadata = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/root/file").expect("entry path should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, vec![missing_metadata]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions {
                include_metadata: true,
                ..qubit_fs::ListOptions::default()
            },
        )
        .expect("stream should open");
    let error = stream
        .next_entry()
        .expect_err("metadata request must be enforced");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        error.kind()
    );

    let descendant = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/root/nested/item")
            .expect("entry path should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, vec![descendant]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions {
                prefix: Some("nested".to_owned()),
                ..qubit_fs::ListOptions::default()
            },
        )
        .expect("stream should open");
    assert!(
        stream
            .next_entry()
            .expect("prefix descendant should be accepted")
            .is_some()
    );
}

/// Resolves a direct child under the filesystem root.
#[test]
fn test_directory_stream_accepts_root_relative_entry() {
    let entry = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/file").expect("entry path should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(&qubit_fs::Path::root(), qubit_fs::ListOptions::default())
        .expect("root stream should open");
    assert!(
        stream
            .next_entry()
            .expect("root-relative entry should be accepted")
            .is_some()
    );
}

/// Makes end-of-enumeration terminal and permits the root entry itself when a
/// provider explicitly reports it.
#[test]
fn test_directory_stream_handles_end_of_enumeration_and_root_entry() {
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, Vec::new());
    let mut empty = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions::default(),
        )
        .expect("empty stream should open");
    assert!(
        empty
            .next_entry()
            .expect("end of enumeration should succeed")
            .is_none()
    );
    let terminal = empty
        .next_entry()
        .expect_err("completed stream must be terminal");
    assert_eq!(qubit_fs::FsErrorKind::InvalidState, terminal.kind());

    let root = qubit_fs::DirEntry::new(
        qubit_fs::Path::root(),
        qubit_fs::FileKind::Directory,
    );
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, vec![root]);
    let mut root_stream = filesystem
        .list(&qubit_fs::Path::root(), qubit_fs::ListOptions::default())
        .expect("root stream should open");
    assert!(
        root_stream
            .next_entry()
            .expect("root entry should be accepted")
            .is_some()
    );
}
