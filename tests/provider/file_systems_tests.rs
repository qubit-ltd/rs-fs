use std::sync::Arc;

use qubit_fs::{
    FileSystemSpec,
    FileSystems,
    FsUri,
};
use qubit_spi::{
    ProviderCreateError,
    ServiceProvider,
};

use crate::common::{
    FailingCreateProvider,
    MockFs,
    MockProvider,
};

#[test]
fn test_file_systems_facade_resolves_fs_and_resources() {
    let fs = MockFs::default();
    FileSystems::register(MockProvider { fs })
        .expect("global provider should register");
    let shared_global: Arc<dyn ServiceProvider<FileSystemSpec>> =
        Arc::new(FailingCreateProvider {
            id: "global-shared",
            error: ProviderCreateError::failed("unused"),
        });
    FileSystems::register_shared(shared_global)
        .expect("global shared provider should register");

    let provider_names = FileSystems::provider_names();
    assert!(provider_names.iter().any(|name| name == "mock"));
    assert!(provider_names.iter().any(|name| name == "global-shared"));

    let global_uri =
        FsUri::parse("mem:///global.txt").expect("URI should parse");
    assert!(
        FileSystems::fs("mem:///global.txt")
            .expect("global fs should resolve")
            .capabilities()
            .directories,
    );
    assert!(
        FileSystems::fs_for_uri(&global_uri)
            .expect("global fs from URI should resolve")
            .capabilities()
            .directories,
    );
    assert!(
        FileSystems::fs_for_scheme("mem")
            .expect("global fs from scheme should resolve")
            .capabilities()
            .directories,
    );

    let global_resource = FileSystems::resource("mock:///global.txt")
        .expect("global resource should resolve");
    assert_eq!("/global.txt", global_resource.path().as_str());
    let global_resource_uri =
        FsUri::parse("mock:///global-from-uri.txt").expect("URI should parse");
    let global_resource = FileSystems::resource_for_uri(&global_resource_uri)
        .expect("global resource from URI should resolve");
    assert_eq!("/global-from-uri.txt", global_resource.path().as_str());
    assert!(FileSystems::resource("not a uri").is_err());
    assert!(FileSystems::fs_for_scheme("bad scheme").is_err());
}
