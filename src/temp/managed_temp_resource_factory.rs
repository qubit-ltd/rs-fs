/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Managed temporary resource factory.

use std::sync::Arc;

use crate::{
    CreateDirOptions,
    FileSystem,
    FsResult,
    ManagedTempDir,
    ManagedTempFile,
    TempDir,
    TempDirOptions,
    TempFile,
    TempFileOptions,
    TempResourceFactory,
    WriteMode,
    WriteOptions,
};

/// Default factory backed by generic [`FileSystem`] operations.
#[derive(Debug)]
pub struct ManagedTempResourceFactory;

impl ManagedTempResourceFactory {
    /// Returns the shared managed temporary resource factory.
    ///
    /// # Returns
    /// Static managed temporary resource factory instance.
    #[must_use]
    pub fn shared() -> &'static Self {
        static FACTORY: ManagedTempResourceFactory = ManagedTempResourceFactory;
        &FACTORY
    }
}

impl TempResourceFactory for ManagedTempResourceFactory {
    fn create_file(
        &self,
        owner: Arc<dyn FileSystem>,
        options: &TempFileOptions,
    ) -> FsResult<Box<dyn TempFile>> {
        let path =
            self.make_temp_path(options.parent.as_ref(), &options.prefix, &options.suffix)?;
        let writer_options = WriteOptions {
            create_parent: true,
            mode: WriteMode::CreateNew,
            ..WriteOptions::default()
        };
        owner.open_writer(&path, &writer_options)?.commit()?;
        Ok(Box::new(ManagedTempFile::new(owner, path)))
    }

    fn create_dir(
        &self,
        owner: Arc<dyn FileSystem>,
        options: &TempDirOptions,
    ) -> FsResult<Box<dyn TempDir>> {
        let path =
            self.make_temp_path(options.parent.as_ref(), &options.prefix, &options.suffix)?;
        owner.create_dir(
            &path,
            &CreateDirOptions {
                recursive: true,
                ..CreateDirOptions::default()
            },
        )?;
        Ok(Box::new(ManagedTempDir::new(owner, path)))
    }
}
