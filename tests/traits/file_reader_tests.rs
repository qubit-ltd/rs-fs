use std::io::{
    Cursor,
    Read,
};

use qubit_fs::FileReader;

#[test]
fn test_default_reader_metadata_is_none_and_reader_still_reads() {
    let mut cursor = Cursor::new(b"abc".to_vec());

    assert!(FileReader::metadata(&cursor).is_none());
    let mut buffer = Vec::new();
    cursor.read_to_end(&mut buffer).expect("cursor should read");
    assert_eq!(b"abc".to_vec(), buffer);
}
