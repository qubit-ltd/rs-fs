// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem result type alias.

use super::fs_error::FsError;

/// Result type used by filesystem APIs.
pub type FsResult<T> = Result<T, FsError>;
