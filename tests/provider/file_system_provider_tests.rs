use std::sync::Arc;

use qubit_fs::FileSystemProvider;

use crate::common::{
    MockFs,
    MockProvider,
};

#[test]
fn test_file_system_provider_alias_accepts_provider_trait_objects() {
    let provider: Arc<FileSystemProvider> = Arc::new(MockProvider {
        fs: MockFs::default(),
    });

    assert_eq!(1, Arc::strong_count(&provider));
}
