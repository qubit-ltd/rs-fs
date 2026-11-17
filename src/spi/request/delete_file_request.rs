// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Validated file-deletion request.
//! The facade routes this request through `FileSystemSpi::delete_file` after
//! validating the path and resolving deletion options.

use super::internal::path_request;
use crate::spi::ResolvedDeleteOptions;

path_request!(DeleteFileRequest, ResolvedDeleteOptions);
