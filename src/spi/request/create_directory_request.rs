// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Validated directory-creation request.
//! The facade routes this request through `FileSystemSpi::create_directory`
//! after validating the path and resolving directory-creation options.

use super::internal::path_request;
use crate::spi::ResolvedCreateDirectoryOptions;

path_request!(CreateDirectoryRequest, ResolvedCreateDirectoryOptions);
