// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FileSystemRegistry;
use qubit_spi::{
    ProviderDescriptor,
    ProviderId,
};

use crate::common::{
    MockFs,
    MockProvider,
};

#[test]
fn test_builder_registers_self_described_provider_and_builds_runtime_registry()
{
    let mut builder = FileSystemRegistry::builder();
    builder
        .register(MockProvider {
            descriptor: descriptor("first"),
            fs: MockFs::default(),
        })
        .expect("self-described provider should register through the builder");
    let registry = builder.build();

    registry
        .register(MockProvider {
            descriptor: descriptor("second"),
            fs: MockFs::default(),
        })
        .expect("built registry should remain runtime mutable");

    assert_eq!(vec!["first", "second"], registry.provider_ids());
}

fn descriptor(id: &str) -> ProviderDescriptor {
    ProviderDescriptor::new(
        ProviderId::new(id).expect("test provider ID should be valid"),
    )
}
