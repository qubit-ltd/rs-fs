// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FileSystemCapabilities, FileSystemCapability, FileSystemId, FileSystemInfo, FileSystemLimit,
    FileSystemLimits, FileSystemProperties, PathSemantics,
};

#[derive(Debug)]
struct Properties {
    info: FileSystemInfo,
    capabilities: FileSystemCapabilities,
    limits: FileSystemLimits,
}

impl FileSystemProperties for Properties {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        self.capabilities
    }

    fn limits(&self) -> &FileSystemLimits {
        &self.limits
    }
}

#[test]
fn common_properties_are_object_safe_local_snapshots() {
    let properties: Box<dyn FileSystemProperties> = Box::new(Properties {
        info: FileSystemInfo::new(
            FileSystemId::new("properties-instance").expect("id should parse"),
            "mock",
            PathSemantics::Hierarchical,
        ),
        capabilities: FileSystemCapabilities::default().with(FileSystemCapability::Read),
        limits: FileSystemLimits::unknown().with_max_write_bytes(FileSystemLimit::Maximum(1024)),
    });

    assert_eq!("properties-instance", properties.info().id().as_str());
    assert!(core::ptr::eq(properties.limits(), properties.limits()));
    assert_eq!(
        FileSystemLimit::Maximum(1024),
        properties.limits().max_write_bytes(),
    );
    assert!(
        properties
            .capabilities()
            .contains(FileSystemCapability::Read)
    );
}
