// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
#![cfg(feature = "async")]
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use qubit_fs::Path;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::read::AsyncFileReader;
use qubit_fs::spi::OpenedAsyncReader;
use qubit_fs::write::AsyncFileWriter;
use qubit_io::AsyncByteInput;
use qubit_io::AsyncByteOutput;
use qubit_io::AsyncInput;

struct PinnedInput(PhantomPinned);
impl AsyncInput for PinnedInput {
    type Item = u8;
    unsafe fn poll_read_unchecked(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &mut [u8],
        _: usize,
        _: usize,
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(Ok(0))
    }
}

#[test]
fn test_async_byte_boundary_accepts_pinned_provider() {
    let constructor: fn(OpenedFileInfo, Box<dyn AsyncByteInput + Send>) -> OpenedAsyncReader = OpenedAsyncReader::new;
    let info = OpenedFileInfo::new(
        FileSystemId::new("async-byte-contract").expect("valid id"),
        Path::parse("/data").expect("valid path"),
    );
    let opened = constructor(info, Box::new(PinnedInput(PhantomPinned)));
    assert_eq!(opened.info().path().as_str(), "/data");
    fn require_input<T: AsyncByteInput>() {}
    fn require_output<T: AsyncByteOutput>() {}
    require_input::<AsyncFileReader>();
    require_output::<AsyncFileWriter>();
}
