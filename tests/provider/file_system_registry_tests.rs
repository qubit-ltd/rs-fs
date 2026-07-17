// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::{
    error::Error as _,
    sync::Arc,
};

use qubit_fs::{
    FileSystemConfig,
    FileSystemProvider,
    FileSystemRegistry,
    FileSystemSpec,
    FsErrorKind,
    FsUri,
};
use qubit_spi::error::{
    ProviderCreationError,
    ProviderError,
    ProviderSelectionError,
    RegistrationError,
};
use qubit_spi::{
    FallbackPolicy,
    ProviderDescriptor,
    ProviderId,
    ProviderSelection,
    ServiceProvider,
};

use crate::common::{
    FailingCreateProvider,
    MockFs,
    MockProvider,
};

#[test]
fn test_registry_registers_provider_at_runtime_and_shares_updates_with_clones()
{
    let fs = MockFs::default();
    let registry = FileSystemRegistry::default();
    let consumer_registry = registry.clone();
    registry
        .register(MockProvider {
            descriptor: mock_descriptor(),
            fs,
        })
        .expect("provider should register at runtime");

    assert_eq!(vec!["mock"], registry.provider_ids());
    let uri = FsUri::parse("mem:///file.txt").expect("URI should parse");
    let opened = consumer_registry
        .fs(&uri)
        .expect("shared registry clone should resolve the new alias");
    assert!(opened.capabilities().directories);
}

#[test]
fn test_registry_maps_registration_conflicts_separately() {
    let registry = registry_with(MockProvider {
        descriptor: mock_descriptor(),
        fs: MockFs::default(),
    });
    let error = registry
        .register(MockProvider {
            descriptor: mock_descriptor(),
            fs: MockFs::default(),
        })
        .expect_err("duplicate provider selector should fail registration");

    assert_eq!(FsErrorKind::Conflict, error.kind());
    assert!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<RegistrationError>())
            .is_some(),
        "registration failures should retain RegistrationError as source",
    );
}

#[test]
fn test_registry_registers_shared_provider_definition() {
    let registry = FileSystemRegistry::default();
    let provider: Arc<FileSystemProvider> = Arc::new(MockProvider {
        descriptor: mock_descriptor(),
        fs: MockFs::default(),
    });

    registry
        .register_shared(provider)
        .expect("shared provider should register");

    assert_eq!(vec!["mock"], registry.provider_ids());
}

#[test]
fn test_registry_returns_error_for_missing_provider() {
    let registry = registry_with(MockProvider {
        descriptor: mock_descriptor(),
        fs: MockFs::default(),
    });

    let missing_uri =
        FsUri::parse("missing:///file.txt").expect("URI should parse");
    let error = registry
        .fs(&missing_uri)
        .expect_err("missing provider should fail selection");

    assert_eq!(FsErrorKind::ProviderUnavailable, error.kind());
    assert!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<ProviderSelectionError>())
            .is_some(),
        "selection failures should retain ProviderSelectionError as source",
    );
}

#[test]
fn test_registry_maps_provider_create_errors() {
    let unavailable_registry = registry_with(FailingCreateProvider {
        descriptor: failing_descriptor("offline"),
        error: ProviderError::unavailable("offline"),
    });
    let unavailable_error = unavailable_registry
        .fs(&FsUri::parse("offline:///file.txt").expect("URI should parse"))
        .expect_err("unavailable provider should fail");
    assert_eq!(FsErrorKind::ProviderUnavailable, unavailable_error.kind());
    assert!(
        unavailable_error
            .source()
            .and_then(|source| source.downcast_ref::<ProviderCreationError>())
            .is_some(),
        "creation failures should retain ProviderCreationError as source",
    );

    let broken_registry = registry_with(FailingCreateProvider {
        descriptor: failing_descriptor("broken"),
        error: ProviderError::initialization_failed("broken"),
    });
    assert_eq!(
        FsErrorKind::Other,
        broken_registry
            .fs(&FsUri::parse("broken:///file.txt").expect("URI should parse"))
            .expect_err("broken provider should fail")
            .kind(),
    );
}

#[test]
fn test_empty_registry_returns_errors_for_fs_and_resource() {
    let registry = FileSystemRegistry::default();
    let missing_uri =
        FsUri::parse("missing:///file.txt").expect("URI should parse");

    assert!(registry.resource(&missing_uri).is_err());
    assert!(registry.fs(&missing_uri).is_err());
}

#[test]
fn test_registry_resolves_explicit_selection_with_its_fallback_policy() {
    let registry = FileSystemRegistry::default();
    registry
        .register(FailingCreateProvider {
            descriptor: failing_descriptor("offline"),
            error: ProviderError::unavailable("offline"),
        })
        .expect("first provider should register");
    registry
        .register(MockProvider {
            descriptor: mock_descriptor(),
            fs: MockFs::default(),
        })
        .expect("fallback provider should register");
    let selection = ProviderSelection::chain(["offline", "mock"])
        .expect("selection should be valid")
        .with_fallback_policy(FallbackPolicy::OnAbsence);
    let provider = registry
        .resolve_selected(&selection)
        .expect("selection should resolve both providers");
    let config = FileSystemConfig::new(
        FsUri::parse("mock:///file.txt").expect("URI should parse"),
    );

    assert!(
        provider
            .create_configured(&config)
            .expect("absence fallback should reach the mock provider")
            .capabilities()
            .directories
    );
}

#[test]
fn test_registry_resolves_configured_default_provider() {
    let registry = registry_with(MockProvider {
        descriptor: mock_descriptor(),
        fs: MockFs::default(),
    });
    registry.set_default_selection(
        ProviderSelection::named("mem").expect("selection should be valid"),
    );
    let provider = registry
        .resolve()
        .expect("configured default should resolve");
    let config = FileSystemConfig::new(
        FsUri::parse("mem:///file.txt").expect("URI should parse"),
    );

    assert!(
        provider
            .create_configured(&config)
            .expect("default provider should create a filesystem")
            .capabilities()
            .directories
    );
}

fn registry_with<P>(provider: P) -> FileSystemRegistry
where
    P: qubit_spi::ProviderDefinition<FileSystemSpec>,
{
    let registry = FileSystemRegistry::default();
    registry
        .register(provider)
        .expect("provider should register");
    registry
}

fn mock_descriptor() -> ProviderDescriptor {
    ProviderDescriptor::new(ProviderId::new("mock").expect("valid provider ID"))
        .with_aliases(["mem"])
        .expect("valid aliases")
}

fn failing_descriptor(id: &str) -> ProviderDescriptor {
    ProviderDescriptor::new(ProviderId::new(id).expect("valid provider ID"))
}
