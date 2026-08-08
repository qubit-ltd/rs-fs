// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Metadata types exposed by filesystem implementations.

mod achieved_atomicity;
mod checksum;
mod checksum_algorithm;
mod dir_entry;
mod file_kind;
mod file_metadata;
mod file_system_capabilities;
mod file_system_capability;
mod file_system_capability_support;
mod file_system_id;
mod file_system_info;
mod file_system_limit;
mod file_system_limits;
mod non_sensitive_metadata;
mod opened_file_info;
mod publication_method;
mod resource_version;
mod user_metadata;
mod write_outcome;

pub use achieved_atomicity::AchievedAtomicity;
pub use checksum::Checksum;
pub use checksum_algorithm::ChecksumAlgorithm;
pub use dir_entry::DirEntry;
pub use file_kind::FileKind;
pub use file_metadata::FileMetadata;
pub use file_system_capabilities::FileSystemCapabilities;
pub use file_system_capability::FileSystemCapability;
pub use file_system_capability_support::FileSystemCapabilitySupport;
pub use file_system_id::FileSystemId;
pub use file_system_info::FileSystemInfo;
pub use file_system_limit::FileSystemLimit;
pub use file_system_limits::FileSystemLimits;
pub use non_sensitive_metadata::NonSensitiveMetadata;
pub use opened_file_info::OpenedFileInfo;
pub use publication_method::PublicationMethod;
pub use resource_version::ResourceVersion;
pub use user_metadata::UserMetadata;
pub use write_outcome::WriteOutcome;
