// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Metadata types exposed by filesystem implementations.

mod checksum;
mod checksum_algorithm;
mod dir_entry;
mod file_kind;
mod file_metadata;
mod file_system_capabilities;
mod file_system_metadata;
mod write_outcome;

pub use checksum::Checksum;
pub use checksum_algorithm::ChecksumAlgorithm;
pub use dir_entry::DirEntry;
pub use file_kind::FileKind;
pub use file_metadata::FileMetadata;
pub use file_system_capabilities::FileSystemCapabilities;
pub use file_system_metadata::FileSystemMetadata;
pub use write_outcome::WriteOutcome;
