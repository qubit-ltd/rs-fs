// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Operation option and outcome types.

mod atomicity_requirement;
mod checksum_policy;
mod copy_conflict_policy;
mod copy_method;
mod copy_mode;
mod copy_options;
mod copy_outcome;
mod copy_stats;
mod create_dir_options;
mod delete_options;
mod list_options;
mod metadata_preserve_policy;
mod persist_options;
mod read_options;
mod rename_options;
mod rename_outcome;
mod server_side_preference;
mod write_disposition;
mod write_options;
mod write_precondition;

pub use atomicity_requirement::AtomicityRequirement;
pub use checksum_policy::ChecksumPolicy;
pub use copy_conflict_policy::CopyConflictPolicy;
pub use copy_method::CopyMethod;
pub use copy_mode::CopyMode;
pub use copy_options::CopyOptions;
pub use copy_outcome::CopyOutcome;
pub use copy_stats::CopyStats;
pub use create_dir_options::CreateDirOptions;
pub use delete_options::DeleteOptions;
pub use list_options::ListOptions;
pub use metadata_preserve_policy::MetadataPreservePolicy;
pub use persist_options::PersistOptions;
pub use read_options::ReadOptions;
pub use rename_options::RenameOptions;
pub use rename_outcome::RenameOutcome;
pub use server_side_preference::ServerSidePreference;
pub use write_disposition::WriteDisposition;
pub use write_options::WriteOptions;
pub use write_precondition::WritePrecondition;
