// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Owned request types accepted by managed filesystem submissions.

// Owns the parameters for a complete-file read.
mod fs_owned_read_request;
// Owns the parameters for publishing a staged directory tree.
mod fs_owned_tree_publish_request;
// Owns the parameters for a write operation.
mod fs_owned_write_request;
// Owns the parameters for a copy operation.
mod fs_owned_copy_request;
// Owns the parameters for a rename operation.
mod fs_owned_rename_request;

pub use fs_owned_copy_request::FsOwnedCopyRequest;
pub use fs_owned_read_request::FsOwnedReadRequest;
pub use fs_owned_rename_request::FsOwnedRenameRequest;
pub use fs_owned_tree_publish_request::FsOwnedTreePublishRequest;
pub use fs_owned_write_request::FsOwnedWriteRequest;
