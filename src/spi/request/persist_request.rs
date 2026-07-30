// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Validated temporary-resource persistence request.

use crate::{
    Path,
    PersistOptions,
};

/// A facade-created request to persist a temporary resource.
pub struct PersistRequest<'a> {
    /// Validated persistence destination.
    target: &'a Path,
    /// Validated persistence requirements.
    options: PersistOptions,
}

impl<'a> PersistRequest<'a> {
    /// Creates the request within the facade boundary.
    ///
    /// # Parameters
    /// - `target`: Validated persistence destination.
    /// - `options`: Validated persistence requirements.
    ///
    /// # Returns
    /// A provider persistence request borrowing `target`.
    #[inline]
    pub(crate) const fn new(target: &'a Path, options: PersistOptions) -> Self {
        Self { target, options }
    }

    /// Returns the validated destination path.
    ///
    /// # Returns
    /// The validated persistence destination.
    #[inline(always)]
    #[must_use]
    pub const fn target(&self) -> &'a Path {
        self.target
    }

    /// Returns persistence requirements.
    ///
    /// # Returns
    /// The immutable persistence requirements.
    #[inline(always)]
    #[must_use]
    pub const fn options(&self) -> &PersistOptions {
        &self.options
    }
}
