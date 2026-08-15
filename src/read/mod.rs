// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! File read options, handles, and facade operation objects.

#[cfg(feature = "async")]
mod async_file_reader;
#[cfg(feature = "async")]
mod async_read_operation;
mod checksum_policy;
mod file_reader;
mod read_operation;
mod read_options;

#[cfg(feature = "async")]
pub use async_file_reader::AsyncFileReader;
#[cfg(feature = "async")]
pub(crate) use async_read_operation::AsyncReadOperation;
pub use checksum_policy::ChecksumPolicy;
pub use file_reader::FileReader;
pub(crate) use read_operation::ReadOperation;
pub use read_options::ReadOptions;
