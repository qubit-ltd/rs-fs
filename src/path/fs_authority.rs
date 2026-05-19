/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! URI authority model.

/// Provider-neutral URI authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FsAuthority {
    /// Host, bucket, namespace, or service endpoint name.
    pub host: String,
    /// Optional network port.
    pub port: Option<u16>,
    /// Optional username hint. Secrets must not be stored here.
    pub username: Option<String>,
}

impl FsAuthority {
    /// Creates an authority from a host-like name.
    ///
    /// # Parameters
    /// - `host`: Host, bucket, namespace, or service endpoint name.
    ///
    /// # Returns
    /// New authority with no port or username.
    #[inline]
    #[must_use]
    pub fn new(host: &str) -> Self {
        Self {
            host: host.to_owned(),
            port: None,
            username: None,
        }
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
    #[inline]
    #[must_use]
    pub fn with_username(mut self, username: &str) -> Self {
        if !username.is_empty() {
            self.username = Some(username.to_owned());
        }
        self
    }
}
