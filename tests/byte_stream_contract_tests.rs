// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::Path;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::read::FileReader;
use qubit_fs::spi::FileWriterSpi;
use qubit_fs::spi::OpenedReader;
use qubit_fs::write::FileWriter;
use qubit_io::ByteInput;
use qubit_io::ByteOutput;
use qubit_io::Input;

/// An input provider that immediately reports end of stream.
struct OnlyInput;

impl Input for OnlyInput {
    type Item = u8;

    unsafe fn read_unchecked(&mut self, _: &mut [u8], _: usize, _: usize) -> std::io::Result<usize> {
        Ok(0)
    }
}

#[test]
fn test_sync_byte_stream_contract() {
    let constructor: fn(OpenedFileInfo, Box<dyn ByteInput + Send>) -> OpenedReader = OpenedReader::new;
    let info = OpenedFileInfo::new(
        FileSystemId::new("byte-contract").expect("valid id"),
        Path::parse("/data").expect("valid path"),
    );
    let _opened = constructor(info, Box::new(OnlyInput));
    fn require_input<T: ByteInput>() {}
    fn require_output<T: ByteOutput>() {}
    fn spi_output<T: FileWriterSpi + ?Sized>(writer: &mut T) {
        fn accepts_output<O: ByteOutput + ?Sized>(_: &mut O) {}
        accepts_output(writer);
    }
    require_input::<FileReader>();
    require_output::<FileWriter>();
    let _: fn(&mut (dyn FileWriterSpi + 'static)) = spi_output::<dyn FileWriterSpi>;
}
