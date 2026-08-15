// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::metadata::Checksum;
use qubit_fs::metadata::ChecksumAlgorithm;

#[test]
fn test_checksum_new_stores_algorithm_and_value() {
    let checksum = Checksum::new(ChecksumAlgorithm::Sha256, "abc");

    assert_eq!(ChecksumAlgorithm::Sha256, checksum.algorithm);
    assert_eq!("abc", checksum.value);
}
