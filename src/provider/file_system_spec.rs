// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! SPI service specification for filesystems.

use qubit_spi::ServiceSpec;

use crate::{
    FileSystem,
    FileSystemConfig,
    FileSystemResolution,
};

/// Service specification for filesystem providers.
#[derive(Debug)]
pub struct FileSystemSpec;

impl ServiceSpec for FileSystemSpec {
    type Config = FileSystemConfig;
    type Output = FileSystemResolution<dyn FileSystem>;
}
