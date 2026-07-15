// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! SPI service specification for filesystems.

use std::sync::Arc;

use qubit_spi::ServiceSpec;

use crate::{FileSystem, FileSystemConfig};

/// Service specification for filesystem providers.
#[derive(Debug)]
pub struct FileSystemSpec;

impl ServiceSpec for FileSystemSpec {
    type Config = FileSystemConfig;
    type Output = Arc<dyn FileSystem>;
}
