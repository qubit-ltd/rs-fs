// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::io::Result as IoResult;

use qubit_fs::{
    FileKind, FileLocation, FileMetadata, FileReader, FileSystemId, FsPath, FsUri, OpenedFileInfo,
};
use qubit_io::Input;

#[derive(Debug)]
struct InputOnly {
    bytes: Vec<u8>,
    position: usize,
}

impl Input for InputOnly {
    type Item = u8;

    unsafe fn read_unchecked(
        &mut self,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> IoResult<usize> {
        let available = self.bytes.len().saturating_sub(self.position);
        let read = count.min(available);
        output[index..index + read]
            .copy_from_slice(&self.bytes[self.position..self.position + read]);
        self.position += read;
        Ok(read)
    }
}

#[test]
fn concrete_file_reader_combines_an_explicit_file_identity_and_input() {
    let location = FileLocation::new(
        FileSystemId::new("mock-instance").expect("id should parse"),
        FsPath::parse_normalized("/notes.txt").expect("path should parse"),
    )
    .with_uri(FsUri::parse("mock:///notes.txt").expect("URI should parse"));
    let info = OpenedFileInfo::new(location).with_metadata(FileMetadata::new(FileKind::File));
    let mut reader = FileReader::new(
        InputOnly {
            bytes: b"abc".to_vec(),
            position: 0,
        },
        info,
    );
    let mut output = [0_u8; 3];

    assert_eq!(3, reader.read_fully(&mut output).expect("read should work"));
    assert_eq!(b"abc", &output);
    assert_eq!("/notes.txt", reader.info().location().path().as_str());
    assert_eq!(
        Some(FileKind::File),
        reader
            .info()
            .metadata()
            .map(|metadata| metadata.kind.clone()),
    );
    assert_eq!(
        Some("mock:///notes.txt"),
        reader
            .info()
            .location()
            .uri()
            .map(ToString::to_string)
            .as_deref(),
    );
    assert!(!reader.is_buffered());
    assert!(format!("{reader:?}").contains("FileReader"));
}
