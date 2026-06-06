// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Write operation options.

use qubit_metadata::Metadata;

use crate::{
    Checksum,
    WriteMode,
};

/// Options controlling a write operation.
#[derive(Clone, Debug, PartialEq)]
pub struct WriteOptions {
    /// Whether missing parent directories should be created.
    pub create_parent: bool,
    /// Write creation mode.
    pub mode: WriteMode,
    /// Optional content type.
    pub content_type: Option<String>,
    /// User-defined metadata to attach to the resource.
    pub user_metadata: Metadata,
    /// Optional expected content checksum.
    pub checksum: Option<Checksum>,
}

impl Default for WriteOptions {
    #[inline]
    fn default() -> Self {
        Self {
            create_parent: false,
            mode: WriteMode::default(),
            content_type: None,
            user_metadata: Metadata::new(),
            checksum: None,
        }
    }
}
