use qubit_fs::FileSystem;
use qubit_fs::Path;
use qubit_fs::copy::CopyFailure;
use qubit_fs::copy::CopyOptions;
use qubit_fs::copy::CopyOutcome;
use qubit_fs::error::FsError;

#[derive(Debug)]
pub struct CopyRecovery {
    pub failure: CopyFailure,
    pub cleanup_error: Option<FsError>,
}

pub fn copy_report(filesystem: &FileSystem, source: &Path, target: &Path) -> Result<CopyOutcome, CopyRecovery> {
    match filesystem.copy(source, target, CopyOptions::default()) {
        Ok(outcome) => Ok(outcome),
        Err(mut failure) => {
            // Abort handles the retained session; it does not promise target rollback.
            let cleanup_error = match failure.writer_mut() {
                Some(writer) => writer.abort().err(),
                None => None,
            };
            // Preserve the publication facts and writer even when cleanup fails.
            Err(CopyRecovery { failure, cleanup_error })
        }
    }
}
