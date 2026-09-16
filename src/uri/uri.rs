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
use qubit_redact::formats::uri::UriRedactionBoundary;

use super::invalid_uri;
use crate::error::FsResult;

/// A validated URI that cannot contain sensitive credentials or a fragment.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::path::Uri;
///
/// let uri = Uri::parse("file:///tmp/report.txt")?;
/// assert_eq!("file", uri.scheme());
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
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
    ///
    /// # Parameters
    /// - `text`: URI text to parse.
    /// - `policy`: Redaction policy used to reject sensitive components.
    ///
    /// # Errors
    /// Returns an invalid-URI error for malformed syntax, fragments, or
    /// sensitive components rejected by `policy`.
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
    ///
    /// The standard policy is always applied as a non-removable safety floor;
    /// an explicit policy can only add classifications. Provider-specific
    /// credentials that are not recognized by either policy remain the
    /// provider's responsibility to remove before constructing a URI.
    #[inline]
    pub fn parse_with_policy(text: &str, policy: &RedactionPolicy) -> FsResult<Self> {
        let parsed = parse_canonical(text)?;
        let floor = RedactionPolicy::standard();
        reject_secrets(&parsed, &floor)?;
        if policy != &floor {
            reject_secrets(&parsed, policy)?;
        }
        Ok(Self { parsed })
    }

    /// Returns the normalized lowercase scheme.
    #[inline]
    #[must_use]
    pub fn scheme(&self) -> &str {
        self.parsed.scheme().as_str()
    }

    /// Returns the raw RFC 3986 authority when it is syntactically present.
    #[inline]
    #[must_use]
    pub fn authority(&self) -> Option<&str> {
        self.parsed.authority().map(|authority| authority.as_str())
    }

    /// Returns whether an authority delimiter was present, including empty
    /// authority.
    #[inline]
    #[must_use]
    pub fn has_authority(&self) -> bool {
        self.parsed.has_authority()
    }

    /// Returns the raw percent-encoded path without decoding separators.
    #[inline]
    #[must_use]
    pub fn path(&self) -> &str {
        self.parsed.path().as_str()
    }

    /// Returns the raw ordered query text when a query delimiter was present.
    #[inline]
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.parsed.query().map(|query| query.as_str())
    }

    /// Returns the complete validated canonical URI spelling.
    #[inline]
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
pub(crate) fn reject_secrets(parsed: &FluentUri<String>, policy: &RedactionPolicy) -> FsResult<()> {
    if parsed.fragment().is_some() {
        return Err(invalid_uri("URI fragments are not supported"));
    }
    match UriRedactionBoundary::new(policy).inspect_uri(parsed.as_str()) {
        Ok(inspection) if !inspection.contains_sensitive() => {}
        Ok(_) => {
            return Err(invalid_uri("sensitive URI components are not supported"));
        }
        Err(_) => {
            return Err(invalid_uri("URI contains invalid or uninspectable components"));
        }
    }
    Ok(())
}

/// Classifies a raw metadata key through the shared URI query policy.
pub(crate) fn query_pair_is_sensitive(key: &str) -> bool {
    RedactionPolicy::standard().sensitivity_for(key).is_some()
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    use super::Uri;

    #[test]
    fn uri_accessors_are_executed_at_runtime() {
        let parse: fn(&str) -> crate::error::FsResult<Uri> = black_box(Uri::parse);
        let scheme: for<'a> fn(&'a Uri) -> &'a str = black_box(Uri::scheme);
        let authority: for<'a> fn(&'a Uri) -> Option<&'a str> = black_box(Uri::authority);
        let has_authority: fn(&Uri) -> bool = black_box(Uri::has_authority);
        let path: for<'a> fn(&'a Uri) -> &'a str = black_box(Uri::path);
        let query: for<'a> fn(&'a Uri) -> Option<&'a str> = black_box(Uri::query);
        let as_str: for<'a> fn(&'a Uri) -> &'a str = black_box(Uri::as_str);

        let uri = parse("HTTPS://example.test/path?query=value").expect("URI should parse");
        assert_eq!("https", scheme(&uri));
        assert_eq!(Some("example.test"), authority(&uri));
        assert!(has_authority(&uri));
        assert_eq!("/path", path(&uri));
        assert_eq!(Some("query=value"), query(&uri));
        assert_eq!("https://example.test/path?query=value", as_str(&uri));
        assert_eq!(as_str(&uri), format!("{uri}"));
    }
}
