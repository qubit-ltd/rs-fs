use crate::path::Path;
use crate::read::ReadAllOptions;
/// Owned input for a managed complete-file read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FsOwnedReadRequest {
    /// Logical provider path to read.
    pub path: Path,
    /// Read bounds and consistency requirements.
    pub options: ReadAllOptions,
}

/// Owned input for a managed write operation.
pub struct FsOwnedWriteRequest {
    /// Destination path.
    pub path: Path,
    /// Complete byte buffer owned until provider release.
    pub contents: Vec<u8>,
    /// Write mode, preconditions, and durability requirements.
    pub options: crate::write::WriteOptions,
}

/// Owned input for a managed copy operation.
pub struct FsOwnedCopyRequest {
    /// Source path.
    pub source: Path,
    /// Destination path.
    pub destination: Path,
    /// Copy policy.
    pub options: crate::copy::CopyOptions,
}

/// Owned input for a managed rename operation.
pub struct FsOwnedRenameRequest {
    /// Source path.
    pub source: Path,
    /// Destination path.
    pub destination: Path,
    /// Rename policy.
    pub options: crate::rename::RenameOptions,
}

/// Owned input for publishing a staged directory tree.
pub struct FsOwnedTreePublishRequest {
    /// Staging root path owned by the provider.
    pub staging_root: Path,
    /// Destination root path.
    pub destination_root: Path,
    /// Tree validation limits.
    pub validation: crate::metadata::TreeValidationOptions,
    /// Version of the same staged owner observed after validation.
    ///
    /// Required atomic publication must compare this token as part of the
    /// publication transaction. It is not a standalone preflight guarantee.
    pub validated_version: Option<crate::metadata::ResourceVersion>,
    /// Publication guarantees.
    pub publication: crate::metadata::TreePublishOptions,
}
