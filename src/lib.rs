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
//! ```rust
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
//!
//! ```compile_fail
//! use qubit_fs::temp::TempFileOptions;
//! ```
//!
//! ```compile_fail
//! use qubit_fs::temp::TempDirOptions;
//! ```
//!
//! ```compile_fail
//! use qubit_fs::TempDirectoryOptions;
//! ```
//!
//! ```compile_fail
//! use qubit_fs::rustdoc_provider;
//! ```

#![deny(missing_docs)]

#[cfg(test)]
mod tests;

#[cfg(feature = "async")]
mod async_file_system;
pub mod copy;
pub mod directory;
pub mod error;
pub mod execution;
mod facade;
mod file_system;
mod file_system_properties;
pub mod metadata;
pub mod path;
mod path_constraints;
mod path_form;
pub mod read;
pub mod rename;
pub mod spi;
pub mod temp;
mod uri;
pub mod write;
#[cfg(feature = "async")]
pub use async_file_system::AsyncFileSystem;
pub use error::FsEffectState;
pub use error::FsError;
pub use error::FsResult;
pub use execution::AsyncFsMutationOperation;
pub use execution::AsyncFsOperation;
pub use execution::FsEffectCapacityError;
pub use execution::FsEffectFact;
pub use execution::FsEffectObservationError;
pub use execution::FsEffectObserver;
pub use execution::FsEffectReservation;
pub use execution::FsEffectSlotPool;
pub use execution::FsEffectSnapshot;
pub use execution::FsEffectWait;
pub use execution::FsExecutionBinding;
pub use execution::FsMutationOperation;
pub use execution::FsOperation;
pub use execution::FsOperationFailure;
pub use execution::FsOperationFailureKind;
pub use execution::FsOwnedCopyRequest;
pub use execution::FsOwnedReadRequest;
pub use execution::FsOwnedRenameRequest;
pub use execution::FsOwnedTreePublishRequest;
pub use execution::FsOwnedWriteRequest;
pub use execution::FsPrepareFailure;
pub use execution::FsPrepareFailureKind;
pub use execution::FsSubmitFailure;
pub use file_system::FileSystem;
pub use path::Path;
pub use read::ConsistentReadOutcome;
pub use read::ReadAllOptions;
pub use read::ReadConsistencyPolicy;
