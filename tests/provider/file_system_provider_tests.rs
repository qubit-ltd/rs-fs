// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::Arc;

use qubit_fs::FileSystemProvider;
use qubit_spi::{
    ProviderDescriptor,
    ProviderId,
};

use crate::common::{
    MockFs,
    MockProvider,
};

#[test]
fn test_file_system_provider_alias_accepts_provider_trait_objects() {
    let provider: Arc<FileSystemProvider> = Arc::new(MockProvider {
        descriptor: ProviderDescriptor::new(
            ProviderId::new("mock").expect("valid provider ID"),
        ),
        fs: MockFs::default(),
    });

    assert_eq!(1, Arc::strong_count(&provider));
}
