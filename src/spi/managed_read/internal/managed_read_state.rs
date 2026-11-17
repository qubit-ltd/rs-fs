// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared state retained by managed synchronous read ports.

use crate::metadata::OpenedFileInfo;
use crate::spi::RandomAccessReadSession;

/// Shared synchronization and budget state for cloned synchronous read ports.
pub(in crate::spi::managed_read) struct ManagedReadState {
    /// Metadata observed when the provider session was opened.
    pub(in crate::spi::managed_read) info: OpenedFileInfo,
    /// Readable length associated with the retained session.
    pub(in crate::spi::managed_read) len: u64,
    /// Maximum byte count allowed in each provider call.
    pub(in crate::spi::managed_read) max_segment: usize,
    /// Cumulative byte limit shared by all clones.
    pub(in crate::spi::managed_read) max_total: u64,
    /// Successful bytes consumed across all clones.
    pub(in crate::spi::managed_read) consumed: u64,
    /// Provider session bound to the opened object and guarded by the port
    /// lock.
    pub(in crate::spi::managed_read) session: Box<dyn RandomAccessReadSession>,
}
