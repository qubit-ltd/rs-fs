/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Provider configuration for filesystem creation.

use qubit_metadata::Metadata;

use crate::{
    CredentialRef,
    FsUri,
};

/// Configuration passed to filesystem providers.
#[derive(Clone, Debug, PartialEq)]
pub struct FileSystemConfig {
    /// URI used to select and initialize the provider.
    pub uri: FsUri,
    /// Non-sensitive provider options.
    pub options: Metadata,
    /// Optional credential reference.
    pub credentials: Option<CredentialRef>,
}

impl FileSystemConfig {
    /// Creates provider configuration from a URI.
    ///
    /// # Parameters
    /// - `uri`: Parsed filesystem URI.
    ///
    /// # Returns
    /// Provider configuration with no extra options or credentials.
    #[inline]
    #[must_use]
    pub fn new(uri: FsUri) -> Self {
        Self {
            uri,
            options: Metadata::new(),
            credentials: None,
        }
    }
}
