// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Unforgeable requests passed from the facade to providers.
//!
//! ```compile_fail
//! use qubit_fs::{Path, spi::StatRequest};
//!
//! let path = Path::root();
//! let _ = StatRequest::new(&path);
//! ```

use super::{
    ResolvedCopyOptions,
    ResolvedCreateDirectoryOptions,
    ResolvedDeleteOptions,
    ResolvedListOptions,
    ResolvedReadOptions,
    ResolvedRenameOptions,
    ResolvedWriteOptions,
};
use crate::{
    Path,
    PersistOptions,
    TempDirectoryOptions,
    TempFileOptions,
};

macro_rules! path_request {
    ($name:ident, $options:ty) => {
        /// A facade-created request with a validated logical path.
        pub struct $name<'a> {
            path: &'a Path,
            options: $options,
        }
        impl<'a> $name<'a> {
            /// Creates this request inside the facade boundary.
            pub(crate) const fn new(path: &'a Path, options: $options) -> Self {
                Self { path, options }
            }
            /// Returns the validated logical path.
            pub const fn path(&self) -> &'a Path {
                self.path
            }
            /// Returns the resolved operation options.
            pub const fn options(&self) -> &$options {
                &self.options
            }
        }
    };
}

path_request!(StatRequest, ());
path_request!(ListRequest, ResolvedListOptions);
path_request!(OpenReaderRequest, ResolvedReadOptions);
path_request!(OpenWriterRequest, ResolvedWriteOptions);
path_request!(CreateDirectoryRequest, ResolvedCreateDirectoryOptions);
path_request!(DeleteFileRequest, ResolvedDeleteOptions);
path_request!(DeleteDirectoryRequest, ResolvedDeleteOptions);

/// A facade-created copy request.
pub struct CopyRequest<'a> {
    source: &'a Path,
    target: &'a Path,
    options: ResolvedCopyOptions,
}
impl<'a> CopyRequest<'a> {
    /// Creates this request inside the facade boundary.
    #[allow(dead_code)]
    pub(crate) const fn new(
        source: &'a Path,
        target: &'a Path,
        options: ResolvedCopyOptions,
    ) -> Self {
        Self {
            source,
            target,
            options,
        }
    }
    /// Returns the source path.
    pub const fn source(&self) -> &'a Path {
        self.source
    }
    /// Returns the target path.
    pub const fn target(&self) -> &'a Path {
        self.target
    }
    /// Returns resolved options.
    pub const fn options(&self) -> &ResolvedCopyOptions {
        &self.options
    }
}

/// A facade-created rename request.
pub struct RenameRequest<'a> {
    source: &'a Path,
    target: &'a Path,
    options: ResolvedRenameOptions,
}
impl<'a> RenameRequest<'a> {
    /// Creates this request inside the facade boundary.
    #[allow(dead_code)]
    pub(crate) const fn new(
        source: &'a Path,
        target: &'a Path,
        options: ResolvedRenameOptions,
    ) -> Self {
        Self {
            source,
            target,
            options,
        }
    }
    /// Returns the source path.
    pub const fn source(&self) -> &'a Path {
        self.source
    }
    /// Returns the target path.
    pub const fn target(&self) -> &'a Path {
        self.target
    }
    /// Returns resolved options.
    pub const fn options(&self) -> &ResolvedRenameOptions {
        &self.options
    }
}

/// A facade-created temporary-file request.
pub struct CreateTempFileRequest {
    options: TempFileOptions,
}
impl CreateTempFileRequest {
    /// Creates this request inside the facade boundary.
    #[allow(dead_code)]
    pub(crate) const fn new(options: TempFileOptions) -> Self {
        Self { options }
    }
    /// Returns requested temporary-file options.
    pub const fn options(&self) -> &TempFileOptions {
        &self.options
    }
}
/// A facade-created temporary-directory request.
pub struct CreateTempDirectoryRequest {
    options: TempDirectoryOptions,
}
impl CreateTempDirectoryRequest {
    /// Creates this request inside the facade boundary.
    #[allow(dead_code)]
    pub(crate) const fn new(options: TempDirectoryOptions) -> Self {
        Self { options }
    }
    /// Returns requested temporary-directory options.
    pub const fn options(&self) -> &TempDirectoryOptions {
        &self.options
    }
}

/// A facade-created request to persist a temporary resource.
pub struct PersistRequest<'a> {
    target: &'a Path,
    options: PersistOptions,
}
impl<'a> PersistRequest<'a> {
    /// Creates the request within the facade boundary.
    pub(crate) const fn new(target: &'a Path, options: PersistOptions) -> Self {
        Self { target, options }
    }
    /// Returns the validated destination path.
    pub const fn target(&self) -> &'a Path {
        self.target
    }
    /// Returns persistence requirements.
    pub const fn options(&self) -> &PersistOptions {
        &self.options
    }
}
