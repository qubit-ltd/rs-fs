// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Reader trait for filesystem resources.

use std::io::Read;

use crate::FileMetadata;

/// Read handle returned by filesystem implementations.
pub trait FileReader: Read + Send {
    /// Gets metadata associated with this reader when it was opened.
    ///
    /// # Returns
    /// `Some` metadata when the provider captured it, or `None` otherwise.
    fn metadata(&self) -> Option<&FileMetadata> {
        None
    }
}

impl<T> FileReader for T where T: Read + Send {}
