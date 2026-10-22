// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use crate::error::FsOperation;
use crate::facade::facade_core::FacadeCore;
use crate::path::Path;

#[test]
fn test_quantity_conversion_is_executed_at_runtime() {
    let path = Path::parse("/facade-core").expect("valid path");
    assert_eq!(
        FacadeCore::quantity_from_usize(7, FsOperation::Read, &path, "test").expect("value fits"),
        7,
    );
}
