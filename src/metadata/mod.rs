// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Metadata types exposed by filesystem implementations.

mod achieved_atomicity;
mod atomicity_requirement;
mod checksum;
mod checksum_algorithm;
mod content_precondition;
mod dir_entry;
mod durability_requirement;
mod file_kind;
mod file_metadata;
mod file_system_capabilities;
mod file_system_capability;
mod file_system_capability_profile;
mod file_system_capability_support;
mod file_system_id;
mod file_system_info;
mod file_system_limit;
mod file_system_limits;
mod non_sensitive_metadata;
mod opened_file_info;
mod privacy_requirement;
mod publication_method;
mod resource_version;
mod storage_observation;
mod symlink_policy;
mod tree_publish_options;
mod tree_validation_options;
mod user_metadata;
mod write_outcome;

pub use achieved_atomicity::AchievedAtomicity;
pub use atomicity_requirement::AtomicityRequirement;
pub use checksum::Checksum;
pub use checksum_algorithm::ChecksumAlgorithm;
pub use content_precondition::ContentPrecondition;
pub use dir_entry::DirEntry;
pub use durability_requirement::DurabilityRequirement;
pub use file_kind::FileKind;
pub use file_metadata::FileMetadata;
pub use file_system_capabilities::FileSystemCapabilities;
pub use file_system_capability::FileSystemCapability;
pub use file_system_capability_profile::FileSystemCapabilityProfile;
pub use file_system_capability_support::FileSystemCapabilitySupport;
pub use file_system_id::FileSystemId;
pub use file_system_info::FileSystemInfo;
pub use file_system_limit::FileSystemLimit;
pub use file_system_limits::FileSystemLimits;
pub use non_sensitive_metadata::NonSensitiveMetadata;
pub use opened_file_info::OpenedFileInfo;
pub use privacy_requirement::PrivacyRequirement;
pub use publication_method::PublicationMethod;
pub use resource_version::ResourceVersion;
pub use storage_observation::StorageObservation;
pub use symlink_policy::SymlinkPolicy;
pub use tree_publish_options::TreePublishOptions;
pub use tree_validation_options::TreeValidationOptions;
pub use user_metadata::UserMetadata;
pub use write_outcome::WriteOutcome;

pub use crate::file_system_properties::FileSystemProperties;
