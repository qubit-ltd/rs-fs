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
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![entry]);
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
    assert!(format!("{error}").contains("outside requested root"));
}

/// Verifies object-key listing accepts a root prefix with a trailing slash.
#[test]
fn test_directory_stream_accepts_object_key_root_with_trailing_slash() {
    struct Entries(std::vec::IntoIter<qubit_fs::DirEntry>);

    impl qubit_fs::spi::DirectoryStreamSpi for Entries {
        fn next_entry(&mut self) -> qubit_fs::FsResult<Option<qubit_fs::DirEntry>> {
            Ok(self.0.next())
        }
    }

    struct ObjectKeySpi;

    impl qubit_fs::spi::FileSystemSpi for ObjectKeySpi {
        fn properties(&self) -> qubit_fs::FileSystemProperties {
            qubit_fs::FileSystemProperties::new(
                qubit_fs::FileSystemInfo::new(
                    qubit_fs::FileSystemId::new("object-key-test")
                        .expect("test filesystem id should be valid"),
                    "object-key-test",
                    qubit_fs::PathSemantics::ObjectKey,
                ),
                qubit_fs::FileSystemCapabilities::new()
                    .with_guaranteed(qubit_fs::FileSystemCapability::List),
                qubit_fs::FileSystemLimits::unknown(),
                qubit_fs::PathConstraints::either(),
                qubit_fs::SymlinkPolicy::Reject,
            )
            .expect("test properties should be valid")
        }

        fn stat(
            &self,
            request: qubit_fs::spi::StatRequest<'_>,
        ) -> qubit_fs::FsResult<qubit_fs::spi::StatResponse> {
            Ok(qubit_fs::spi::StatResponse::new(
                request.path().clone(),
                qubit_fs::FileMetadata::new(qubit_fs::FileKind::File),
            ))
        }

        fn list(
            &self,
            _request: qubit_fs::spi::ListRequest<'_>,
        ) -> qubit_fs::FsResult<qubit_fs::spi::OpenedDirectoryStream> {
            let entry = qubit_fs::DirEntry::new(
                qubit_fs::Path::parse_literal("bucket/prefix/file")
                    .expect("object-key entry should parse"),
                qubit_fs::FileKind::File,
            );
            Ok(qubit_fs::spi::OpenedDirectoryStream::new(Box::new(
                Entries(vec![entry].into_iter()),
            )))
        }
    }

    let filesystem = qubit_fs::FileSystem::from_spi(ObjectKeySpi)
        .expect("object-key filesystem should construct");
    let root =
        qubit_fs::Path::parse_literal("bucket/prefix/").expect("object-key root should parse");
    let mut stream = filesystem
        .list(&root, qubit_fs::ListOptions::default())
        .expect("object-key stream should open");

    assert!(
        stream
            .next_entry()
            .expect("object-key entry should be accepted")
            .is_some()
    );
}

/// Verifies providers cannot silently ignore the requested lexical prefix.
#[test]
fn test_directory_stream_rejects_entry_outside_requested_prefix() {
    let entry = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/root/other").expect("entry should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions::default().with_prefix(Some("nested".to_owned())),
        )
        .expect("stream should open");
    let error = stream
        .next_entry()
        .expect_err("provider must honor the requested prefix");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        error.kind()
    );
    assert!(format!("{error}").contains("outside requested prefix"));
}

/// Verifies a nested prefix is evaluated before direct-child filtering.
#[test]
fn test_directory_stream_accepts_nested_prefix_without_recursive_option() {
    let entry = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/root/nested/item").expect("entry should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions::default().with_prefix(Some("nested/item".to_owned())),
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
        qubit_fs::Path::parse("/root/nested/item").expect("entry path should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![entry]);
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
    assert!(format!("{error}").contains("nested directory entry"));
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
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![missing_metadata]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions::default().with_include_metadata(true),
        )
        .expect("stream should open");
    let error = stream
        .next_entry()
        .expect_err("metadata request must be enforced");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        error.kind()
    );
    assert!(format!("{error}").contains("without requested metadata"));

    let descendant = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/root/nested/item").expect("entry path should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![descendant]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions::default().with_prefix(Some("nested".to_owned())),
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
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![entry]);
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

/// Rejects provider entries whose path semantics disagree with the filesystem.
#[test]
fn test_directory_stream_rejects_foreign_path_semantics() {
    let entry = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse_with_semantics("/root/file", qubit_fs::PathSemantics::ObjectKey)
            .expect("entry path should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions::default(),
        )
        .expect("stream should open");
    let error = stream
        .next_entry()
        .expect_err("foreign path semantics must fail");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        error.kind()
    );
}

/// Rejects provider entries whose name or metadata kind disagrees with path.
#[test]
fn test_directory_stream_rejects_inconsistent_entry_identity() {
    let mut wrong_name = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/root/file").expect("entry path should parse"),
        qubit_fs::FileKind::File,
    );
    wrong_name.name = "other".to_owned();
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![wrong_name]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions::default(),
        )
        .expect("stream should open");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        stream
            .next_entry()
            .expect_err("inconsistent entry name must fail")
            .kind()
    );

    let mut wrong_metadata = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/root/file").expect("entry path should parse"),
        qubit_fs::FileKind::File,
    );
    wrong_metadata.metadata = Some(qubit_fs::FileMetadata::new(qubit_fs::FileKind::Directory));
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![wrong_metadata]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions::default(),
        )
        .expect("stream should open");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        stream
            .next_entry()
            .expect_err("inconsistent metadata kind must fail")
            .kind()
    );
}

/// Makes end-of-enumeration terminal and permits the root entry itself when a
/// provider explicitly reports it.
#[test]
fn test_directory_stream_handles_end_of_enumeration_and_root_entry() {
    let (filesystem, _, _) = crate::handle_support::filesystem(false, Vec::new());
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

    let root = qubit_fs::DirEntry::new(qubit_fs::Path::root(), qubit_fs::FileKind::Directory);
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![root]);
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
