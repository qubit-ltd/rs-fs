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
pub use copy::AsyncCopyFailure;
#[cfg(feature = "async")]
pub use copy::AsyncCopyOperation;
#[cfg(feature = "async")]
pub use copy::AsyncCopyOperationState;
pub use copy::CopyFailure;
pub use copy::CopyFailureState;
pub use error::FsError;
pub use error::FsErrorKind;
pub use error::FsOperation;
pub use error::FsResult;
pub use file_system::FileSystem;
pub use file_system_properties::FileSystemProperties;
#[cfg(feature = "async")]
pub use handle::AsyncDirectoryStream;
#[cfg(feature = "async")]
pub use handle::AsyncFileReader;
#[cfg(feature = "async")]
pub use handle::AsyncFileWriter;
#[cfg(feature = "async")]
pub use handle::AsyncWriteAllFailure;
pub use handle::DirectoryStream;
pub use handle::DirectoryStreamState;
pub use handle::FileReader;
pub use handle::FileWriter;
pub use handle::WriteAbortOutcome;
pub use handle::WriteAllFailure;
pub use handle::WriteFailure;
pub use handle::WriteFailureState;
pub use handle::WriterState;
pub use metadata::AchievedAtomicity;
pub use metadata::Checksum;
pub use metadata::ChecksumAlgorithm;
pub use metadata::DirEntry;
pub use metadata::FileKind;
pub use metadata::FileMetadata;
pub use metadata::FileSystemCapabilities;
pub use metadata::FileSystemCapability;
pub use metadata::FileSystemCapabilitySupport;
pub use metadata::FileSystemId;
pub use metadata::FileSystemInfo;
pub use metadata::FileSystemLimit;
pub use metadata::FileSystemLimits;
pub use metadata::NonSensitiveMetadata;
pub use metadata::OpenedFileInfo;
pub use metadata::PublicationMethod;
pub use metadata::ResourceVersion;
pub use metadata::UserMetadata;
pub use metadata::WriteOutcome;
pub use options::AtomicityRequirement;
pub use options::ChecksumPolicy;
pub use options::CopyConflictPolicy;
pub use options::CopyMethod;
pub use options::CopyMode;
pub use options::CopyOptions;
pub use options::CopyOutcome;
pub use options::CopyStats;
pub use options::CreateDirectoryOptions;
pub use options::CreateDirectoryOutcome;
pub use options::DeleteOptions;
pub use options::DeleteOutcome;
pub use options::DurabilityRequirement;
pub use options::ListOptions;
pub use options::MetadataPreservePolicy;
pub use options::PersistOptions;
pub use options::ReadOptions;
pub use options::RenameOptions;
pub use options::RenameOutcome;
pub use options::ServerSidePreference;
pub use options::SymlinkPolicy;
pub use options::WriteDisposition;
pub use options::WriteOptions;
pub use options::WritePrecondition;
pub use path::NativePathCodec;
pub use path::NativePathCodecError;
pub use path::Path;
pub use path::PathComponent;
pub use path::PathComponents;
pub use path::PathSemantics;
pub use path::RelativePath;
pub use path_constraints::PathConstraints;
pub use path_form::PathForm;
pub use rename::RenameFailure;
pub use rename::RenameFailureState;
#[cfg(feature = "async")]
pub use temp::AsyncTempDirectory;
#[cfg(feature = "async")]
pub use temp::AsyncTempFile;
pub use temp::PersistCleanupState;
pub use temp::PersistFailure;
pub use temp::PersistFailureState;
pub use temp::PersistOutcome;
pub use temp::TempDirOptions as TempDirectoryOptions;
pub use temp::TempDirectory;
pub use temp::TempFile;
pub use temp::TempFileOptions;
pub use temp::TempResourceState;
pub use uri::ConnectionUri;
pub use uri::Uri;
