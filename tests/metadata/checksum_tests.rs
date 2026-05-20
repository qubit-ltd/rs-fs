use qubit_fs::{
    Checksum,
    ChecksumAlgorithm,
};

#[test]
fn test_checksum_new_stores_algorithm_and_value() {
    let checksum = Checksum::new(ChecksumAlgorithm::Sha256, "abc");

    assert_eq!(ChecksumAlgorithm::Sha256, checksum.algorithm);
    assert_eq!("abc", checksum.value);
}
