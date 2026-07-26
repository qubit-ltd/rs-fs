// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::io::Result as IoResult;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use qubit_fs::{AsyncFileReader, FileLocation, FileSystemId, FsPath, OpenedFileInfo};
use qubit_io::AsyncInput;

#[derive(Debug)]
struct ReadyInput {
    bytes: Vec<u8>,
    position: usize,
}

impl AsyncInput for ReadyInput {
    type Item = u8;

    unsafe fn poll_read_unchecked(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        let this = self.get_mut();
        let read = count.min(this.bytes.len().saturating_sub(this.position));
        output[index..index + read]
            .copy_from_slice(&this.bytes[this.position..this.position + read]);
        this.position += read;
        Poll::Ready(Ok(read))
    }
}

#[test]
fn async_file_reader_forwards_polling_and_preserves_identity() {
    let info = OpenedFileInfo::new(FileLocation::new(
        FileSystemId::new("async-instance").expect("id should parse"),
        FsPath::parse_normalized("/async.bin").expect("path should parse"),
    ));
    let mut reader = AsyncFileReader::new(
        ReadyInput {
            bytes: b"xyz".to_vec(),
            position: 0,
        },
        info,
    );
    let mut bytes = [0_u8; 3];
    let mut context = Context::from_waker(Waker::noop());

    assert!(matches!(
        Pin::new(&mut reader).poll_read(&mut context, &mut bytes),
        Poll::Ready(Ok(3)),
    ));
    assert_eq!(b"xyz", &bytes);
    assert_eq!("/async.bin", reader.info().location().path().as_str());
    assert!(!reader.is_buffered());
    assert!(format!("{reader:?}").contains("AsyncFileReader"));
}
