// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! # Qubit FS
//!
//! Provider-neutral synchronous and asynchronous filesystem abstraction.
//!
//! This crate defines filesystem properties and operation traits, explicit
//! file handles over [`qubit_io`] streams, distinct URI and provider-local path
//! models, typed capabilities and outcomes, recoverable writer and temporary
//! resource lifecycles. Runtime provider discovery and SPI integration live in
//! the companion `qubit-fs-registry` crate. This crate contains no concrete
//! storage backend and binds to no asynchronous runtime.
//!
//! ## Addressing a resource
//!
//! ```no_run
//! use qubit_fs::Path;
//!
//! let path = Path::parse("/reports/2026/summary.csv")?;
//! assert!(path.is_absolute());
//! # Ok::<(), qubit_fs::FsError>(())
//! ```
//!
//! Application code uses the concrete facades and their handles. Provider
//! contracts are available only under [`spi`]. Legacy resource wrappers and
//! provider-local path values are not public API.
//!
//! ```compile_fail
//! use qubit_fs::FileResource;
//! ```
//!
//! ```compile_fail
//! use qubit_fs::FsPath;
//! ```
//!
//! ```compile_fail
//! use qubit_fs::FileSystemSpi;
//! ```

#![deny(missing_docs)]

#[cfg(feature = "async")]
mod async_file_system;
mod copy;
mod error;
mod facade_context;
mod file_system;
mod file_system_properties;
mod handle;
mod internal;
mod metadata;
mod options;
mod path;
mod path_constraints;
mod path_form;
mod rename;
pub mod spi;
mod temp;
mod uri;

#[cfg(feature = "async")]
pub use async_file_system::AsyncFileSystem;
#[cfg(feature = "async")]
pub use copy::{
    AsyncCopyFailure,
    AsyncCopyOperation,
    AsyncCopyOperationState,
};
pub use copy::{
    CopyFailure,
    CopyFailureState,
};
pub use error::{
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
};
pub use file_system::FileSystem;
pub use file_system_properties::FileSystemProperties;
#[cfg(feature = "async")]
pub use handle::{
    AsyncDirectoryStream,
    AsyncFileReader,
    AsyncFileWriter,
    AsyncWriteAllFailure,
};
pub use handle::{
    DirectoryStream,
    FileReader,
    FileWriter,
    WriteAbortOutcome,
    WriteAllFailure,
    WriteFailure,
    WriteFailureState,
    WriterState,
};
pub use metadata::{
    AchievedAtomicity,
    Checksum,
    ChecksumAlgorithm,
    DirEntry,
    FileKind,
    FileMetadata,
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemCapabilitySupport,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimit,
    FileSystemLimits,
    NonSensitiveMetadata,
    OpenedFileInfo,
    PublicationMethod,
    ResourceVersion,
    UserMetadata,
    WriteOutcome,
};
pub use options::{
    AtomicityRequirement,
    ChecksumPolicy,
    CopyConflictPolicy,
    CopyMethod,
    CopyMode,
    CopyOptions,
    CopyOutcome,
    CopyStats,
    CreateDirectoryOptions,
    CreateDirectoryOutcome,
    DeleteOptions,
    DeleteOutcome,
    DurabilityRequirement,
    ListOptions,
    MetadataPreservePolicy,
    PersistOptions,
    ReadOptions,
    RenameOptions,
    RenameOutcome,
    ServerSidePreference,
    SymlinkPolicy,
    WriteDisposition,
    WriteOptions,
    WritePrecondition,
};
pub use path::{
    NativePathCodec,
    NativePathCodecError,
    Path,
    PathComponent,
    PathComponents,
    PathSemantics,
    RelativePath,
};
pub use path_constraints::PathConstraints;
pub use path_form::PathForm;
pub use rename::{
    RenameFailure,
    RenameFailureState,
};
#[cfg(feature = "async")]
pub use temp::{
    AsyncTempDirectory,
    AsyncTempFile,
};
pub use temp::{
    PersistCleanupState,
    PersistFailure,
    PersistFailureState,
    PersistOutcome,
    TempDirOptions as TempDirectoryOptions,
    TempDirectory,
    TempFile,
    TempFileOptions,
    TempResourceState,
};
pub use uri::{
    ConnectionUri,
    Uri,
};
