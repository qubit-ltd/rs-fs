// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! File read options, handles, and facade operation objects.

#[cfg(feature = "async")]
mod async_file_reader;
#[cfg(feature = "async")]
mod async_random_access_reader;
#[cfg(feature = "async")]
mod async_read_operation;
mod checksum_policy;
mod consistent_read_outcome;
mod file_reader;
mod internal;
#[cfg(feature = "async")]
mod observed_read_error;
mod prefix_read_outcome;
mod prefix_read_plan;
mod prefix_read_termination;
mod random_access_reader;
mod read_all_options;
mod read_consistency_policy;
#[cfg(feature = "async")]
mod read_io_event;
#[cfg(feature = "async")]
mod read_io_observer;
#[cfg(feature = "async")]
mod read_io_phase;
mod read_operation;
mod read_options;

#[cfg(feature = "async")]
pub use async_file_reader::AsyncFileReader;
#[cfg(feature = "async")]
pub use async_random_access_reader::AsyncRandomAccessReader;
#[cfg(feature = "async")]
pub(crate) use async_read_operation::AsyncReadOperation;
pub use checksum_policy::ChecksumPolicy;
pub use consistent_read_outcome::ConsistentReadOutcome;
pub use file_reader::FileReader;
pub(crate) use internal::RandomAccessReadPolicy;
pub(crate) use internal::read_retry_interrupted;
#[cfg(feature = "async")]
pub use observed_read_error::ObservedReadError;
pub use prefix_read_outcome::PrefixReadOutcome;
pub use prefix_read_termination::PrefixReadTermination;
pub use random_access_reader::RandomAccessReader;
pub use read_all_options::ReadAllOptions;
pub use read_consistency_policy::ReadConsistencyPolicy;
#[cfg(feature = "async")]
pub use read_io_event::ReadIoEvent;
#[cfg(feature = "async")]
pub use read_io_observer::ReadIoObserver;
#[cfg(feature = "async")]
pub use read_io_phase::ReadIoPhase;
pub(crate) use read_operation::ReadOperation;
pub use read_options::ReadOptions;
