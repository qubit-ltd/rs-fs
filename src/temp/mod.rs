/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Temporary filesystem resource handles.

mod managed_temp_dir;
mod managed_temp_file;
mod managed_temp_resource_factory;
mod temp_dir;
mod temp_dir_options;
mod temp_file;
mod temp_file_options;
mod temp_resource;
mod temp_resource_factory;
mod temp_resources;

pub use managed_temp_dir::ManagedTempDir;
pub use managed_temp_file::ManagedTempFile;
pub use managed_temp_resource_factory::ManagedTempResourceFactory;
pub use temp_dir::TempDir;
pub use temp_dir_options::TempDirOptions;
pub use temp_file::TempFile;
pub use temp_file_options::TempFileOptions;
pub use temp_resource::TempResource;
pub use temp_resource_factory::TempResourceFactory;
pub use temp_resources::TempResources;
