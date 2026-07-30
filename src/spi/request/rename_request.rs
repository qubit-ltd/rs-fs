// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Validated rename request.

use crate::Path;
use crate::spi::ResolvedRenameOptions;

/// A facade-created rename request.
pub struct RenameRequest<'a> {
    source: &'a Path,
    target: &'a Path,
    options: ResolvedRenameOptions,
}

impl<'a> RenameRequest<'a> {
    /// Creates this request inside the facade boundary.
    #[allow(dead_code)]
    #[inline(always)]
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
    pub const fn options(&self) -> &ResolvedRenameOptions {
        &self.options
    }
}
