// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Validated rename request.

use crate::path::Path;
use crate::spi::ResolvedRenameOptions;

/// A rename request passed by the facade to `FileSystemSpi::rename` after it
/// validates both paths and resolves the rename policy.
///
/// Providers can inspect this request but cannot construct it.
///
/// # Type Parameters
/// - `'a`: The lifetime shared by the validated source and destination paths.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::RenameRequest;
///
/// fn inspect_rename_request(request: RenameRequest<'_>) -> (&Path, &Path) {
///     (request.source(), request.target())
/// }
///
/// // A provider receives this request from the facade and can inspect its
/// // resolved options as well.
/// let _ = inspect_rename_request;
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
#[must_use = "pass the rename request to the provider or explicitly drop it"]
pub struct RenameRequest<'a> {
    /// Validated source path.
    source: &'a Path,
    /// Validated destination path.
    target: &'a Path,
    /// Facade-resolved rename policy.
    options: ResolvedRenameOptions,
}

impl<'a> RenameRequest<'a> {
    /// Creates this request inside the facade boundary.
    ///
    /// # Parameters
    /// - `source`: Validated source path.
    /// - `target`: Validated destination path.
    /// - `options`: Facade-resolved rename options.
    ///
    /// # Returns
    /// A provider rename request borrowing both paths.
    #[allow(dead_code)]
    #[inline]
    pub(crate) const fn new(source: &'a Path, target: &'a Path, options: ResolvedRenameOptions) -> Self {
        Self {
            source,
            target,
            options,
        }
    }

    /// Returns the source path.
    ///
    /// # Returns
    /// The validated source path.
    #[inline]
    #[must_use]
    pub const fn source(&self) -> &'a Path {
        self.source
    }

    /// Returns the target path.
    ///
    /// # Returns
    /// The validated destination path.
    #[inline]
    #[must_use]
    pub const fn target(&self) -> &'a Path {
        self.target
    }

    /// Returns resolved options.
    ///
    /// # Returns
    /// The immutable facade-resolved rename options.
    #[inline]
    #[must_use = "inspect the resolved rename policy before provider dispatch"]
    pub const fn options(&self) -> &ResolvedRenameOptions {
        &self.options
    }
}
