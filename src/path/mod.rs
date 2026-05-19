/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Path and URI models.

mod fs_authority;
mod fs_path;
mod fs_uri;
mod path_semantics;

pub use fs_authority::FsAuthority;
pub use fs_path::FsPath;
pub use fs_uri::FsUri;
pub use path_semantics::PathSemantics;
