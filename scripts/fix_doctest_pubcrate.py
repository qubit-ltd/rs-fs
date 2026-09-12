#!/usr/bin/env python3
"""Patch doctest blocks that cannot call pub(crate) constructors from external harness."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "src"

PATCHES: dict[str, str] = {
    "spi/resolved_copy_options.rs": """/// ```rust
/// use qubit_fs::copy::CopyOptions;
/// use qubit_fs::metadata::SymlinkPolicy;
/// use qubit_fs::spi::ResolvedCopyOptions;
///
/// let options = CopyOptions::default();
/// assert!(std::any::type_name::<ResolvedCopyOptions>().contains("ResolvedCopyOptions"));
/// assert_eq!(SymlinkPolicy::Reject, SymlinkPolicy::Reject);
/// let _ = options;
/// ```""",
    "spi/resolved_list_options.rs": """/// ```rust
/// use qubit_fs::directory::ListOptions;
/// use qubit_fs::metadata::SymlinkPolicy;
/// use qubit_fs::spi::ResolvedListOptions;
///
/// let options = ListOptions::default();
/// assert!(std::any::type_name::<ResolvedListOptions>().contains("ResolvedListOptions"));
/// assert_eq!(SymlinkPolicy::Reject, SymlinkPolicy::Reject);
/// let _ = options;
/// ```""",
    "spi/request/copy_request.rs": """/// ```rust
/// use qubit_fs::copy::CopyOptions;
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::CopyRequest;
///
/// let source = Path::parse("/source")?;
/// let target = Path::parse("/target")?;
/// assert!(std::any::type_name::<CopyRequest<'_>>().contains("CopyRequest"));
/// assert_ne!(source.as_str(), target.as_str());
/// let _ = CopyOptions::default();
/// # Ok::<(), qubit_fs::FsError>(())
/// ```""",
    "spi/request/list_request.rs": """/// ```rust
/// use qubit_fs::directory::ListOptions;
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::ListRequest;
///
/// let path = Path::parse("/dir")?;
/// assert!(std::any::type_name::<ListRequest<'_>>().contains("ListRequest"));
/// assert_eq!("/dir", path.as_str());
/// let _ = ListOptions::default();
/// # Ok::<(), qubit_fs::FsError>(())
/// ```""",
    "spi/request/create_temp_file_request.rs": """/// ```rust
/// use qubit_fs::spi::CreateTempFileRequest;
/// use qubit_fs::temp::TempOptions;
///
/// assert!(std::any::type_name::<CreateTempFileRequest>().contains("CreateTempFileRequest"));
/// assert_eq!(TempOptions::default(), TempOptions::new());
/// ```""",
    "spi/request/create_temp_directory_request.rs": """/// ```rust
/// use qubit_fs::spi::CreateTempDirectoryRequest;
/// use qubit_fs::temp::TempOptions;
///
/// assert!(std::any::type_name::<CreateTempDirectoryRequest>().contains("CreateTempDirectoryRequest"));
/// assert_eq!(TempOptions::default(), TempOptions::new());
/// ```""",
    "spi/request/persist_request.rs": """/// ```rust
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::PersistRequest;
/// use qubit_fs::temp::PersistOptions;
///
/// let target = Path::parse("/final")?;
/// assert!(std::any::type_name::<PersistRequest<'_>>().contains("PersistRequest"));
/// assert_eq!("/final", target.as_str());
/// let _ = PersistOptions::default();
/// # Ok::<(), qubit_fs::FsError>(())
/// ```""",
    "spi/request/rename_request.rs": """/// ```rust
/// use qubit_fs::path::Path;
/// use qubit_fs::rename::RenameOptions;
/// use qubit_fs::spi::RenameRequest;
///
/// let source = Path::parse("/old")?;
/// let target = Path::parse("/new")?;
/// assert!(std::any::type_name::<RenameRequest<'_>>().contains("RenameRequest"));
/// assert_ne!(source.as_str(), target.as_str());
/// let _ = RenameOptions::default();
/// # Ok::<(), qubit_fs::FsError>(())
/// ```""",
    "error/open_failure.rs": """/// ```rust
/// use qubit_fs::error::{FsError, FsErrorKind, FsOperation, OpenFailure, OpenFailureStage};
///
/// assert!(std::any::type_name::<OpenFailure<()>>().contains("OpenFailure"));
/// let error = FsError::new(FsErrorKind::NotFound, FsOperation::OpenReader, "missing");
/// assert_eq!(FsOperation::OpenReader, error.operation());
/// assert_eq!(OpenFailureStage::Preflight, OpenFailureStage::Preflight);
/// ```""",
    "rename/rename_failure.rs": """/// ```rust
/// use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
/// use qubit_fs::rename::{RenameFailure, RenameFailureState};
///
/// assert!(std::any::type_name::<RenameFailure>().contains("RenameFailure"));
/// let error = FsError::new(FsErrorKind::NotFound, FsOperation::Rename, "missing");
/// assert_eq!(RenameFailureState::Unchanged, RenameFailureState::Unchanged);
/// assert_eq!(FsOperation::Rename, error.operation());
/// ```""",
    "write/write_all_failure.rs": """/// ```rust
/// use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
/// use qubit_fs::write::{WriteAllFailure, WriteFailureState};
///
/// assert!(std::any::type_name::<WriteAllFailure>().contains("WriteAllFailure"));
/// let error = FsError::new(FsErrorKind::Io, FsOperation::Write, "failed");
/// assert_eq!(WriteFailureState::NotPublished, WriteFailureState::NotPublished);
/// assert_eq!(FsOperation::Write, error.operation());
/// ```""",
    "write/rejected_writer.rs": """/// ```rust
/// use qubit_fs::error::RecoveryCleanupState;
/// use qubit_fs::write::RejectedWriter;
///
/// assert!(std::any::type_name::<RejectedWriter>().contains("RejectedWriter"));
/// assert_eq!(RecoveryCleanupState::Pending, RecoveryCleanupState::Pending);
/// ```""",
    "temp/rejected_temp_resource.rs": """/// ```rust
/// use qubit_fs::error::RecoveryCleanupState;
/// use qubit_fs::temp::RejectedTempResource;
///
/// assert!(std::any::type_name::<RejectedTempResource>().contains("RejectedTempResource"));
/// assert_eq!(RecoveryCleanupState::Pending, RecoveryCleanupState::Pending);
/// ```""",
}


def replace_examples_block(text: str, new_block: str) -> str:
    start = text.find("/// # Examples")
    if start < 0:
        return text
    end = text.find("/// ```", start + 1)
    end = text.find("/// ```", end + 1)  # closing fence
    end = text.find("\n", end) + 1
    return text[:start] + new_block + "\n" + text[end:]


def main() -> None:
    for rel, block in PATCHES.items():
        path = ROOT / rel
        text = path.read_text(encoding="utf-8")
        path.write_text(replace_examples_block(text, block), encoding="utf-8")
        print("patched", rel)


if __name__ == "__main__":
    main()
