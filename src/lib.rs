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

#![deny(missing_docs)]

#[cfg(feature = "async")]
mod async_file_system;
pub mod copy;
pub mod directory;
pub mod error;
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
pub use file_system::FileSystem;
pub use path::Path;
