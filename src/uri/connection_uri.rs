// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Redacted connection URI values.

use std::fmt::{
    Debug,
    Display,
    Formatter,
    Result as FmtResult,
};

use fluent_uri::Uri as FluentUri;
use qubit_redact::UriRedactor;

use crate::FsResult;

use super::{
    invalid_uri,
    uri::{
        Uri,
        parse_canonical,
    },
};

/// A connection URI whose normal formatting always redacts credentials.
#[derive(Clone, Eq, PartialEq)]
pub struct ConnectionUri {
    /// RFC 3986 parser-owned raw connection URI representation.
    parsed: FluentUri<String>,
}

impl ConnectionUri {
    /// Parses a connection URI with optional credentials but no fragment.
    ///
    /// Returns an invalid-URI error for malformed syntax or a fragment.
    pub fn parse(text: &str) -> FsResult<Self> {
        let parsed = parse_canonical(text)?;
        if parsed.fragment().is_some() {
            return Err(invalid_uri("URI fragments are not supported"));
        }
        Ok(Self { parsed })
    }

    /// Returns the normalized URI scheme without exposing credential-bearing
    /// components.
    #[must_use]
    #[inline(always)]
    pub fn scheme(&self) -> &str {
        self.parsed.scheme().as_str()
    }

    /// Returns whether the URI contains any component classified as sensitive
    /// by the process URI policy.
    ///
    /// Username-only userinfo is not considered a secret because it can be
    /// paired with an external credential reference.
    #[inline]
    #[must_use]
    pub fn has_embedded_secret(&self) -> bool {
        UriRedactor::default()
            .redact_uri_str(self.parsed.as_str())
            .has_sensitive_components()
    }

    /// Converts this connection URI to a secret-free resource URI.
    ///
    /// # Errors
    ///
    /// Returns an invalid-URI error when the connection URI contains sensitive
    /// components that cannot appear in [`Uri`].
    #[inline(always)]
    pub fn try_to_uri(&self) -> FsResult<Uri> {
        Uri::parse(self.parsed.as_str())
    }

    /// Gives `inspect` ephemeral access to the unredacted URI text.
    ///
    /// The callback result is returned unchanged; callers must not use it to
    /// expose secret data through ordinary formatting or serialization.
    #[inline(always)]
    pub fn expose_unredacted<R>(&self, inspect: impl FnOnce(&str) -> R) -> R {
        inspect(self.parsed.as_str())
    }

    /// Renders a URI while preserving component order and masking sensitive
    /// values through the shared URI redactor.
    fn redacted_text(&self) -> String {
        UriRedactor::default()
            .redact_uri_str(self.parsed.as_str())
            .into_log_safe_text()
    }
}

impl Display for ConnectionUri {
    /// Formats only the redacted connection URI.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(&self.redacted_text())
    }
}

impl Debug for ConnectionUri {
    /// Formats only the redacted connection URI for diagnostics.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_tuple("ConnectionUri")
            .field(&self.redacted_text())
            .finish()
    }
}
