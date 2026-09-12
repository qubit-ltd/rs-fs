#!/usr/bin/env python3
"""Insert type-level # Examples blocks for DOC-003 (one-time batch helper)."""
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "src"

SUPPORT_MOD = (
    '# mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), '
    '"/tests/common/rustdoc_support.rs")); }'
)

# Example bodies (rust code inside doctest, without /// prefixes).
EXAMPLES: dict[str, str] = {}


def _e(body: str) -> None:
    pass


def register(path: str, body: str) -> None:
    EXAMPLES[path] = body.strip() + "\n"


# --- copy ---
register(
    "copy/copy_method.rs",
    """use qubit_fs::copy::CopyMethod;

assert_eq!(CopyMethod::Streamed, CopyMethod::Streamed);""",
)
register(
    "copy/copy_mode.rs",
    """use qubit_fs::copy::CopyMode;

assert_eq!(CopyMode::Auto, CopyMode::default());""",
)
register(
    "copy/copy_conflict_policy.rs",
    """use qubit_fs::copy::CopyConflictPolicy;

assert_eq!(CopyConflictPolicy::Fail, CopyConflictPolicy::default());""",
)
register(
    "copy/copy_stats.rs",
    """use qubit_fs::copy::CopyStats;

let stats = CopyStats { files: 3, ..CopyStats::default() };
assert_eq!(3, stats.files);""",
)
register(
    "copy/copy_outcome.rs",
    """use qubit_fs::copy::{CopyMethod, CopyOutcome, CopyStats};
use qubit_fs::metadata::AchievedAtomicity;

let outcome = CopyOutcome::new(CopyStats::default(), CopyMethod::Streamed, AchievedAtomicity::Atomic);
assert_eq!(CopyMethod::Streamed, outcome.method());""",
)
register(
    "copy/metadata_preserve_policy.rs",
    """use qubit_fs::copy::MetadataPreservePolicy;

assert_eq!(MetadataPreservePolicy::Portable, MetadataPreservePolicy::default());""",
)
register(
    "copy/server_side_preference.rs",
    """use qubit_fs::copy::ServerSidePreference;

assert_eq!(ServerSidePreference::Disable, ServerSidePreference::default());""",
)
register(
    "copy/copy_failure_state.rs",
    """use qubit_fs::copy::CopyFailureState;

assert_eq!(CopyFailureState::Unchanged, CopyFailureState::Unchanged);""",
)
register(
    "copy/async_copy_operation_state.rs",
    """use qubit_fs::copy::AsyncCopyOperationState;

assert!(matches!(AsyncCopyOperationState::Ready, AsyncCopyOperationState::Ready));""",
)
register(
    "copy/async_copy_failure.rs",
    """use qubit_fs::copy::{AsyncCopyFailure, CopyFailureState, CopyStats};
use qubit_fs::error::{FsError, FsErrorKind, FsOperation};

let failure = AsyncCopyFailure::new(
    FsError::new(FsErrorKind::Io, FsOperation::Copy, "interrupted"),
    CopyFailureState::Unchanged,
    CopyStats::default(),
);
assert_eq!(FsOperation::Copy, failure.error().operation());""",
)

# --- directory ---
register(
    "directory/delete_options.rs",
    """use qubit_fs::directory::DeleteOptions;

assert!(!DeleteOptions::default().recursive());""",
)
register(
    "directory/list_filter.rs",
    """use qubit_fs::directory::ListFilter;

let filter = ListFilter::Subtree("reports/2026".to_owned());
assert!(matches!(filter, ListFilter::Subtree(_)));""",
)
register(
    "directory/create_directory_options.rs",
    """use qubit_fs::directory::CreateDirectoryOptions;

assert!(!CreateDirectoryOptions::default().recursive());""",
)
register(
    "directory/create_directory_outcome.rs",
    """use qubit_fs::directory::CreateDirectoryOutcome;

let outcome = CreateDirectoryOutcome::new(false);
assert!(!outcome.already_existed());""",
)
register(
    "directory/delete_outcome.rs",
    """use qubit_fs::directory::DeleteOutcome;

let outcome = DeleteOutcome::new(true);
assert!(outcome.already_missing());""",
)
register(
    "directory/directory_stream_state.rs",
    """use qubit_fs::directory::DirectoryStreamState;

assert!(matches!(DirectoryStreamState::Open, DirectoryStreamState::Open));""",
)

# --- metadata ---
register(
    "metadata/achieved_atomicity.rs",
    """use qubit_fs::metadata::AchievedAtomicity;

assert!(matches!(AchievedAtomicity::Atomic, AchievedAtomicity::Atomic));""",
)
register(
    "metadata/atomicity_requirement.rs",
    """use qubit_fs::metadata::AtomicityRequirement;

assert_eq!(AtomicityRequirement::Preferred, AtomicityRequirement::default());""",
)
register(
    "metadata/file_kind.rs",
    """use qubit_fs::metadata::FileKind;

assert!(matches!(FileKind::File, FileKind::File));""",
)
register(
    "metadata/file_system_id.rs",
    """use qubit_fs::metadata::FileSystemId;

let id = FileSystemId::new("local-instance")?;
assert_eq!("local-instance", id.as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "metadata/non_sensitive_metadata.rs",
    """use qubit_fs::metadata::{NonSensitiveMetadata, UserMetadata};

let safe = NonSensitiveMetadata::from(
    UserMetadata::new().with("content-language", "en")?,
);
assert_eq!(Some("en"), safe.get("content-language"));
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "metadata/file_metadata.rs",
    """use qubit_fs::metadata::{FileKind, FileMetadata};

let metadata = FileMetadata::new(FileKind::File).with_len(Some(42));
assert_eq!(Some(42), metadata.len());""",
)
register(
    "metadata/opened_file_info.rs",
    """use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
use qubit_fs::path::Path;

let info = OpenedFileInfo::new(FileSystemId::new("doc")?, Path::parse("/report")?)
    .with_metadata(FileMetadata::new(FileKind::File));
assert_eq!("/report", info.path().as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "metadata/dir_entry.rs",
    """use qubit_fs::metadata::{DirEntry, FileKind};
use qubit_fs::path::Path;

let entry = DirEntry::new(Path::parse("/a")?, FileKind::File);
assert_eq!("/a", entry.path.as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "metadata/checksum.rs",
    """use qubit_fs::metadata::{Checksum, ChecksumAlgorithm};

let checksum = Checksum::new(ChecksumAlgorithm::Sha256, "deadbeef");
assert_eq!(ChecksumAlgorithm::Sha256, checksum.algorithm);""",
)
register(
    "metadata/checksum_algorithm.rs",
    """use qubit_fs::metadata::ChecksumAlgorithm;

assert!(matches!(ChecksumAlgorithm::Sha256, ChecksumAlgorithm::Sha256));""",
)
register(
    "metadata/symlink_policy.rs",
    """use qubit_fs::metadata::SymlinkPolicy;

assert!(matches!(SymlinkPolicy::Reject, SymlinkPolicy::Reject));""",
)
register(
    "metadata/durability_requirement.rs",
    """use qubit_fs::metadata::DurabilityRequirement;

assert!(matches!(DurabilityRequirement::NotRequired, DurabilityRequirement::NotRequired));""",
)
register(
    "metadata/publication_method.rs",
    """use qubit_fs::metadata::PublicationMethod;

assert!(matches!(PublicationMethod::Direct, PublicationMethod::Direct));""",
)
register(
    "metadata/resource_version.rs",
    """use qubit_fs::metadata::ResourceVersion;

let version = ResourceVersion::new("v1");
assert_eq!("v1", version.as_str());""",
)
register(
    "metadata/write_outcome.rs",
    """use qubit_fs::metadata::{AchievedAtomicity, PublicationMethod, WriteOutcome};

let outcome = WriteOutcome::new(AchievedAtomicity::Atomic, PublicationMethod::Direct);
assert_eq!(AchievedAtomicity::Atomic, outcome.atomicity());""",
)
register(
    "metadata/file_system_capability.rs",
    """use qubit_fs::metadata::FileSystemCapability;

assert!(matches!(FileSystemCapability::Read, FileSystemCapability::Read));""",
)
register(
    "metadata/file_system_capability_support.rs",
    """use qubit_fs::metadata::FileSystemCapabilitySupport;

assert!(matches!(FileSystemCapabilitySupport::Unsupported, FileSystemCapabilitySupport::Unsupported));""",
)
register(
    "metadata/file_system_limit.rs",
    """use qubit_fs::metadata::FileSystemLimit;

assert!(matches!(FileSystemLimit::MaxPathLength, FileSystemLimit::MaxPathLength));""",
)
register(
    "metadata/file_system_limits.rs",
    """use qubit_fs::metadata::{FileSystemLimit, FileSystemLimits};

let limits = FileSystemLimits::unknown();
assert_eq!(FileSystemLimit::Unknown, limits.max_path_text_bytes());""",
)

# --- read / error ---
register(
    "read/checksum_policy.rs",
    """use qubit_fs::read::ChecksumPolicy;

assert_eq!(ChecksumPolicy::None, ChecksumPolicy::default());""",
)
register(
    "error/fs_operation.rs",
    """use qubit_fs::error::{FsError, FsErrorKind, FsOperation};

let error = FsError::new(FsErrorKind::NotFound, FsOperation::Stat, "missing");
assert_eq!(FsOperation::Stat, error.operation());""",
)
register(
    "error/open_failure_stage.rs",
    """use qubit_fs::error::OpenFailureStage;

assert!(matches!(OpenFailureStage::Preflight, OpenFailureStage::Preflight));""",
)
register(
    "error/recovery_cleanup_state.rs",
    """use qubit_fs::error::RecoveryCleanupState;

assert!(matches!(RecoveryCleanupState::NotAttempted, RecoveryCleanupState::NotAttempted));""",
)

# --- path ---
register(
    "path/path_component.rs",
    """use qubit_fs::path::PathComponent;

let component = PathComponent::parse("reports")?;
assert_eq!("reports", component.as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "path/path_semantics.rs",
    """use qubit_fs::path::PathSemantics;

assert_eq!(PathSemantics::Hierarchical, PathSemantics::default());""",
)
register(
    "path/relative_path.rs",
    """use qubit_fs::path::RelativePath;

let relative = RelativePath::parse("reports/2026")?;
assert_eq!("reports/2026", relative.as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "path/path_components.rs",
    """use qubit_fs::path::{Path, PathComponents};

let path = Path::parse("/a/b")?;
let components: PathComponents<'_> = path.components();
assert_eq!(2, components.count());""",
)
register(
    "path_constraints.rs",
    """use qubit_fs::path::{PathConstraints, PathForm};

let constraints = PathConstraints::absolute();
assert_eq!(PathForm::Absolute, constraints.form());""",
)
register(
    "path_form.rs",
    """use qubit_fs::path::PathForm;

assert!(matches!(PathForm::Absolute, PathForm::Absolute));""",
)

# --- rename ---
register(
    "rename/rename_options.rs",
    """use qubit_fs::rename::RenameOptions;

assert!(!RenameOptions::default().overwrite());""",
)
register(
    "rename/rename_outcome.rs",
    """use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::path::Path;
use qubit_fs::rename::RenameOutcome;

let outcome = RenameOutcome::new(Path::parse("/dest")?, AchievedAtomicity::Atomic);
assert_eq!("/dest", outcome.target().as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "rename/rename_failure_state.rs",
    """use qubit_fs::rename::RenameFailureState;

assert!(matches!(RenameFailureState::Unchanged, RenameFailureState::Unchanged));""",
)
register(
    "rename/rename_failure.rs",
    """use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
use qubit_fs::rename::{RenameFailure, RenameFailureState};

let failure = RenameFailure::new(
    FsError::new(FsErrorKind::NotFound, FsOperation::Rename, "missing"),
    RenameFailureState::Unchanged,
);
assert_eq!(RenameFailureState::Unchanged, failure.state());""",
)

# --- uri ---
register(
    "uri/uri.rs",
    """use qubit_fs::path::Uri;

let uri = Uri::parse("file:///tmp/report.txt")?;
assert_eq!("file", uri.scheme());
# Ok::<(), qubit_fs::FsError>(())""",
)

# --- temp ---
register(
    "temp/temp_options.rs",
    """use qubit_fs::temp::TempOptions;

assert_eq!(TempOptions::default(), TempOptions::new());""",
)
register(
    "temp/persist_options.rs",
    """use qubit_fs::temp::PersistOptions;

assert!(!PersistOptions::default().overwrite());""",
)
register(
    "temp/persist_outcome.rs",
    """use qubit_fs::metadata::{AchievedAtomicity, PublicationMethod};
use qubit_fs::path::Path;
use qubit_fs::temp::PersistOutcome;

let outcome = PersistOutcome::new(
    Path::parse("/published")?,
    AchievedAtomicity::Atomic,
    PublicationMethod::Direct,
);
assert_eq!("/published", outcome.target().as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "temp/persist_failure_state.rs",
    """use qubit_fs::temp::PersistFailureState;

assert!(matches!(PersistFailureState::NotPublished, PersistFailureState::NotPublished));""",
)
register(
    "temp/persist_cleanup_state.rs",
    """use qubit_fs::temp::PersistCleanupState;

assert!(matches!(PersistCleanupState::Removed, PersistCleanupState::Removed));""",
)
register(
    "temp/persist_failure.rs",
    """use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
use qubit_fs::temp::{PersistFailure, PersistFailureState};

let failure = PersistFailure::new(
    FsError::new(FsErrorKind::Io, FsOperation::PersistTemp, "failed"),
    PersistFailureState::NotPublished,
);
assert_eq!(PersistFailureState::NotPublished, failure.state());""",
)
register(
    "temp/temp_resource_state.rs",
    """use qubit_fs::temp::TempResourceState;

assert!(matches!(TempResourceState::Open, TempResourceState::Open));""",
)

# --- write ---
register(
    "write/write_failure.rs",
    """use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
use qubit_fs::write::{WriteFailure, WriteFailureState};

let failure = WriteFailure::new(
    FsError::new(FsErrorKind::Io, FsOperation::Write, "failed"),
    WriteFailureState::NotPublished,
);
assert_eq!(WriteFailureState::NotPublished, failure.state());""",
)
register(
    "write/write_failure_state.rs",
    """use qubit_fs::write::WriteFailureState;

assert!(matches!(WriteFailureState::NotPublished, WriteFailureState::NotPublished));""",
)
register(
    "write/write_disposition.rs",
    """use qubit_fs::write::WriteDisposition;

assert_eq!(WriteDisposition::CreateOrTruncate, WriteDisposition::default());""",
)
register(
    "write/write_precondition.rs",
    """use qubit_fs::write::WritePrecondition;

assert!(matches!(WritePrecondition::None, WritePrecondition::None));""",
)
register(
    "write/write_abort_outcome.rs",
    """use qubit_fs::write::WriteAbortOutcome;

assert!(matches!(WriteAbortOutcome::NotPublished, WriteAbortOutcome::NotPublished));""",
)
register(
    "write/writer_state.rs",
    """use qubit_fs::write::WriterState;

assert!(matches!(WriterState::Open, WriterState::Open));""",
)
register(
    "write/writer_recovery.rs",
    """use qubit_fs::write::WriterRecovery;

assert!(matches!(WriterRecovery::None, WriterRecovery::None));""",
)
register(
    "write/write_all_failure.rs",
    """use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
use qubit_fs::write::{WriteAllFailure, WriteFailureState};

let failure = WriteAllFailure::new(
    FsError::new(FsErrorKind::Io, FsOperation::Write, "failed"),
    WriteFailureState::NotPublished,
);
assert_eq!(WriteFailureState::NotPublished, failure.state());""",
)

# --- spi (sync envelopes via support) ---
_SPI_OPENED = f"""{SUPPORT_MOD}
use qubit_fs::spi::{{{{type}}}};

let _opened: {{{{type}}}} = support::rustdoc_provider::{{{{helper}}}}();"""

register(
    "spi/opened_reader.rs",
    f"""{SUPPORT_MOD}
use qubit_fs::spi::OpenedReader;

let _opened: OpenedReader = support::rustdoc_provider::opened_reader();""",
)
register(
    "spi/opened_writer.rs",
    f"""{SUPPORT_MOD}
use qubit_fs::spi::OpenedWriter;

let _opened: OpenedWriter = support::rustdoc_provider::opened_writer();""",
)
register(
    "spi/opened_temp_file.rs",
    f"""{SUPPORT_MOD}
use qubit_fs::spi::OpenedTempFile;

let _opened: OpenedTempFile = support::rustdoc_provider::opened_temp_file();""",
)
register(
    "spi/stat_response.rs",
    """use qubit_fs::metadata::{FileKind, FileMetadata};
use qubit_fs::path::Path;
use qubit_fs::spi::StatResponse;

let response = StatResponse::new(
    Path::parse("/object")?,
    FileMetadata::new(FileKind::File),
);
assert_eq!("/object", response.path().as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "spi/copy_decline_reason.rs",
    """use qubit_fs::spi::CopyDeclineReason;

assert!(matches!(CopyDeclineReason::NotImplemented, CopyDeclineReason::NotImplemented));""",
)
register(
    "spi/copy_attempt.rs",
    """use qubit_fs::copy::{CopyMethod, CopyOutcome, CopyStats};
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::spi::{CopyAttempt, CopyDeclineReason};

let attempt = CopyAttempt::Completed(CopyOutcome::new(
    CopyStats::default(),
    CopyMethod::Native,
    AchievedAtomicity::Atomic,
));
assert!(matches!(attempt, CopyAttempt::Completed(_)));
let declined = CopyAttempt::Declined(CopyDeclineReason::NotApplicable);
assert!(matches!(declined, CopyAttempt::Declined(_)));""",
)
register(
    "spi/provider_operation.rs",
    """use qubit_fs::spi::ProviderOperation;

assert!(matches!(ProviderOperation::Stat, ProviderOperation::Stat));""",
)
register(
    "spi/provider_operations.rs",
    """use qubit_fs::spi::{ProviderOperation, ProviderOperations};

let ops = ProviderOperations::new().with(ProviderOperation::Stat);
assert!(ops.supports(ProviderOperation::Stat));""",
)
register(
    "spi/spi_write_failure.rs",
    """use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
use qubit_fs::spi::SpiWriteFailure;
use qubit_fs::write::WriteFailureState;

let failure = SpiWriteFailure::new(
    FsError::new(FsErrorKind::Io, FsOperation::Write, "failed"),
    WriteFailureState::NotPublished,
);
assert_eq!(WriteFailureState::NotPublished, failure.state());""",
)
register(
    "spi/spi_persist_failure.rs",
    """use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
use qubit_fs::spi::SpiPersistFailure;
use qubit_fs::temp::PersistFailureState;

let failure = SpiPersistFailure::new(
    FsError::new(FsErrorKind::Io, FsOperation::PersistTemp, "failed"),
    PersistFailureState::NotPublished,
);
assert_eq!(PersistFailureState::NotPublished, failure.state());""",
)
register(
    "spi/spi_rename_failure.rs",
    """use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
use qubit_fs::rename::RenameFailureState;
use qubit_fs::spi::SpiRenameFailure;

let failure = SpiRenameFailure::new(
    FsError::new(FsErrorKind::NotFound, FsOperation::Rename, "missing"),
    RenameFailureState::Unchanged,
);
assert_eq!(RenameFailureState::Unchanged, failure.state());""",
)
register(
    "spi/spi_copy_failure.rs",
    """use qubit_fs::copy::CopyFailureState;
use qubit_fs::copy::CopyStats;
use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
use qubit_fs::spi::SpiCopyFailure;

let failure = SpiCopyFailure::new(
    FsError::new(FsErrorKind::Io, FsOperation::Copy, "failed"),
    CopyFailureState::Unchanged,
    CopyStats::default(),
);
assert_eq!(CopyFailureState::Unchanged, failure.state());""",
)
register(
    "spi/opened_directory_stream.rs",
    """use qubit_fs::error::FsResult;
use qubit_fs::metadata::DirEntry;
use qubit_fs::spi::{DirectoryStreamSpi, OpenedDirectoryStream};

struct EmptyStream;
impl DirectoryStreamSpi for EmptyStream {
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
        Ok(None)
    }
}
let _stream = OpenedDirectoryStream::new(Box::new(EmptyStream));""",
)
register(
    "spi/resolved_copy_options.rs",
    """use qubit_fs::copy::CopyOptions;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::spi::ResolvedCopyOptions;

let resolved = ResolvedCopyOptions::new(CopyOptions::default(), SymlinkPolicy::Reject);
assert_eq!(SymlinkPolicy::Reject, resolved.symlink_policy());""",
)
register(
    "spi/resolved_list_options.rs",
    """use qubit_fs::directory::ListOptions;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::spi::ResolvedListOptions;

let resolved = ResolvedListOptions::new(ListOptions::default(), SymlinkPolicy::Reject);
assert_eq!(SymlinkPolicy::Reject, resolved.symlink_policy());""",
)

# spi requests (pub(crate) constructors visible in-file)
register(
    "spi/request/copy_request.rs",
    """use qubit_fs::copy::CopyOptions;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::Path;
use qubit_fs::spi::{CopyRequest, ResolvedCopyOptions};

let source = Path::parse("/source")?;
let target = Path::parse("/target")?;
let options = ResolvedCopyOptions::new(CopyOptions::default(), SymlinkPolicy::Reject);
let request = CopyRequest::new(&source, &target, options);
assert_eq!("/source", request.source().as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "spi/request/list_request.rs",
    """use qubit_fs::directory::ListOptions;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::Path;
use qubit_fs::spi::{ListRequest, ResolvedListOptions};

let path = Path::parse("/dir")?;
let options = ResolvedListOptions::new(ListOptions::default(), SymlinkPolicy::Reject);
let request = ListRequest::new(&path, options);
assert_eq!("/dir", request.path().as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "spi/request/persist_request.rs",
    """use qubit_fs::path::Path;
use qubit_fs::spi::PersistRequest;
use qubit_fs::temp::PersistOptions;

let source = Path::parse("/tmp/scratch")?;
let target = Path::parse("/final")?;
let request = PersistRequest::new(&source, &target, PersistOptions::default());
assert_eq!("/final", request.target().as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "spi/request/create_temp_file_request.rs",
    """use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::temp::TempOptions;

let request = CreateTempFileRequest::new(TempOptions::default());
assert_eq!(TempOptions::default(), request.options());""",
)
register(
    "spi/request/create_temp_directory_request.rs",
    """use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::temp::TempOptions;

let request = CreateTempDirectoryRequest::new(TempOptions::default());
assert_eq!(TempOptions::default(), request.options());""",
)
register(
    "spi/request/rename_request.rs",
    """use qubit_fs::path::Path;
use qubit_fs::rename::RenameOptions;
use qubit_fs::spi::RenameRequest;

let source = Path::parse("/old")?;
let target = Path::parse("/new")?;
let request = RenameRequest::new(&source, &target, RenameOptions::default());
assert_eq!("/old", request.source().as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)

register(
    "spi/opened_async_reader.rs",
    """use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
use qubit_fs::path::Path;
use qubit_fs::spi::OpenedAsyncReader;
use std::pin::Pin;
use std::task::{Context, Poll};

struct EmptyReader;
impl qubit_io::AsyncInput for EmptyReader {
    type Item = u8;
    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(None)
    }
}
let info = OpenedFileInfo::new(FileSystemId::new("doc")?, Path::parse("/x")?)
    .with_metadata(FileMetadata::new(FileKind::File));
let reader = OpenedAsyncReader::new(info, Box::new(EmptyReader));
assert_eq!("/x", reader.info().path().as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)

register(
    "error/open_failure.rs",
    """use qubit_fs::error::{FsError, FsErrorKind, FsOperation, OpenFailure, OpenFailureStage};

let failure = OpenFailure::new(
    FsError::new(FsErrorKind::NotFound, FsOperation::OpenReader, "missing"),
    OpenFailureStage::Preflight,
    None::<()>,
);
assert_eq!(OpenFailureStage::Preflight, failure.stage());""",
)

register(
    "write/write_all_failure.rs",
    """use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
use qubit_fs::write::{WriteAllFailure, WriteFailureState};

let failure = WriteAllFailure::new(
    FsError::new(FsErrorKind::Io, FsOperation::Write, "failed"),
    WriteFailureState::NotPublished,
);
assert_eq!(WriteFailureState::NotPublished, failure.state());""",
)

register(
    "spi/opened_temp_directory.rs",
    """use qubit_fs::error::FsResult;
use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
use qubit_fs::path::Path;
use qubit_fs::spi::{OpenedTempDirectory, TempResourceSpi};

struct EmptyTemp;
impl TempResourceSpi for EmptyTemp {
    fn persist(
        &mut self,
        _: qubit_fs::spi::PersistRequest<'_>,
    ) -> Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure> {
        unreachable!()
    }
    fn keep(
        &mut self,
    ) -> Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure> {
        unreachable!()
    }
    fn cleanup(&mut self) -> FsResult<()> {
        Ok(())
    }
}
let info = OpenedFileInfo::new(FileSystemId::new("doc")?, Path::parse("/tmp")?)
    .with_metadata(FileMetadata::new(FileKind::Directory));
let _opened = OpenedTempDirectory::new(info, Box::new(EmptyTemp));
# Ok::<(), qubit_fs::FsError>(())""",
)

register(
    "temp/temp_directory.rs",
    f"""{SUPPORT_MOD}
# use support::rustdoc_provider;
# let filesystem = rustdoc_provider::filesystem();
use qubit_fs::temp::{{TempOptions, TempResourceState}};

let mut temporary = filesystem.create_temp_directory(TempOptions::default())?;
assert_eq!(TempResourceState::Owned, temporary.state());
# Ok::<(), Box<dyn std::error::Error>>(())""",
)

register(
    "spi/opened_async_writer.rs",
    """use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
use qubit_fs::path::Path;
use qubit_fs::spi::{AsyncFileWriteSession, OpenedAsyncWriter, SpiFuture};
use qubit_fs::write::{WriteAbortOutcome, WriteFailure};
use qubit_io::AsyncOutput;
use std::io::Result as IoResult;
use std::pin::Pin;
use std::task::{Context, Poll};

struct Session;
impl AsyncOutput for Session {
    type Item = u8;
    unsafe fn poll_write_unchecked(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &[u8],
        _: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        Poll::Ready(Ok(count))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
        Poll::Ready(Ok(()))
    }
}
impl AsyncFileWriteSession for Session {
    fn commit_async<'a>(
        self: Pin<&'a mut Self>,
    ) -> SpiFuture<'a, Result<qubit_fs::metadata::WriteOutcome, WriteFailure>> {
        Box::pin(async { unreachable!() })
    }
    fn abort_async<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, qubit_fs::error::FsResult<WriteAbortOutcome>> {
        Box::pin(async { Ok(WriteAbortOutcome::NotPublished) })
    }
}
let info = OpenedFileInfo::new(FileSystemId::new("doc")?, Path::parse("/draft")?)
    .with_metadata(FileMetadata::new(FileKind::File));
let writer = OpenedAsyncWriter::new(info, Box::new(Session));
assert_eq!("/draft", writer.info().path().as_str());
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "spi/opened_async_temp_file.rs",
    """use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
use qubit_fs::path::Path;
use qubit_fs::spi::{AsyncTempResourceSpi, OpenedAsyncTempFile, PersistRequest, SpiFuture};
use std::pin::Pin;

struct Session;
impl AsyncTempResourceSpi for Session {
    fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, qubit_fs::error::FsResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn keep<'a>(
        self: Pin<&'a mut Self>,
    ) -> SpiFuture<'a, Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure>> {
        Box::pin(async { unreachable!() })
    }
    fn persist<'a>(
        self: Pin<&'a mut Self>,
        _: PersistRequest<'a>,
    ) -> SpiFuture<'a, Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure>> {
        Box::pin(async { unreachable!() })
    }
}
let info = OpenedFileInfo::new(FileSystemId::new("doc")?, Path::parse("/scratch")?)
    .with_metadata(FileMetadata::new(FileKind::File));
let _opened = OpenedAsyncTempFile::new(info, Box::new(Session));
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "spi/opened_async_temp_directory.rs",
    """use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
use qubit_fs::path::Path;
use qubit_fs::spi::{AsyncTempResourceSpi, OpenedAsyncTempDirectory, PersistRequest, SpiFuture};
use std::pin::Pin;

struct Session;
impl AsyncTempResourceSpi for Session {
    fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, qubit_fs::error::FsResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn keep<'a>(
        self: Pin<&'a mut Self>,
    ) -> SpiFuture<'a, Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure>> {
        Box::pin(async { unreachable!() })
    }
    fn persist<'a>(
        self: Pin<&'a mut Self>,
        _: PersistRequest<'a>,
    ) -> SpiFuture<'a, Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure>> {
        Box::pin(async { unreachable!() })
    }
}
let info = OpenedFileInfo::new(FileSystemId::new("doc")?, Path::parse("/scratch-dir")?)
    .with_metadata(FileMetadata::new(FileKind::Directory));
let _opened = OpenedAsyncTempDirectory::new(info, Box::new(Session));
# Ok::<(), qubit_fs::FsError>(())""",
)
register(
    "spi/opened_async_directory_stream.rs",
    """use qubit_fs::error::FsResult;
use qubit_fs::metadata::DirEntry;
use qubit_fs::spi::{AsyncDirectoryStreamSession, OpenedAsyncDirectoryStream, SpiFuture};

struct EmptyStream;
impl AsyncDirectoryStreamSession for EmptyStream {
    fn next_entry_async(&mut self) -> SpiFuture<'_, FsResult<Option<DirEntry>>> {
        Box::pin(async { Ok(None) })
    }
}
let _stream = OpenedAsyncDirectoryStream::new(Box::new(EmptyStream));""",
)
register(
    "temp/async_temp_directory.rs",
    f"""{SUPPORT_MOD}
# use support::*;
# let (filesystem, _) = async_recording_spi::async_recording_file_system(Default::default());
# poll_support::ready(async {{
use qubit_fs::temp::{{TempOptions, TempResourceState}};

let mut temporary = filesystem.create_temp_directory(TempOptions::default()).await?;
temporary.cleanup().await?;
assert_eq!(TempResourceState::Cleaned, temporary.state());
# Ok::<(), Box<dyn std::error::Error>>(())
# }}).unwrap();""",
)

register(
    "write/async_write_all_operation_state.rs",
    """use qubit_fs::write::AsyncWriteAllOperationState;

assert!(matches!(AsyncWriteAllOperationState::Ready, AsyncWriteAllOperationState::Ready));""",
)
register(
    "write/async_writer_recovery.rs",
    """use qubit_fs::write::AsyncWriterRecovery;

assert!(matches!(AsyncWriterRecovery::None, AsyncWriterRecovery::None));""",
)
register(
    "write/rejected_writer.rs",
    """use qubit_fs::error::{FsResult, RecoveryCleanupState};
use qubit_fs::spi::FileWriterSpi;
use qubit_fs::write::{RejectedWriter, WriteAbortOutcome};

struct NoopWriter;
impl FileWriterSpi for NoopWriter {
    fn commit(
        &mut self,
    ) -> Result<qubit_fs::metadata::WriteOutcome, qubit_fs::spi::SpiWriteFailure> {
        unreachable!()
    }
    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        Ok(WriteAbortOutcome::NotPublished)
    }
}
let rejected = RejectedWriter::new(Box::new(NoopWriter), "rustdoc", None);
assert_eq!(RecoveryCleanupState::Pending, rejected.cleanup_state());""",
)
register(
    "write/rejected_async_writer.rs",
    """use qubit_fs::error::RecoveryCleanupState;
use qubit_fs::spi::AsyncFileWriteSession;
use qubit_fs::write::RejectedAsyncWriter;

struct NoopSession;
impl AsyncFileWriteSession for NoopSession {
    fn poll_abort(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<qubit_fs::error::FsResult<qubit_fs::write::WriteAbortOutcome>> {
        std::task::Poll::Ready(Ok(qubit_fs::write::WriteAbortOutcome::NotPublished))
    }
}
let rejected = RejectedAsyncWriter::new(Box::new(NoopSession), "rustdoc", None);
assert_eq!(RecoveryCleanupState::Pending, rejected.cleanup_state());""",
)
register(
    "temp/rejected_temp_resource.rs",
    """use qubit_fs::error::{FsResult, RecoveryCleanupState};
use qubit_fs::spi::TempResourceSpi;
use qubit_fs::temp::RejectedTempResource;

struct NoopTemp;
impl TempResourceSpi for NoopTemp {
    fn persist(
        &mut self,
        _: qubit_fs::spi::PersistRequest<'_>,
    ) -> Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure> {
        unreachable!()
    }
    fn keep(
        &mut self,
    ) -> Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure> {
        unreachable!()
    }
    fn cleanup(&mut self) -> FsResult<()> {
        Ok(())
    }
}
let rejected = RejectedTempResource::new(Box::new(NoopTemp), "rustdoc", None);
assert_eq!(RecoveryCleanupState::Pending, rejected.cleanup_state());""",
)
register(
    "temp/rejected_async_temp_resource.rs",
    """use qubit_fs::error::RecoveryCleanupState;
use qubit_fs::spi::{AsyncTempResourceSpi, PersistRequest, SpiFuture};
use qubit_fs::temp::RejectedAsyncTempResource;
use std::pin::Pin;

struct NoopSession;
impl AsyncTempResourceSpi for NoopSession {
    fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, qubit_fs::error::FsResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn keep<'a>(
        self: Pin<&'a mut Self>,
    ) -> SpiFuture<'a, Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure>> {
        Box::pin(async { unreachable!() })
    }
    fn persist<'a>(
        self: Pin<&'a mut Self>,
        _: PersistRequest<'a>,
    ) -> SpiFuture<'a, Result<qubit_fs::temp::PersistOutcome, qubit_fs::spi::SpiPersistFailure>> {
        Box::pin(async { unreachable!() })
    }
}
let rejected = RejectedAsyncTempResource::new(Box::new(NoopSession), "rustdoc", None);
assert_eq!(RecoveryCleanupState::Pending, rejected.cleanup_state());""",
)


def format_block(body: str) -> str:
    lines = ["///", "/// # Examples", "///", "/// ```rust"]
    for line in body.rstrip().split("\n"):
        lines.append(f"/// {line}" if line else "///")
    lines.append("/// ```")
    return "\n".join(lines)


def insert_examples(content: str, block: str, rel: str = "") -> str:
    if "# Examples" in content:
        return content
    if rel.endswith("directory/list_filter.rs"):
        content = content.replace(
            "//! Explicit directory and object-key listing filters.\n/// Listing",
            "//! Explicit directory and object-key listing filters.\n\n/// Listing",
        )
    match = re.search(r"^pub (struct|enum) ", content, re.M)
    if not match:
        return content
    pub_line_start = match.start()
    prefix = content[:pub_line_start]
    lines = prefix.splitlines(keepends=True)
    idx = len(lines) - 1
    while idx >= 0 and lines[idx].strip().startswith("#["):
        idx -= 1
    last_doc = idx
    while idx >= 0 and lines[idx].startswith("///"):
        idx -= 1
    insert_at = last_doc + 1
    new_prefix = "".join(lines[:insert_at]) + block + "\n" + "".join(lines[insert_at:])
    return new_prefix + content[pub_line_start:]


def main() -> None:
    applied = 0
    for rel, body in sorted(EXAMPLES.items()):
        path = ROOT / rel
        if not path.exists():
            print("skip missing", rel)
            continue
        text = path.read_text(encoding="utf-8")
        if "# Examples" in text:
            continue
        block = format_block(body)
        new_text = insert_examples(text, block, rel)
        if new_text != text:
            path.write_text(new_text, encoding="utf-8")
            applied += 1
    print(f"applied {applied} files, registered {len(EXAMPLES)} examples")


if __name__ == "__main__":
    main()
