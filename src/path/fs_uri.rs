// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Full filesystem URI model.

use qubit_metadata::Metadata;
use url::Url;

use crate::{
    FsAuthority,
    FsError,
    FsOperation,
    FsPath,
    FsResult,
};

/// Full filesystem URI.
#[derive(Clone, Debug, PartialEq)]
pub struct FsUri {
    /// Provider scheme.
    pub scheme: String,
    /// Optional URI authority.
    pub authority: Option<FsAuthority>,
    /// Provider-local path.
    pub path: FsPath,
    /// Non-sensitive query options.
    pub query: Metadata,
}

impl FsUri {
    /// Parses a full filesystem URI.
    ///
    /// # Parameters
    /// - `uri`: URI string to parse.
    ///
    /// # Returns
    /// Parsed filesystem URI.
    ///
    /// # Errors
    /// Returns [`FsError`] when the URI or its path is invalid.
    pub fn parse(uri: &str) -> FsResult<Self> {
        let parsed = Url::parse(uri).map_err(|error| {
            FsError::with_source(
                crate::FsErrorKind::InvalidPath,
                FsOperation::ParseUri,
                "invalid filesystem URI",
                error,
            )
        })?;
        let scheme = parsed.scheme().to_ascii_lowercase();
        let authority = parsed.host_str().map(|host| {
            let mut authority = FsAuthority::new(host);
            if let Some(port) = parsed.port() {
                authority = authority.with_port(port);
            }
            let username = parsed.username();
            if !username.is_empty() {
                authority = authority.with_username(username);
            }
            authority
        });
        let path = FsPath::parse(parsed.path()).map_err(|error| {
            FsError::new(
                error.kind(),
                FsOperation::ParseUri,
                "URI path is not a valid filesystem path",
            )
        })?;
        let mut query = Metadata::new();
        for (key, value) in parsed.query_pairs() {
            query.set(key.as_ref(), value.into_owned());
        }
        Ok(Self {
            scheme,
            authority,
            path,
            query,
        })
    }
}
