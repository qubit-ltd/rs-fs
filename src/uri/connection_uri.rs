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
use qubit_redact::{
    Redactor,
    Sensitivity,
};

use crate::FsResult;

use super::{
    invalid_uri,
    uri::parse_canonical,
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

    /// Gives `inspect` ephemeral access to the unredacted URI text.
    ///
    /// The callback result is returned unchanged; callers must not use it to
    /// expose secret data through ordinary formatting or serialization.
    pub fn expose_unredacted<R>(&self, inspect: impl FnOnce(&str) -> R) -> R {
        inspect(self.parsed.as_str())
    }

    /// Renders a URI while preserving component order and masking sensitive
    /// values.
    fn redacted_text(&self) -> String {
        let scheme = self.parsed.scheme().as_str();
        let mut rendered = String::from(scheme);
        rendered.push(':');
        if self.parsed.has_authority() {
            rendered.push_str("//");
            if let Some(authority) = self.parsed.authority() {
                rendered.push_str(&redact_authority(authority.as_str()));
            }
        }
        rendered.push_str(self.parsed.path().as_str());
        if let Some(query) = self.parsed.query() {
            rendered.push('?');
            rendered.push_str(&redact_query(query.as_str()));
        }
        rendered
    }
}

impl Display for ConnectionUri {
    /// Formats only the redacted connection URI.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(&self.redacted_text())
    }
}

impl Debug for ConnectionUri {
    /// Formats only the redacted connection URI for diagnostics.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_tuple("ConnectionUri")
            .field(&self.redacted_text())
            .finish()
    }
}

/// Redacts all userinfo in an authority while retaining the host text.
fn redact_authority(authority: &str) -> String {
    let Some((userinfo, host)) = authority.rsplit_once('@') else {
        return authority.to_owned();
    };
    let masked = Redactor::default()
        .redact_at(Sensitivity::Secret, userinfo)
        .into_owned();
    format!("{masked}@{host}")
}

/// Redacts sensitive ordered query values without decoding or collapsing
/// duplicates.
fn redact_query(query: &str) -> String {
    query
        .split('&')
        .map(redact_query_pair)
        .collect::<Vec<_>>()
        .join("&")
}

/// Redacts one raw query pair while retaining its key and separator spelling.
fn redact_query_pair(pair: &str) -> String {
    let Some((key, value)) = pair.split_once('=') else {
        return pair.to_owned();
    };
    if super::uri::query_pair_is_sensitive(pair) {
        let masked = Redactor::default()
            .redact_at(Sensitivity::Secret, value)
            .into_owned();
        format!("{key}={masked}")
    } else {
        pair.to_owned()
    }
}
