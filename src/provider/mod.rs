/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! SPI-backed filesystem provider registry.

mod credential_ref;
mod file_system_config;
mod file_system_registry;
mod file_system_resolver;
mod file_system_spec;
mod resolved_path;

pub use credential_ref::CredentialRef;
pub use file_system_config::FileSystemConfig;
pub use file_system_registry::FileSystemProvider;
pub use file_system_registry::FileSystemRegistry;
pub use file_system_resolver::FileSystemResolver;
pub use file_system_spec::FileSystemSpec;
pub use resolved_path::ResolvedPath;
