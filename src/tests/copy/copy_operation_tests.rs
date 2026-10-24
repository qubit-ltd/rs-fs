// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for crate-internal synchronous copy operation contracts.

use crate::copy::CopyOptions;
use crate::copy::StreamCopyPlan;
use crate::error::FsOperation;
use crate::metadata::FileSystemLimits;
use crate::path::Path;

#[test]
fn test_copied_byte_accounting_is_executed_at_runtime() {
    let source = Path::parse("/source").expect("valid source path");
    let target = Path::parse("/target").expect("valid target path");
    let options = CopyOptions::default();
    let limits = FileSystemLimits::unknown();
    let plan = StreamCopyPlan::new(&options, &limits, &source, &target);

    assert_eq!(plan.next_bytes(4, 3).expect("value fits"), 7);
    let error = plan.next_bytes(u64::MAX, 1).expect_err("overflow must be rejected");
    assert_eq!(error.kind(), crate::error::FsErrorKind::ResourceLimitExceeded);
    assert_eq!(error.operation(), FsOperation::Copy);
    assert_eq!(
        plan.next_bytes(u64::MAX, 1)
            .expect_err("overflow must be rejected")
            .kind(),
        crate::error::FsErrorKind::ResourceLimitExceeded
    );
}
