// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! SPI-backed filesystem provider registry.

mod async_file_resource;
mod async_file_system_provider;
mod async_file_system_registry;
mod credential_ref;
mod file_resource;
mod file_system_config;
mod file_system_provider;
mod file_system_registry;
mod file_system_resolution;
mod file_system_spec;

pub use async_file_resource::AsyncFileResource;
pub use async_file_system_provider::AsyncFileSystemProvider;
pub use async_file_system_registry::AsyncFileSystemRegistry;
pub use credential_ref::CredentialRef;
pub use file_resource::FileResource;
pub use file_system_config::FileSystemConfig;
pub use file_system_provider::FileSystemProvider;
pub use file_system_registry::FileSystemRegistry;
pub use file_system_resolution::FileSystemResolution;
pub use file_system_spec::FileSystemSpec;
