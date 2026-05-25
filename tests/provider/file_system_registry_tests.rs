use std::sync::Arc;

use qubit_fs::{
    FileSystemRegistry,
    FileSystemSpec,
    FsErrorKind,
    FsUri,
};
use qubit_spi::{
    ProviderCreateError,
    ProviderFailure,
    ProviderRegistryError,
    ServiceProvider,
};

use crate::common::{
    DescriptorErrorProvider,
    FailingCreateProvider,
    MockFs,
    MockProvider,
    provider_name,
};

#[test]
fn test_registry_registers_provider_and_resolves_alias() {
    let fs = MockFs::default();
    let mut registry = FileSystemRegistry::new();

    registry
        .register(MockProvider { fs })
        .expect("provider should register");

    assert_eq!(vec!["mock"], registry.provider_names());
    let uri = FsUri::parse("mem:///file.txt").expect("URI should parse");
    let opened = registry.fs(&uri).expect("alias should resolve");
    assert!(opened.capabilities().directories);
}

#[test]
fn test_registry_returns_error_for_missing_provider() {
    let mut registry = FileSystemRegistry::new();
    registry
        .register(MockProvider { fs: MockFs::default() })
        .expect("provider should register");

    let missing_uri = FsUri::parse("missing:///file.txt").expect("URI should parse");
    assert!(registry.fs(&missing_uri).is_err());
}

#[test]
fn test_registry_maps_spi_descriptor_errors() {
    let descriptor_errors = vec![
        ProviderRegistryError::EmptyProviderName,
        ProviderRegistryError::InvalidProviderName {
            name: "bad name".to_owned(),
            reason: "contains whitespace".to_owned(),
        },
        ProviderRegistryError::DuplicateProviderName {
            name: provider_name("duplicate"),
        },
        ProviderRegistryError::DuplicateProviderCandidate {
            name: provider_name("duplicate"),
        },
        ProviderRegistryError::UnknownProvider {
            name: provider_name("missing"),
        },
        ProviderRegistryError::ProviderUnavailable {
            name: provider_name("offline"),
            source: ProviderCreateError::unavailable("offline"),
        },
        ProviderRegistryError::ProviderCreate {
            name: provider_name("broken"),
            source: ProviderCreateError::failed("broken"),
        },
        ProviderRegistryError::NoAvailableProvider {
            failures: vec![ProviderFailure::unknown("missing").expect("failure should be valid")],
        },
        ProviderRegistryError::EmptyRegistry,
    ];

    for error in descriptor_errors {
        let mut registry = FileSystemRegistry::new();
        let mapped = registry
            .register(DescriptorErrorProvider { error })
            .expect_err("descriptor error should be mapped");
        assert!(matches!(
            mapped.kind(),
            FsErrorKind::InvalidPath | FsErrorKind::ProviderUnavailable | FsErrorKind::Other,
        ));
    }
}

#[test]
fn test_register_shared_accepts_arc_provider() {
    let mut registry = FileSystemRegistry::new();
    let shared: Arc<dyn ServiceProvider<FileSystemSpec>> = Arc::new(MockProvider { fs: MockFs::default() });

    registry
        .register_shared(shared)
        .expect("shared provider should register");

    assert_eq!(vec!["mock"], registry.provider_names());
}

#[test]
fn test_registry_maps_provider_create_errors() {
    let mut unavailable_registry = FileSystemRegistry::new();
    unavailable_registry
        .register(FailingCreateProvider {
            id: "offline",
            error: ProviderCreateError::unavailable("offline"),
        })
        .expect("provider should register");
    assert_eq!(
        FsErrorKind::ProviderUnavailable,
        unavailable_registry
            .fs(&FsUri::parse("offline:///file.txt").expect("URI should parse"))
            .expect_err("unavailable provider should fail")
            .kind(),
    );

    let mut broken_registry = FileSystemRegistry::new();
    broken_registry
        .register(FailingCreateProvider {
            id: "broken",
            error: ProviderCreateError::failed("broken"),
        })
        .expect("provider should register");
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
    let registry = FileSystemRegistry::new();
    let missing_uri = FsUri::parse("missing:///file.txt").expect("URI should parse");

    assert!(registry.resource(&missing_uri).is_err());
    assert!(registry.fs(&missing_uri).is_err());
}
