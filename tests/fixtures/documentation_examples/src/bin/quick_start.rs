// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::Path;
use qubit_fs::copy::CopyExecutionRoute;
use qubit_fs::copy::CopyOptions;
use qubit_fs::directory::ListScope;
use qubit_fs::read::ReadOptions;
use qubit_fs::read::PrefixReadTermination;
use qubit_fs::write::WriteOptions;
use qubit_fs_local::LocalFileSystems;
use qubit_fs_local::LocalResourcePolicy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let policy = LocalResourcePolicy::standard();
    let filesystem = LocalFileSystems::rooted(directory.path(), policy)?;
    let path = Path::parse("/report.txt")?;
    filesystem.write_all(&path, b"report ready", WriteOptions::default())?;
    let bytes = filesystem.read_all(&path, ReadOptions::default(), 1024)?;
    assert_eq!(b"report ready", bytes.as_slice());
    let prefix = filesystem.read_prefix(&path, ReadOptions::default(), 6)?;
    assert_eq!(prefix.bytes(), b"report");
    assert_eq!(prefix.termination(), PrefixReadTermination::LimitReached);
    let target = Path::parse("/report-copy.txt")?;
    let assessment = filesystem.assess_copy(&path, &target, &CopyOptions::default())?;
    assert_eq!(assessment.route(), CopyExecutionRoute::ProviderThenStream);
    assert_eq!(assessment.fallback_rejection(), None);
    let scope = ListScope::Path(Path::root());
    let mut entries = filesystem.list(&scope, Default::default())?;
    assert_eq!(entries.next_entry()?.expect("published report").path, path);
    assert!(entries.next_entry()?.is_none());
    println!("{}", String::from_utf8(bytes)?);
    Ok(())
}
