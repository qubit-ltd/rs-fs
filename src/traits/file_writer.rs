// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Writer trait for filesystem resources.

use std::io::Write;

use crate::{
    FsResult,
    WriteOutcome,
};

/// Write handle returned by filesystem implementations.
pub trait FileWriter: Write + Send {
    /// Commits the write operation.
    ///
    /// # Returns
    /// Write outcome reported by the provider.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the provider cannot commit the write.
    fn commit(self: Box<Self>) -> FsResult<WriteOutcome>;

    /// Aborts the write operation.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the provider cannot abort or clean up
    /// the write session.
    fn abort(self: Box<Self>) -> FsResult<()>;
}
