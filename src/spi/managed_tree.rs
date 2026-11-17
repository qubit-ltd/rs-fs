// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Managed staged-tree publication support types and protocols.

mod internal;
mod managed_fs_tree_spi;
mod prepared_fs_tree_publish;
mod staged_directory;

pub use managed_fs_tree_spi::ManagedFsTreeSpi;
pub use prepared_fs_tree_publish::PreparedFsTreePublish;
pub use staged_directory::StagedDirectory;
