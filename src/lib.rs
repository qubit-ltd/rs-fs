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
//! ## Binding a resource
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use qubit_fs::{
//!     FileResource,
//!     FileSystem,
//!     FsPath,
//!     FsResult,
//! };
//!
//! fn bind_report(filesystem: Arc<dyn FileSystem>) -> FsResult<FileResource> {
//!     let path = FsPath::parse("/reports/2026/summary.csv")?;
//!     Ok(FileResource::new(filesystem, path))
//! }
//! ```

#![deny(missing_docs)]

mod error;
mod metadata;
mod options;
mod path;
mod resource;
mod temp;
mod traits;

pub use error::{FsError, FsErrorKind, FsOperation, FsResult};
pub use metadata::{
    AchievedAtomicity, Checksum, ChecksumAlgorithm, DirEntry, FileKind, FileLocation, FileMetadata,
    FileSystemCapabilities, FileSystemCapability, FileSystemId, FileSystemInfo, FileSystemLimit,
    FileSystemLimits, NonSensitiveMetadata, OpenedFileInfo, PublicationMethod, ResourceVersion,
    UserMetadata, WriteOutcome,
};
pub use options::{
    AtomicityRequirement, ChecksumPolicy, CopyConflictPolicy, CopyMethod, CopyMode, CopyOptions,
    CopyOutcome, CopyStats, CreateDirOptions, DeleteOptions, ListOptions, MetadataPreservePolicy,
    PersistOptions, ReadOptions, RenameOptions, RenameOutcome, ServerSidePreference,
    WriteDisposition, WriteOptions, WritePrecondition,
};
pub use path::{
    EscapedBytePathCodec, FsAuthority, FsName, FsPath, FsScheme, FsUri, FsUriAuthority, FsUriPath,
    FsUriQuery, NativePathCodec, NativePathCodecError, OsStrPathCodec, PathSemantics,
    RelativeFsPath, Utf8PathCodec,
};
pub use resource::{AsyncFileResource, FileResource};
pub use temp::{
    AsyncTempDir, AsyncTempFile, AsyncTempResourceSession, PersistFailure, PersistFailureState,
    PersistFuture, PersistOutcome, TempDir, TempDirOptions, TempFile, TempFileOptions,
    TempResourceSession, TempResourceState,
};
pub use traits::{
    AsyncDirectoryStream, AsyncDirectoryStreamExt, AsyncDirectoryStreamSession, AsyncFileReader,
    AsyncFileSystem, AsyncFileSystemExt, AsyncFileWriteSession, AsyncFileWriter, DirectoryStream,
    DirectoryStreamExt, DirectoryStreamSession, FileReader, FileSystem, FileSystemExt,
    FileSystemProperties, FileWriteSession, FileWriter, FsFuture, WriteFailure, WriteFailureState,
    WriteFuture, WriterState,
};
