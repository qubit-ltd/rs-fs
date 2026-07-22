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
//! resource lifecycles, and synchronous/asynchronous provider registries. It
//! contains no concrete storage backend and binds to no asynchronous runtime.

#![deny(missing_docs)]

mod error;
mod metadata;
mod options;
mod path;
mod provider;
mod temp;
mod traits;

pub use error::{
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
};
pub use metadata::{
    AchievedAtomicity,
    Checksum,
    ChecksumAlgorithm,
    DirEntry,
    FileKind,
    FileLocation,
    FileMetadata,
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimit,
    FileSystemLimits,
    NonSensitiveMetadata,
    OpenedFileInfo,
    PublicationMethod,
    ResourceVersion,
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
    CreateDirOptions,
    DeleteOptions,
    ListOptions,
    MetadataPreservePolicy,
    PersistOptions,
    ProgressPolicy,
    ReadOptions,
    RenameOptions,
    RenameOutcome,
    ServerSidePreference,
    WriteDisposition,
    WriteOptions,
    WritePrecondition,
};
pub use path::{
    EscapedBytePathCodec,
    FsAuthority,
    FsName,
    FsPath,
    FsScheme,
    FsUri,
    FsUriPath,
    FsUriQuery,
    NativePathCodec,
    NativePathCodecError,
    OsStrPathCodec,
    PathSemantics,
    RelativeFsPath,
    Utf8PathCodec,
};
pub use provider::{
    AsyncFileResource,
    AsyncFileSystemProvider,
    AsyncFileSystemRegistry,
    CredentialRef,
    FileResource,
    FileSystemConfig,
    FileSystemProvider,
    FileSystemRegistry,
    FileSystemResolution,
    FileSystemSpec,
    map_async_provider_error,
};
pub use temp::{
    AsyncTempDir,
    AsyncTempFile,
    AsyncTempResourceSession,
    PersistFailure,
    PersistFailureState,
    PersistFuture,
    PersistOutcome,
    TempDir,
    TempDirOptions,
    TempFile,
    TempFileOptions,
    TempResourceSession,
    TempResourceState,
};
pub use traits::{
    AsyncDirectoryStream,
    AsyncDirectoryStreamExt,
    AsyncDirectoryStreamSession,
    AsyncFileReader,
    AsyncFileSystem,
    AsyncFileSystemExt,
    AsyncFileWriteSession,
    AsyncFileWriter,
    DirectoryStream,
    DirectoryStreamExt,
    DirectoryStreamSession,
    FileReader,
    FileSystem,
    FileSystemExt,
    FileSystemProperties,
    FileWriteSession,
    FileWriter,
    FsFuture,
    WriterState,
};
