// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! URI authority model.

use std::fmt::{Display, Formatter, Result as FmtResult};
use std::net::Ipv6Addr;

use crate::FsResult;

use super::uri_codec::{invalid_uri, percent_decode, percent_encode_query};

/// Provider-neutral URI authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FsAuthority {
    /// Host, bucket, namespace, or service endpoint name.
    host: Box<str>,
    /// Optional network port.
    port: Option<u16>,
    /// Optional username hint. Secrets must not be stored here.
    username: Option<Box<str>>,
}

impl FsAuthority {
    /// Creates an authority from a host-like name.
    ///
    /// # Parameters
    /// - `host`: Host, bucket, namespace, or service endpoint name.
    ///
    /// # Returns
    /// New authority with no port or username.
    ///
    /// # Errors
    ///
    /// Returns an invalid-URI error when `host` is empty, is not a valid
    /// ASCII URI host, or contains an invalid IPv6 address.
    #[inline]
    pub fn new(host: &str) -> FsResult<Self> {
        validate_host(host)?;
        Ok(Self {
            host: host.into(),
            port: None,
            username: None,
        })
    }

    /// Parses the raw authority portion of a filesystem URI.
    pub(super) fn parse_encoded(authority: &str) -> FsResult<Self> {
        let (userinfo, host_port) = match authority.rsplit_once('@') {
            Some((userinfo, host_port)) => {
                if userinfo.contains('@') {
                    return Err(invalid_uri("invalid URI authority user-info"));
                }
                (Some(userinfo), host_port)
            }
            None => (None, authority),
        };
        let username = match userinfo {
            Some(userinfo) => {
                if userinfo.contains(':') {
                    return Err(invalid_uri(
                        "passwords are forbidden in filesystem URI authority",
                    ));
                }
                let username = percent_decode(userinfo)?;
                validate_username(&username)?;
                Some(username.into_boxed_str())
            }
            None => None,
        };
        let (host, port) = parse_host_port(host_port)?;
        Ok(Self {
            host: host.into(),
            port,
            username,
        })
    }

    /// Sets the authority port.
    ///
    /// # Parameters
    /// - `port`: Network port.
    ///
    /// # Returns
    /// Updated authority.
    #[inline]
    #[must_use]
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Sets the username hint.
    ///
    /// # Parameters
    /// - `username`: Username hint. This must not contain a password or token.
    ///
    /// # Returns
    /// Updated authority.
    ///
    /// # Errors
    ///
    /// Returns an invalid-URI error when `username` is empty, contains a
    /// control character, or contains `:`, which would make it ambiguous with
    /// password-bearing user-info.
    #[inline]
    pub fn with_username(mut self, username: &str) -> FsResult<Self> {
        validate_username(username)?;
        self.username = Some(username.into());
        Ok(self)
    }

    /// Returns the host, bucket, or endpoint name.
    #[inline(always)]
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Returns the optional network port.
    #[inline(always)]
    #[must_use]
    pub const fn port(&self) -> Option<u16> {
        self.port
    }

    /// Returns the optional non-sensitive username hint.
    #[inline(always)]
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }
}

impl Display for FsAuthority {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        if let Some(username) = &self.username {
            write!(formatter, "{}@", percent_encode_query(username))?;
        }
        if self.host.contains(':') {
            write!(formatter, "[{}]", self.host)?;
        } else {
            formatter.write_str(&self.host)?;
        }
        if let Some(port) = self.port {
            write!(formatter, ":{port}")?;
        }
        Ok(())
    }
}

/// Splits and validates an authority host and optional port.
fn parse_host_port(host_port: &str) -> FsResult<(&str, Option<u16>)> {
    if host_port.is_empty() {
        return Err(invalid_uri("URI authority host must not be empty"));
    }
    if host_port.chars().any(char::is_control) {
        return Err(invalid_uri("URI authority must not contain controls"));
    }
    if let Some(rest) = host_port.strip_prefix('[') {
        let Some(end) = rest.find(']') else {
            return Err(invalid_uri("unterminated IPv6 URI authority"));
        };
        let host = &rest[..end];
        validate_host(host)?;
        let suffix = &rest[end + 1..];
        let port = if suffix.is_empty() {
            None
        } else {
            let value = suffix
                .strip_prefix(':')
                .ok_or_else(|| invalid_uri("invalid IPv6 URI authority suffix"))?;
            Some(parse_port(value)?)
        };
        return Ok((host, port));
    }
    if host_port.matches(':').count() > 1 {
        return Err(invalid_uri("IPv6 URI authorities must use brackets"));
    }
    match host_port.rsplit_once(':') {
        Some((host, port)) => {
            validate_host(host)?;
            Ok((host, Some(parse_port(port)?)))
        }
        None => {
            validate_host(host_port)?;
            Ok((host_port, None))
        }
    }
}

/// Validates one decoded host representation used by an authority.
fn validate_host(host: &str) -> FsResult<()> {
    if host.is_empty() {
        return Err(invalid_uri("URI authority host must not be empty"));
    }
    if host.contains(':') {
        host.parse::<Ipv6Addr>()
            .map_err(|_| invalid_uri("invalid IPv6 URI authority"))?;
        return Ok(());
    }
    if !host.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
            )
    }) {
        return Err(invalid_uri("invalid URI authority host"));
    }
    Ok(())
}

/// Validates a decoded, non-secret username hint.
fn validate_username(username: &str) -> FsResult<()> {
    if username.is_empty() {
        return Err(invalid_uri("URI username hint must not be empty"));
    }
    if username.contains(':') {
        return Err(invalid_uri(
            "passwords are forbidden in filesystem URI authority",
        ));
    }
    if username.chars().any(char::is_control) {
        return Err(invalid_uri("URI username hint must not contain controls"));
    }
    Ok(())
}

/// Parses a decimal URI port.
fn parse_port(port: &str) -> FsResult<u16> {
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid_uri("invalid URI authority port"));
    }
    port.parse()
        .map_err(|_| invalid_uri("URI authority port is out of range"))
}
