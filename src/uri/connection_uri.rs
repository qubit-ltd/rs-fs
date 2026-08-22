// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Redacted connection URI values.

use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use fluent_uri::Uri as FluentUri;
use qubit_redact::RedactionCompletion;
use qubit_redact::RedactionPolicy;
use qubit_redact::Redactor;

use super::invalid_uri;
use super::uri::Uri;
use super::uri::parse_canonical;
use crate::error::FsResult;

/// A connection URI whose normal formatting always redacts credentials.
#[derive(Clone, Eq, PartialEq)]
pub struct ConnectionUri {
    /// RFC 3986 parser-owned raw connection URI representation.
    parsed: FluentUri<String>,
    /// Immutable policy snapshot used for secret classification and display.
    redaction_policy: RedactionPolicy,
}

impl ConnectionUri {
    /// Parses a connection URI with optional credentials but no fragment.
    ///
    /// Returns an invalid-URI error for malformed syntax or a fragment.
    pub fn parse(text: &str) -> FsResult<Self> {
        Self::parse_with_policy(text, &RedactionPolicy::standard())
    }

    /// Parses a connection URI using an explicit redaction policy snapshot.
    ///
    /// # Parameters
    ///
    /// * `text` - URI text to parse and canonicalize.
    /// * `policy` - Policy captured for later secret classification and
    ///   formatting.
    pub fn parse_with_policy(
        text: &str,
        policy: &RedactionPolicy,
    ) -> FsResult<Self> {
        let parsed = parse_canonical(text)?;
        if parsed.fragment().is_some() {
            return Err(invalid_uri("URI fragments are not supported"));
        }
        Ok(Self {
            parsed,
            redaction_policy: policy.clone(),
        })
    }

    /// Returns the normalized URI scheme without exposing credential-bearing
    /// components.
    #[must_use]
    #[inline(always)]
    pub fn scheme(&self) -> &str {
        self.parsed.scheme().as_str()
    }

    /// Returns whether the URI contains any component classified as sensitive
    /// by the policy snapshot captured during parsing.
    ///
    /// Username-only userinfo is not considered a secret because it can be
    /// paired with an external credential reference. Classification uses
    /// metadata-only inspection, so the diagnostic output budget cannot hide
    /// a late sensitive component. Invalid inspection, including an exceeded
    /// input budget or invalid encoded component, is treated conservatively as
    /// secret-bearing.
    ///
    /// # Returns
    ///
    /// `false` only after inspection passes through without a sensitive
    /// component; `true` after redaction or any invalid inspection result.
    #[inline]
    #[must_use]
    pub fn has_embedded_secret(&self) -> bool {
        let output = Redactor::new(self.redaction_policy.clone())
            .redact_uri(self.parsed.as_str());
        output.summary().completion() != RedactionCompletion::Complete
            || output.text().as_str() != self.parsed.as_str()
    }

    /// Converts this connection URI to a secret-free resource URI.
    ///
    /// # Errors
    ///
    /// Returns an invalid-URI error when the connection URI contains sensitive
    /// components that cannot appear in [`Uri`].
    #[inline(always)]
    pub fn try_to_uri(&self) -> FsResult<Uri> {
        Uri::parse_with_policy(self.parsed.as_str(), &self.redaction_policy)
    }

    /// Gives `inspect` ephemeral access to the unredacted URI text.
    ///
    /// The callback result is returned unchanged; callers must not use it to
    /// expose secret data through ordinary formatting or serialization.
    #[inline(always)]
    pub fn expose_unredacted<R>(&self, inspect: impl FnOnce(&str) -> R) -> R {
        inspect(self.parsed.as_str())
    }

    /// Renders the connection URI under the structured completion contract.
    ///
    /// A complete result preserves the full log-safe rendering. A truncated
    /// result contains only a substitute for omitted output, while an
    /// exhausted result means that no safe substitute fit and processing must
    /// stop without reading further input. Both incomplete states are mapped
    /// to one outer marker so normal formatting never exposes or mistakes a
    /// partial connection URI for a complete resource location.
    ///
    /// # Returns
    ///
    /// The complete redacted URI, or `<truncated>` when redaction did not
    /// complete.
    #[inline]
    #[must_use]
    fn redacted_text(&self) -> String {
        let redaction = Redactor::new(self.redaction_policy.clone())
            .redact_uri(self.parsed.as_str());
        match redaction.summary().completion() {
            RedactionCompletion::Complete => {
                redaction.into_text().into_string()
            }
            RedactionCompletion::Truncated | RedactionCompletion::Exhausted => {
                "<truncated>".to_owned()
            }
        }
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
        let redacted = self.redacted_text();
        formatter
            .debug_tuple("ConnectionUri")
            .field(&redacted)
            .finish()
    }
}
