// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Secret-aware RFC 3986 URI value objects.

mod connection_uri;
#[allow(clippy::module_inception)]
mod uri;
mod uri_error;

pub use connection_uri::ConnectionUri;
pub use uri::Uri;
pub(crate) use uri::query_pair_is_sensitive;
pub(crate) use uri_error::invalid_uri;
