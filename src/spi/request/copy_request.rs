// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Validated copy request.

use crate::path::Path;
use crate::spi::ResolvedCopyOptions;

/// A facade-created copy request.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::copy::CopyOptions;
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::CopyRequest;
///
/// let source = Path::parse("/source")?;
/// let target = Path::parse("/target")?;
/// assert!(std::any::type_name::<CopyRequest<'_>>().contains("CopyRequest"));
/// assert_ne!(source.as_str(), target.as_str());
/// let _ = CopyOptions::default();
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
pub struct CopyRequest<'a> {
    /// Validated source path.
    source: &'a Path,
    /// Validated destination path.
    target: &'a Path,
    /// Facade-resolved copy policy.
    options: ResolvedCopyOptions,
}

impl<'a> CopyRequest<'a> {
    /// Creates this request inside the facade boundary.
    ///
    /// # Parameters
    /// - `source`: Validated source path.
    /// - `target`: Validated destination path.
    /// - `options`: Facade-resolved copy options.
    ///
    /// # Returns
    /// A provider copy request borrowing both paths.
    #[allow(dead_code)]
    #[inline]
    pub(crate) const fn new(source: &'a Path, target: &'a Path, options: ResolvedCopyOptions) -> Self {
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
    /// The immutable facade-resolved copy options.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &ResolvedCopyOptions {
        &self.options
    }
}
