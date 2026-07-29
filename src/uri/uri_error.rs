// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

// qubit-style: allow all -- facade integration tests exercise this API group.
//! URI error construction shared by URI values.

use crate::{
    FsError,
    FsErrorKind,
    FsOperation,
};

/// Builds a sanitized invalid-URI error without retaining input text.
pub(crate) fn invalid_uri(message: &'static str) -> FsError {
    FsError::new(FsErrorKind::InvalidUri, FsOperation::ParseUri, message)
}
