// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemId,
    FileSystemInfo,
    FileSystemProperties,
    PathSemantics,
};
use qubit_spi::ProviderId;

#[derive(Debug)]
struct Properties {
    info: FileSystemInfo,
    capabilities: FileSystemCapabilities,
}

impl FileSystemProperties for Properties {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        self.capabilities
    }
}

#[test]
fn common_properties_are_object_safe_local_snapshots() {
    let properties: Box<dyn FileSystemProperties> = Box::new(Properties {
        info: FileSystemInfo::new(
            FileSystemId::new("properties-instance").expect("id should parse"),
            ProviderId::new("mock").expect("provider id should parse"),
            PathSemantics::Hierarchical,
        ),
        capabilities: FileSystemCapabilities::default()
            .with(FileSystemCapability::Stat),
    });

    assert_eq!("properties-instance", properties.info().id().as_str());
    assert!(
        properties
            .capabilities()
            .contains(FileSystemCapability::Stat)
    );
}
