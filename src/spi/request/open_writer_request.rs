// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Validated writer-open request.
//! The facade routes this request through `FileSystemSpi::open_writer` after
//! validating the path and resolving write options.

use super::internal::path_request;
use crate::spi::ResolvedWriteOptions;

path_request!(OpenWriterRequest, ResolvedWriteOptions);
