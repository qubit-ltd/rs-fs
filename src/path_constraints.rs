// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Immutable path constraints used by filesystem property snapshots.

use crate::{
    FsError,
    FsOperation,
    FsResult,
    Path,
    PathForm,
};

/// Immutable path form constraints attached to one filesystem snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathConstraints {
    form: PathForm,
}

impl PathConstraints {
    /// Creates constraints accepting only absolute paths.
    #[inline(always)]
    #[must_use]
    pub const fn absolute() -> Self {
        Self {
            form: PathForm::Absolute,
        }
    }

    /// Creates constraints accepting only relative paths.
    #[inline(always)]
    #[must_use]
    pub const fn relative() -> Self {
        Self {
            form: PathForm::Relative,
        }
    }

    /// Creates constraints accepting either logical path form.
    #[inline(always)]
    #[must_use]
    pub const fn either() -> Self {
        Self {
            form: PathForm::Either,
        }
    }

    /// Returns the configured accepted path form.
    #[inline(always)]
    #[must_use]
    pub const fn form(&self) -> PathForm {
        self.form
    }

    /// Validates a logical path without performing I/O.
    ///
    /// # Parameters
    /// - `path`: Logical path whose absolute or relative form is checked.
    ///
    /// # Errors
    /// Returns an invalid-path error when `path` has a disallowed form.
    #[inline]
    pub fn validate(&self, path: &Path) -> FsResult<()> {
        let allowed = matches!(self.form, PathForm::Either)
            || matches!(
                (self.form, path.is_absolute()),
                (PathForm::Absolute, true) | (PathForm::Relative, false)
            );
        if allowed {
            Ok(())
        } else {
            Err(FsError::invalid_path(
                FsOperation::ParsePath,
                "path form is not accepted by this filesystem",
            ))
        }
    }
}
