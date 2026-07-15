// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! # Qubit FS
//!
//! Pluggable filesystem abstraction for Rust.
//!
//! This crate defines provider-neutral filesystem contracts, path and URI
//! models, metadata, operation options, copy outcomes, temporary resource
//! handles, and SPI-backed provider registry types.

#![deny(missing_docs)]

mod error;
mod metadata;
mod options;
mod path;
mod provider;
mod temp;
mod traits;

pub use error::{FsError, FsErrorKind, FsOperation, FsResult};
pub use metadata::{
    Checksum, ChecksumAlgorithm, DirEntry, FileKind, FileMetadata, FileSystemCapabilities,
    FileSystemMetadata, WriteOutcome,
};
pub use options::{
    AtomicityRequirement, ChecksumPolicy, CopyConflictPolicy, CopyMethod, CopyMode, CopyOptions,
    CopyOutcome, CopyStats, CreateDirOptions, DeleteOptions, ListOptions, MetadataPreservePolicy,
    PersistOptions, ProgressPolicy, ReadOptions, RenameOptions, ServerSidePreference, WriteMode,
    WriteOptions,
};
pub use path::{FsAuthority, FsPath, FsUri, PathSemantics};
pub use provider::{
    CredentialRef, FileResource, FileSystemConfig, FileSystemProvider, FileSystemRegistry,
    FileSystemRegistryBuilder, FileSystemSpec,
};
pub use temp::{
    ManagedTempDir, ManagedTempFile, ManagedTempResourceFactory, TempDir, TempDirOptions, TempFile,
    TempFileOptions, TempResource, TempResourceFactory, TempResources,
};
pub use traits::{
    DirectoryStream, DirectoryStreamExt, FileReader, FileSystem, FileSystemExt, FileWriter,
};
