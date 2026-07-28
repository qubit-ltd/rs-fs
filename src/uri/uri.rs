//! Secret-free resource URI values.

use std::fmt::{Display, Formatter, Result as FmtResult};

use fluent_uri::Uri as FluentUri;
use qubit_redact::RedactionPolicy;

use crate::FsResult;

use super::invalid_uri;

/// A validated URI that cannot contain credentials or a fragment.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Uri {
    /// RFC 3986 parser-owned lexical URI representation.
    parsed: FluentUri<String>,
}

impl Uri {
    /// Parses a secret-free RFC 3986 URI.
    ///
    /// Returns an invalid-URI error for malformed syntax, fragments,
    /// userinfo passwords, or sensitive query fields.
    pub fn parse(text: &str) -> FsResult<Self> {
        let parsed = parse_canonical(text)?;
        reject_secrets(&parsed)?;
        Ok(Self { parsed })
    }

    /// Returns the normalized lowercase scheme.
    #[must_use]
    pub fn scheme(&self) -> &str {
        self.parsed.scheme().as_str()
    }

    /// Returns the raw RFC 3986 authority when it is syntactically present.
    #[must_use]
    pub fn authority(&self) -> Option<&str> {
        self.parsed.authority().map(|authority| authority.as_str())
    }

    /// Returns whether an authority delimiter was present, including empty authority.
    #[must_use]
    pub fn has_authority(&self) -> bool {
        self.parsed.has_authority()
    }

    /// Returns the raw percent-encoded path without decoding separators.
    #[must_use]
    pub fn path(&self) -> &str {
        self.parsed.path().as_str()
    }

    /// Returns the raw ordered query text when a query delimiter was present.
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.parsed.query().map(|query| query.as_str())
    }

    /// Returns the complete validated canonical URI spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.parsed.as_str()
    }
}

impl Display for Uri {
    /// Formats the lossless validated URI spelling.
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

/// Rejects credentials and fragments from the secret-free URI boundary.
pub(crate) fn reject_secrets(parsed: &FluentUri<String>) -> FsResult<()> {
    if parsed.fragment().is_some() {
        return Err(invalid_uri("URI fragments are not supported"));
    }
    if parsed
        .authority()
        .is_some_and(|authority| authority.has_userinfo())
    {
        return Err(invalid_uri("URI userinfo is not supported"));
    }
    if parsed
        .query()
        .is_some_and(|query| query.as_str().split('&').any(query_pair_is_sensitive))
    {
        return Err(invalid_uri("sensitive URI query fields are not supported"));
    }
    Ok(())
}

/// Classifies one raw query pair without decoding or reordering it.
pub(crate) fn query_pair_is_sensitive(pair: &str) -> bool {
    let key = pair.split_once('=').map_or(pair, |(key, _)| key);
    RedactionPolicy::default()
        .classify_field(key)
        .sensitivity()
        .is_some()
}
