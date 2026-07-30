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

use crate::Path;
use crate::spi::ResolvedCopyOptions;

/// A facade-created copy request.
pub struct CopyRequest<'a> {
    source: &'a Path,
    target: &'a Path,
    options: ResolvedCopyOptions,
}

impl<'a> CopyRequest<'a> {
    /// Creates this request inside the facade boundary.
    #[allow(dead_code)]
    #[inline(always)]
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
    #[inline(always)]
    #[must_use]
    pub const fn source(&self) -> &'a Path {
        self.source
    }

    /// Returns the target path.
    #[inline(always)]
    #[must_use]
    pub const fn target(&self) -> &'a Path {
        self.target
    }

    /// Returns resolved options.
    #[inline(always)]
    #[must_use]
    pub const fn options(&self) -> &ResolvedCopyOptions {
        &self.options
    }
}
