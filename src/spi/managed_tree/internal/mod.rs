// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private helpers for staged-tree publication.

mod staged_directory_cleanup;
mod staged_directory_state;

pub(in crate::spi::managed_tree) use staged_directory_cleanup::StagedDirectoryCleanup;
pub(in crate::spi::managed_tree) use staged_directory_state::StagedDirectoryState;
