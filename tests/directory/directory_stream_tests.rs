// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::time::Duration;

use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::directory::DirectoryStreamState;
use qubit_fs::directory::ListOptions;
use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::DirEntry;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::DirectoryStreamSpi;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::ListRequest;
use qubit_fs::spi::OpenedDirectoryStream;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

#[test]
fn test_directory_stream_enforces_entry_budget_before_returning_excess_entry() {
    let entries = vec![
        DirEntry::new(Path::parse("/root/first").expect("entry should parse"), FileKind::File),
        DirEntry::new(Path::parse("/root/second").expect("entry should parse"), FileKind::File),
    ];
    let (filesystem, _, _) = crate::handle_support::filesystem(false, entries);
    let mut stream = filesystem
        .list(
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default().with_max_entries(Some(1)),
        )
        .expect("stream should open");

    assert!(stream.next_entry().expect("first entry should fit").is_some());
    let error = stream.next_entry().expect_err("second entry must exceed the budget");
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(DirectoryStreamState::Failed, stream.state());
}

#[test]
fn test_directory_stream_enforces_depth_and_deadline_budgets() {
    let nested = DirEntry::new(
        Path::parse("/root/nested/item").expect("entry should parse"),
        FileKind::File,
    );
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![nested]);
    let mut stream = filesystem
        .list(
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default().with_recursive(true).with_max_depth(Some(1)),
        )
        .expect("stream should open");
    assert_eq!(
        FsErrorKind::ResourceLimitExceeded,
        stream
            .next_entry()
            .expect_err("nested entry must exceed depth one")
            .kind(),
    );

    let (filesystem, _, _) = crate::handle_support::filesystem(false, Vec::new());
    let mut stream = filesystem
        .list(
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default().with_deadline(Some(Duration::ZERO)),
        )
        .expect("stream should open");
    assert_eq!(
        FsErrorKind::ResourceLimitExceeded,
        stream
            .next_entry()
            .expect_err("zero deadline must expire before polling")
            .kind(),
    );
}
#[test]
fn test_directory_entry_path_can_be_compared_with_requested_root() {
    let entry = DirEntry::new(Path::parse("/outside").expect("entry should parse"), FileKind::File);
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default(),
        )
        .expect("stream should open");
    assert_eq!(DirectoryStreamState::Open, stream.state());
    let error = stream.next_entry().expect_err("outside entry must fail");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert_eq!(DirectoryStreamState::Failed, stream.state());
    assert!(format!("{error}").contains("outside requested root"));
}

/// Verifies object-key listing accepts a root prefix with a trailing slash.
#[test]
fn test_directory_stream_accepts_object_key_root_with_trailing_slash() {
    struct Entries(std::vec::IntoIter<DirEntry>);

    impl DirectoryStreamSpi for Entries {
        fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
            Ok(self.0.next())
        }
    }

    struct ObjectKeySpi;

    impl FileSystemSpi for ObjectKeySpi {
        fn properties(&self) -> ProviderProperties {
            ProviderProperties::new(
                FileSystemInfo::new(
                    FileSystemId::new("object-key-test").expect("test filesystem id should be valid"),
                    "object-key-test",
                    PathSemantics::ObjectKey,
                ),
                ProviderOperations::new()
                    .with(ProviderOperation::Stat)
                    .with(ProviderOperation::List),
                FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::List),
                FileSystemLimits::unknown(),
                PathConstraints::either(),
                SymlinkPolicy::Reject,
            )
            .expect("test properties should be valid")
        }

        fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
            Ok(StatResponse::new(
                request.path().clone(),
                FileMetadata::new(FileKind::File),
            ))
        }

        fn list(&self, _request: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
            let entry = DirEntry::new(
                Path::parse_literal("bucket/prefix/file").expect("object-key entry should parse"),
                FileKind::File,
            );
            Ok(OpenedDirectoryStream::new(Box::new(Entries(vec![entry].into_iter()))))
        }
    }

    let filesystem = FileSystem::from_spi(ObjectKeySpi).expect("object-key filesystem should construct");
    let root = Path::parse_literal("bucket/prefix/").expect("object-key root should parse");
    let mut stream = filesystem
        .list(&root, ListOptions::default())
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
    let entry = DirEntry::new(Path::parse("/root/other").expect("entry should parse"), FileKind::File);
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default().with_prefix(Some("nested".to_owned())),
        )
        .expect("stream should open");
    let error = stream
        .next_entry()
        .expect_err("provider must honor the requested prefix");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert!(format!("{error}").contains("outside requested prefix"));
}

/// Verifies a nested prefix is evaluated before direct-child filtering.
#[test]
fn test_directory_stream_accepts_nested_prefix_without_recursive_option() {
    let entry = DirEntry::new(
        Path::parse("/root/nested/item").expect("entry should parse"),
        FileKind::File,
    );
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default().with_prefix(Some("nested/item".to_owned())),
        )
        .expect("stream should open");

    let entry = stream
        .next_entry()
        .expect("nested prefix must be accepted")
        .expect("matching entry must be returned");
    assert_eq!(Path::parse("/root/nested/item").expect("path should parse"), entry.path);
}

/// Rejects a nested entry from a direct-child listing and keeps formatting
/// opaque to the provider session implementation.
#[test]
fn test_directory_stream_rejects_nested_entry_for_direct_listing() {
    let entry = DirEntry::new(
        Path::parse("/root/nested/item").expect("entry path should parse"),
        FileKind::File,
    );
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default(),
        )
        .expect("stream should open");
    let error = stream
        .next_entry()
        .expect_err("direct listing must reject nested provider entries");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert!(format!("{error}").contains("nested directory entry"));
    assert!(format!("{stream:?}").contains("DirectoryStream"));
}

/// Enforces requested entry metadata and accepts a descendant of an explicit
/// lexical prefix.
#[test]
fn test_directory_stream_validates_metadata_and_prefix_descendants() {
    let missing_metadata = DirEntry::new(
        Path::parse("/root/file").expect("entry path should parse"),
        FileKind::File,
    );
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![missing_metadata]);
    let mut stream = filesystem
        .list(
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default().with_include_metadata(true),
        )
        .expect("stream should open");
    let error = stream.next_entry().expect_err("metadata request must be enforced");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert!(format!("{error}").contains("without requested metadata"));

    let descendant = DirEntry::new(
        Path::parse("/root/nested/item").expect("entry path should parse"),
        FileKind::File,
    );
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![descendant]);
    let mut stream = filesystem
        .list(
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default().with_prefix(Some("nested".to_owned())),
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
    let entry = DirEntry::new(Path::parse("/file").expect("entry path should parse"), FileKind::File);
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(&Path::root(), ListOptions::default())
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
    let entry = DirEntry::new(
        Path::parse_with_semantics("/root/file", PathSemantics::ObjectKey).expect("entry path should parse"),
        FileKind::File,
    );
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default(),
        )
        .expect("stream should open");
    let error = stream.next_entry().expect_err("foreign path semantics must fail");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
}

/// Rejects provider entries whose name or metadata kind disagrees with path.
#[test]
fn test_directory_stream_rejects_inconsistent_entry_identity() {
    let mut wrong_name = DirEntry::new(
        Path::parse("/root/file").expect("entry path should parse"),
        FileKind::File,
    );
    wrong_name.name = "other".to_owned();
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![wrong_name]);
    let mut stream = filesystem
        .list(
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default(),
        )
        .expect("stream should open");
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        stream
            .next_entry()
            .expect_err("inconsistent entry name must fail")
            .kind()
    );

    let mut wrong_metadata = DirEntry::new(
        Path::parse("/root/file").expect("entry path should parse"),
        FileKind::File,
    );
    wrong_metadata.metadata = Some(FileMetadata::new(FileKind::Directory));
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![wrong_metadata]);
    let mut stream = filesystem
        .list(
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default(),
        )
        .expect("stream should open");
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
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
            &Path::parse("/root").expect("root should parse"),
            ListOptions::default(),
        )
        .expect("empty stream should open");
    assert!(empty.next_entry().expect("end of enumeration should succeed").is_none());
    assert_eq!(DirectoryStreamState::Exhausted, empty.state());
    let terminal = empty.next_entry().expect_err("completed stream must be terminal");
    assert_eq!(FsErrorKind::InvalidState, terminal.kind());

    let root = DirEntry::new(Path::root(), FileKind::Directory);
    let (filesystem, _, _) = crate::handle_support::filesystem(false, vec![root]);
    let mut root_stream = filesystem
        .list(&Path::root(), ListOptions::default())
        .expect("root stream should open");
    assert!(
        root_stream
            .next_entry()
            .expect("root entry should be accepted")
            .is_some()
    );
}
