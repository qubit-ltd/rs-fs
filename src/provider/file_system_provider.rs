/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Filesystem provider trait object alias.

use qubit_spi::ServiceProvider;

use super::file_system_spec::FileSystemSpec;

/// Filesystem provider trait object type.
pub type FileSystemProvider = dyn ServiceProvider<FileSystemSpec>;
