// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Managed read support types and protocols.

mod async_managed_read_port;
mod internal;
mod managed_fs_read_spi;
mod managed_read_port;
mod prepared_fs_read;

#[cfg(feature = "async")]
pub use async_managed_read_port::AsyncManagedReadPort;
pub use managed_fs_read_spi::ManagedFsReadSpi;
pub use managed_read_port::ManagedReadPort;
pub use prepared_fs_read::PreparedFsRead;
