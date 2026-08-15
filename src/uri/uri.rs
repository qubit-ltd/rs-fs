// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Secret-free resource URI values.

use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use fluent_uri::Uri as FluentUri;
use qubit_redact::RedactionPolicy;
use qubit_redact::uri::UriRedactionStatus;
use qubit_redact::uri::UriRedactor;

use super::invalid_uri;
use crate::error::FsResult;

/// A validated URI that cannot contain sensitive credentials or a fragment.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Uri {
    /// RFC 3986 parser-owned lexical URI representation.
    parsed: FluentUri<String>,
}

impl Uri {
    /// Parses a secret-free RFC 3986 URI.
    ///
    /// Returns an invalid-URI error for malformed syntax, fragments, or URI
    /// components classified as sensitive by the fixed standard policy.
    #[inline]
    pub fn parse(text: &str) -> FsResult<Self> {
        Self::parse_with_policy(text, &RedactionPolicy::standard())
    }

    /// Parses a secret-free URI using an explicit redaction policy snapshot.
    ///
    /// # Parameters
    ///
    /// * `text` - URI text to parse and canonicalize.
    /// * `policy` - Policy used to classify sensitive URI components.
    #[inline]
    pub fn parse_with_policy(
        text: &str,
        policy: &RedactionPolicy,
    ) -> FsResult<Self> {
        let parsed = parse_canonical(text)?;
        reject_secrets(&parsed, policy)?;
        Ok(Self { parsed })
    }

    /// Returns the normalized lowercase scheme.
    #[inline(always)]
    #[must_use]
    pub fn scheme(&self) -> &str {
        self.parsed.scheme().as_str()
    }

    /// Returns the raw RFC 3986 authority when it is syntactically present.
    #[inline(always)]
    #[must_use]
    pub fn authority(&self) -> Option<&str> {
        self.parsed.authority().map(|authority| authority.as_str())
    }

    /// Returns whether an authority delimiter was present, including empty
    /// authority.
    #[inline(always)]
    #[must_use]
    pub fn has_authority(&self) -> bool {
        self.parsed.has_authority()
    }

    /// Returns the raw percent-encoded path without decoding separators.
    #[inline(always)]
    #[must_use]
    pub fn path(&self) -> &str {
        self.parsed.path().as_str()
    }

    /// Returns the raw ordered query text when a query delimiter was present.
    #[inline(always)]
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.parsed.query().map(|query| query.as_str())
    }

    /// Returns the complete validated canonical URI spelling.
    #[inline(always)]
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.parsed.as_str()
    }
}

impl Display for Uri {
    /// Formats the lossless validated URI spelling.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_str())
    }
}

/// Parses a URI after normalizing only the case-insensitive scheme.
pub(crate) fn parse_canonical(text: &str) -> FsResult<FluentUri<String>> {
    let (scheme, rest) = text
        .split_once(':')
        .ok_or_else(|| invalid_uri("URI must include a scheme"))?;
    if scheme.is_empty() {
        return Err(invalid_uri("URI scheme must not be empty"));
    }
    let canonical = format!("{}:{rest}", scheme.to_ascii_lowercase());
    FluentUri::parse(canonical).map_err(|_| invalid_uri("URI is malformed"))
}

/// Rejects fragments and URI components classified as sensitive.
pub(crate) fn reject_secrets(
    parsed: &FluentUri<String>,
    policy: &RedactionPolicy,
) -> FsResult<()> {
    if parsed.fragment().is_some() {
        return Err(invalid_uri("URI fragments are not supported"));
    }
    let result =
        UriRedactor::new(policy.clone()).inspect_uri_str(parsed.as_str());
    if result.status() == UriRedactionStatus::Invalid {
        return Err(invalid_uri("URI contains invalid encoded components"));
    }
    if result.has_sensitive_components() {
        return Err(invalid_uri("sensitive URI components are not supported"));
    }
    Ok(())
}

/// Classifies a raw metadata key through the shared URI query policy.
pub(crate) fn query_pair_is_sensitive(key: &str) -> bool {
    UriRedactor::default()
        .policy()
        .sensitivity_for(key)
        .is_some()
}
