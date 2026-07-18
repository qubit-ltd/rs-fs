// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Path and URI models.

mod fs_authority;
mod fs_name;
mod fs_path;
mod fs_scheme;
mod fs_uri;
mod fs_uri_path;
mod fs_uri_query;
mod path_semantics;
mod relative_fs_path;
mod uri_codec;

pub use fs_authority::FsAuthority;
pub use fs_name::FsName;
pub use fs_path::FsPath;
pub use fs_scheme::FsScheme;
pub use fs_uri::FsUri;
pub use fs_uri_path::FsUriPath;
pub use fs_uri_query::FsUriQuery;
pub(crate) use fs_uri_query::is_sensitive_key;
pub use path_semantics::PathSemantics;
pub use relative_fs_path::RelativeFsPath;
