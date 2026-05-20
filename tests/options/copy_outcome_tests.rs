use qubit_fs::{
    CopyMethod,
    CopyOutcome,
    CopyStats,
};

#[test]
fn test_copy_outcome_new_stores_stats_and_method() {
    let stats = CopyStats {
        files: 1,
        bytes: 4,
        ..Default::default()
    };
    let outcome = CopyOutcome::new(stats, CopyMethod::Mixed);

    assert_eq!(1, outcome.stats.files);
    assert_eq!(4, outcome.stats.bytes);
    assert_eq!(CopyMethod::Mixed, outcome.method);
}
