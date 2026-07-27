// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Full filesystem URI model.

use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::{FsAuthority, FsResult, FsScheme, FsUriAuthority, FsUriPath, FsUriQuery};

use super::uri_codec::invalid_uri;

/// Full filesystem URI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FsUri {
    /// Provider scheme.
    scheme: FsScheme,
    /// Optional URI authority.
    authority: Option<FsAuthority>,
    /// Whether the URI used the hierarchical `//` authority form.
    authority_present: bool,
    /// Raw encoded URI path.
    path: FsUriPath,
    /// Non-sensitive query options.
    query: FsUriQuery,
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
    /// Returns an invalid-URI error when the URI contains malformed syntax,
    /// a fragment, password, sensitive query, invalid percent encoding,
    /// invalid decoded UTF-8, or control characters.
    pub fn parse(uri: &str) -> FsResult<Self> {
        if uri.chars().any(char::is_control) {
            return Err(invalid_uri("filesystem URI must not contain controls"));
        }
        if uri.contains('#') {
            return Err(invalid_uri("filesystem URI fragments are forbidden"));
        }
        let Some(scheme_end) = uri.find(':') else {
            return Err(invalid_uri("filesystem URI has no scheme"));
        };
        let scheme = FsScheme::parse(&uri[..scheme_end])?;
        let remainder = &uri[scheme_end + 1..];
        if remainder.is_empty() {
            return Err(invalid_uri("filesystem URI has no resource path"));
        }
        let (location, query) = match remainder.split_once('?') {
            Some((location, query)) => (location, FsUriQuery::parse(query)?),
            None => (remainder, FsUriQuery::default()),
        };
        let authority_present = location.starts_with("//");
        let (authority, encoded_path) = match location.strip_prefix("//") {
            Some(authority_and_path) => match authority_and_path.find('/') {
                Some(path_index) => {
                    let authority = &authority_and_path[..path_index];
                    let path = &authority_and_path[path_index..];
                    let authority = if authority.is_empty() {
                        None
                    } else {
                        Some(FsAuthority::parse_encoded(authority)?)
                    };
                    (authority, path)
                }
                None => {
                    let authority = if authority_and_path.is_empty() {
                        None
                    } else {
                        Some(FsAuthority::parse_encoded(authority_and_path)?)
                    };
                    (authority, "/")
                }
            },
            None => (None, location),
        };
        let path = FsUriPath::parse(encoded_path)?;
        Ok(Self {
            scheme,
            authority,
            authority_present,
            path,
            query,
        })
    }

    /// Creates a URI from independently validated components.
    ///
    /// # Errors
    ///
    /// Returns an invalid-URI error when an authority is paired with a
    /// relative path, or when an authority-free URI uses a path beginning in
    /// `//` that would be reparsed as an authority component.
    #[inline]
    pub fn new(
        scheme: FsScheme,
        authority: Option<FsAuthority>,
        path: FsUriPath,
        query: FsUriQuery,
    ) -> FsResult<Self> {
        let authority = match authority {
            Some(authority) => FsUriAuthority::Present(authority),
            None => FsUriAuthority::Absent,
        };
        Self::new_with_authority(scheme, authority, path, query)
    }

    /// Creates a URI from independently validated components while preserving
    /// whether an authority was absent, empty, or non-empty.
    ///
    /// # Errors
    ///
    /// Returns an invalid-URI error when an authority component is paired with
    /// a relative path, or when an authority-free URI uses a path beginning in
    /// `//` that would be reparsed as an authority component.
    #[inline]
    pub fn new_with_authority(
        scheme: FsScheme,
        authority: FsUriAuthority,
        path: FsUriPath,
        query: FsUriQuery,
    ) -> FsResult<Self> {
        let (authority, authority_present) = match authority {
            FsUriAuthority::Absent => (None, false),
            FsUriAuthority::Empty => (None, true),
            FsUriAuthority::Present(authority) => (Some(authority), true),
        };
        if authority_present && !path.as_encoded().starts_with('/') {
            return Err(invalid_uri(
                "filesystem URI authority requires an absolute path",
            ));
        }
        if !authority_present && path.as_encoded().starts_with("//") {
            return Err(invalid_uri(
                "authority-free filesystem URI path must not begin with //",
            ));
        }
        Ok(Self {
            scheme,
            authority,
            authority_present,
            path,
            query,
        })
    }

    /// Returns the normalized provider-selection scheme.
    #[inline]
    #[must_use]
    pub const fn scheme(&self) -> &FsScheme {
        &self.scheme
    }

    /// Returns the optional resource authority.
    #[inline]
    #[must_use]
    pub const fn authority(&self) -> Option<&FsAuthority> {
        self.authority.as_ref()
    }

    /// Returns whether this URI has a hierarchical authority component.
    ///
    /// This is `true` for an explicitly empty authority such as `file:///tmp`
    /// even though [`Self::authority`] returns `None`.
    ///
    /// # Returns
    /// Whether `//` was present after the scheme.
    #[inline]
    #[must_use]
    pub const fn has_authority_component(&self) -> bool {
        self.authority_present
    }

    /// Returns the raw encoded URI path.
    #[inline]
    #[must_use]
    pub const fn path(&self) -> &FsUriPath {
        &self.path
    }

    /// Returns the ordered non-sensitive URI query.
    #[inline]
    #[must_use]
    pub const fn query(&self) -> &FsUriQuery {
        &self.query
    }
}

impl Display for FsUri {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        write!(formatter, "{}:", self.scheme)?;
        if self.authority_present {
            formatter.write_str("//")?;
            if let Some(authority) = &self.authority {
                write!(formatter, "{authority}")?;
            }
        }
        formatter.write_str(self.path.as_encoded())?;
        if !self.query.is_empty() {
            write!(formatter, "?{}", self.query)?;
        }
        Ok(())
    }
}
