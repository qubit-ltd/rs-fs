use qubit_fs::{
    FileSystemRegistry,
    FileSystemSpec,
    FsErrorKind,
    FsUri,
};
use qubit_spi::error::ProviderError;
use qubit_spi::{
    ProviderDescriptor,
    ProviderId,
    ProviderRegistry,
};

use crate::common::{
    FailingCreateProvider,
    MockFs,
    MockProvider,
};

#[test]
fn test_registry_registers_provider_and_resolves_alias() {
    let fs = MockFs::default();
    let registry = registry_with(mock_descriptor(), MockProvider { fs });

    assert_eq!(vec!["mock"], registry.provider_ids());
    let uri = FsUri::parse("mem:///file.txt").expect("URI should parse");
    let opened = registry.fs(&uri).expect("alias should resolve");
    assert!(opened.capabilities().directories);
}

#[test]
fn test_registry_returns_error_for_missing_provider() {
    let registry = registry_with(
        mock_descriptor(),
        MockProvider {
            fs: MockFs::default(),
        },
    );

    let missing_uri =
        FsUri::parse("missing:///file.txt").expect("URI should parse");
    assert!(registry.fs(&missing_uri).is_err());
}

#[test]
fn test_registry_maps_provider_create_errors() {
    let unavailable_registry = registry_with(
        failing_descriptor("offline"),
        FailingCreateProvider {
            error: ProviderError::unavailable("offline"),
        },
    );
    assert_eq!(
        FsErrorKind::ProviderUnavailable,
        unavailable_registry
            .fs(&FsUri::parse("offline:///file.txt").expect("URI should parse"))
            .expect_err("unavailable provider should fail")
            .kind(),
    );

    let broken_registry = registry_with(
        failing_descriptor("broken"),
        FailingCreateProvider {
            error: ProviderError::initialization_failed("broken"),
        },
    );
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
    let registry = FileSystemRegistry::new(ProviderRegistry::builder().build());
    let missing_uri =
        FsUri::parse("missing:///file.txt").expect("URI should parse");

    assert!(registry.resource(&missing_uri).is_err());
    assert!(registry.fs(&missing_uri).is_err());
}

#[test]
fn test_registry_uses_explicit_resolver_with_arc_output() {
    let mut providers = ProviderRegistry::<FileSystemSpec>::builder();
    providers
        .register(
            ProviderDescriptor::new(
                ProviderId::new("mock").expect("valid provider ID"),
            )
            .with_aliases(["mem"])
            .expect("valid aliases"),
            MockProvider {
                fs: MockFs::default(),
            },
        )
        .expect("provider should register");
    let registry = FileSystemRegistry::new(providers.build());

    let uri = FsUri::parse("mem:///file.txt").expect("URI should parse");
    assert!(
        registry
            .fs(&uri)
            .expect("alias should resolve")
            .capabilities()
            .directories
    );
}

fn registry_with<P>(
    descriptor: ProviderDescriptor,
    provider: P,
) -> FileSystemRegistry
where
    P: qubit_spi::ServiceProvider<FileSystemSpec>,
{
    let mut providers = ProviderRegistry::<FileSystemSpec>::builder();
    providers
        .register(descriptor, provider)
        .expect("provider should register");
    FileSystemRegistry::new(providers.build())
}

fn mock_descriptor() -> ProviderDescriptor {
    ProviderDescriptor::new(ProviderId::new("mock").expect("valid provider ID"))
        .with_aliases(["mem"])
        .expect("valid aliases")
}

fn failing_descriptor(id: &str) -> ProviderDescriptor {
    ProviderDescriptor::new(ProviderId::new(id).expect("valid provider ID"))
}
