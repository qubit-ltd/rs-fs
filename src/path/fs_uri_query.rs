// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Ordered non-sensitive filesystem URI query.

use std::fmt::{
    Display,
    Formatter,
    Result as FmtResult,
};

use crate::FsResult;

use super::uri_codec::{
    invalid_uri,
    percent_decode,
    percent_encode_query,
};

/// An ordered multi-map of decoded, non-sensitive URI query pairs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FsUriQuery {
    pairs: Vec<(Box<str>, Box<str>)>,
}

impl FsUriQuery {
    /// Parses a raw query while preserving pair order and duplicate keys.
    ///
    /// # Errors
    ///
    /// Returns an invalid-URI error for malformed encoding, control
    /// characters, empty keys, or sensitive credential-like keys.
    pub fn parse(query: &str) -> FsResult<Self> {
        if query.is_empty() {
            return Ok(Self::default());
        }
        let mut pairs = Vec::new();
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let key = percent_decode(key)?;
            let value = percent_decode(value)?;
            if key.is_empty() {
                return Err(invalid_uri("URI query key must not be empty"));
            }
            if is_sensitive_key(&key) {
                return Err(invalid_uri(
                    "sensitive credentials are forbidden in filesystem URIs",
                ));
            }
            pairs.push((key.into(), value.into()));
        }
        Ok(Self { pairs })
    }

    /// Returns all values associated with `key` in encounter order.
    #[must_use]
    pub fn get_all(&self, key: &str) -> Vec<&str> {
        self.pairs
            .iter()
            .filter_map(|(candidate, value)| {
                (candidate.as_ref() == key).then_some(value.as_ref())
            })
            .collect()
    }

    /// Iterates over decoded key-value pairs in encounter order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.pairs
            .iter()
            .map(|(key, value)| (key.as_ref(), value.as_ref()))
    }

    /// Returns whether the query contains no pairs.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

impl Display for FsUriQuery {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        for (index, (key, value)) in self.pairs.iter().enumerate() {
            if index > 0 {
                formatter.write_str("&")?;
            }
            write!(
                formatter,
                "{}={}",
                percent_encode_query(key),
                percent_encode_query(value),
            )?;
        }
        Ok(())
    }
}

/// Returns whether a query key is reserved for credential material.
pub(crate) fn is_sensitive_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    const SENSITIVE_MARKERS: &[&str] = &[
        "password",
        "passwd",
        "token",
        "accesskey",
        "secret",
        "apikey",
        "credential",
        "authorization",
        "bearer",
        "privatekey",
        "signature",
    ];
    normalized == "sig"
        || SENSITIVE_MARKERS
            .iter()
            .any(|marker| normalized.contains(marker))
}
