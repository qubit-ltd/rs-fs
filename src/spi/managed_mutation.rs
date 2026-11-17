// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Owned mutation protocol declarations for managed providers.

mod managed_fs_mutation_spi;
mod prepared_fs_copy;
mod prepared_fs_rename;
mod prepared_fs_write;

pub use managed_fs_mutation_spi::ManagedFsMutationSpi;
pub use prepared_fs_copy::PreparedFsCopy;
pub use prepared_fs_rename::PreparedFsRename;
pub use prepared_fs_write::PreparedFsWrite;
