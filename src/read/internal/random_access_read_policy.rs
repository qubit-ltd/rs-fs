//! Shared validation for random-access opens and reads.
use crate::error::{FsError, FsErrorKind, FsOperation, FsResult};
use crate::metadata::{FileKind, FileMetadata, FileSystemLimit};
use crate::read::ReadOptions;

pub(crate) struct RandomAccessReadPolicy;
impl RandomAccessReadPolicy {
    pub(crate) fn validate_open(options: &ReadOptions) -> FsResult<()> {
        if options.offset().is_some_and(|offset| offset != 0) || options.length().is_some() {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::OpenReader,
                "random-access sessions require a complete resource",
            ));
        }
        if options.checksum() != crate::read::ChecksumPolicy::None {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::OpenReader,
                "random-access sessions do not validate checksums",
            ));
        }
        Ok(())
    }
    pub(crate) fn opened_len(metadata: Option<&FileMetadata>) -> FsResult<u64> {
        let metadata = metadata
            .ok_or_else(|| contract("provider omitted metadata for opened random-access object"))?;
        if metadata.kind() != &FileKind::File {
            return Err(contract("provider opened a non-file random-access object"));
        }
        metadata
            .len()
            .ok_or_else(|| contract("provider omitted opened random-access object length"))
    }
    pub(crate) fn validate_read(
        offset: u64,
        output_len: usize,
        opened_len: u64,
        limit: FileSystemLimit,
    ) -> FsResult<usize> {
        let requested = u64::try_from(output_len)
            .map_err(|_| invalid("buffer length cannot be represented"))?;
        offset
            .checked_add(requested)
            .ok_or_else(|| invalid("read range overflows"))?;
        if requested == 0 || offset >= opened_len {
            return Ok(0);
        }
        let count = usize::try_from((opened_len - offset).min(requested))
            .map_err(|_| invalid("read range cannot be represented"))?;
        if limit.is_exceeded_by(count as u64) {
            return Err(FsError::new(
                FsErrorKind::ResourceLimitExceeded,
                FsOperation::Read,
                "random-access read exceeds provider range limit",
            ));
        }
        Ok(count)
    }
}
fn invalid(s: &str) -> FsError {
    FsError::new(FsErrorKind::InvalidOptions, FsOperation::Read, s)
}
fn contract(s: &str) -> FsError {
    FsError::new(
        FsErrorKind::ProviderContractViolation,
        FsOperation::ValidateProviderOutcome,
        s,
    )
}
