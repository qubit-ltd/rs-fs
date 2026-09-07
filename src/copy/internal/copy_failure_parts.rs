// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private storage for synchronous copy failure details.
use crate::copy::CopyFailureState;
use crate::copy::CopyStats;
use crate::error::FsError;
use crate::write::FileWriter;
/// Heap-owned error and writer storage behind the public failure.
pub(in crate::copy) struct CopyFailureParts {
    /// Contextual primary error.
    pub(in crate::copy) error: FsError,
    /// Publication certainty at failure.
    pub(in crate::copy) state: CopyFailureState,
    /// Confirmed progress before the operation stopped.
    pub(in crate::copy) partial_stats: CopyStats,
    /// Destination session retained for explicit recovery.
    pub(in crate::copy) writer: Option<Box<FileWriter>>,
}
