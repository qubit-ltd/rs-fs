// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous read operation implementation.

use std::future::Future;
use std::future::poll_fn;
use std::ops::ControlFlow;
use std::pin::pin;
use std::task::Context;
use std::task::Poll;

use qubit_io::AsyncInput;

use crate::AsyncFileSystem;
use crate::error::FsError;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::facade::facade_core::FacadeCore;
use crate::facade::internal::FileSystemResource;
use crate::path::Path;
use crate::read::ObservedReadError;
use crate::read::PrefixReadOutcome;
use crate::read::PrefixReadTermination;
use crate::read::ReadAllOptions;
use crate::read::ReadConsistencyPolicy;
use crate::read::ReadIoEvent;
use crate::read::ReadIoObserver;
use crate::read::ReadIoPhase;
use crate::read::ReadOptions;
use crate::read::internal::NoopReadIoObserver;
use crate::read::internal::ReadBuffer;
use crate::read::prefix_read_plan::PrefixReadPlan;
use crate::spi::ResolvedReadOptions;

/// Yields after a bounded number of successful aggregate reads.
///
/// When the budget is exhausted, this helper wakes the current task and
/// returns `Pending` once before allowing the next provider read. The budget
/// is local to one aggregate operation and is reset when that operation is
/// polled again.
///
/// # Parameters
///
/// - `reads_since_yield`: Number of successful reads since the last yield.
async fn yield_read_budget(reads_since_yield: &mut usize) {
    if *reads_since_yield < 64 {
        return;
    }
    let mut yielded = false;
    poll_fn(|cx| {
        if yielded {
            Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await;
    *reads_since_yield = 0;
}

/// Coordinates asynchronous reads through one filesystem facade.
///
/// The operation object borrows the facade so every reader uses the same
/// provider configuration and error-context policy.
pub(crate) struct AsyncReadOperation<'a> {
    /// Opens readers and enriches their errors with filesystem context.
    filesystem: &'a AsyncFileSystem,
}

impl<'a> AsyncReadOperation<'a> {
    /// Creates read operations using `filesystem` for reader creation and error
    /// context.
    ///
    /// # Parameters
    /// - `filesystem`: Facade borrowed for opening and contextualizing reads.
    ///
    /// # Returns
    /// An operation object borrowing the supplied filesystem.
    #[inline]
    #[must_use]
    pub(crate) const fn new(filesystem: &'a AsyncFileSystem) -> Self {
        Self { filesystem }
    }

    /// Reads a complete file asynchronously, rejecting content beyond
    /// `max_bytes`.
    ///
    /// The reader is consumed until end of stream. If the stream contains more
    /// than the allowed number of bytes, the returned error identifies a read
    /// budget violation; stream and buffer errors retain filesystem context.
    ///
    /// # Parameters
    /// - `path`: File to read.
    /// - `options`: Options used to open the reader.
    /// - `max_bytes`: Maximum aggregate byte count accepted.
    ///
    /// # Returns
    /// The complete file contents when they fit within `max_bytes`.
    ///
    /// # Errors
    /// Returns an error if the file cannot be opened or read, if its content
    /// exceeds `max_bytes`, or if the bounded read buffer cannot append data.
    ///
    /// # Panics
    /// Panics only if converting the read count back to `usize` fails, which
    /// would violate the invariant that the count originated from a `usize`.
    pub(crate) async fn read_all(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        self.read_all_observed(path, options, max_bytes, &mut NoopReadIoObserver)
            .await
            .map_err(|error| match error {
                ObservedReadError::Stopped => unreachable!("no-op observer cannot stop"),
                ObservedReadError::Provider(error) => error,
            })
    }

    /// Reads a complete file asynchronously while observing provider I/O.
    ///
    /// # Type Parameters
    /// - `O`: Observer that controls and records provider boundaries.
    ///
    /// # Parameters
    /// - `path`: File to read.
    /// - `options`: Options used to open the reader.
    /// - `max_bytes`: Maximum accepted byte count.
    /// - `observer`: Boundary callback and provider-poll event receiver.
    ///
    /// # Returns
    /// The complete contents when they fit within `max_bytes`.
    ///
    /// # Errors
    /// Returns an observer stop or the original filesystem error.
    pub(crate) async fn read_all_observed<O: ReadIoObserver>(
        &self,
        path: &Path,
        options: ReadOptions,
        max_bytes: usize,
        observer: &mut O,
    ) -> Result<Vec<u8>, ObservedReadError> {
        // Reject a limit without a representable overflow sentinel before
        // opening the provider object, matching the synchronous read path.
        ReadAllOptions::new(max_bytes, ReadConsistencyPolicy::BestEffort)
            .sentinel_capacity()
            .map_err(|error| self.filesystem.core().enrich(error, Some(path), FsOperation::Read))?;
        let resolved = self.filesystem.prepare_open_reader(path, ResolvedReadOptions::new(options))?;
        before_io(observer, ReadIoPhase::Open)?;
        let open = self.filesystem.dispatch_open_reader(path, resolved);
        let opened = observe_io(open, observer, ReadIoPhase::Open).await?;
        let mut reader = self.filesystem.validate_opened_reader(path, opened)?;
        let mut bytes = ReadBuffer::new(max_bytes);
        let maximum = FacadeCore::quantity_from_usize(
            max_bytes,
            FsOperation::Read,
            path,
            self.filesystem.properties().info().provider_id(),
        )?;
        let mut read_budget = FacadeCore::byte_budget(FileSystemResource::ReadBytes, maximum);
        let mut buffer = [0_u8; 8192];
        let mut reads_since_yield = 0_usize;
        loop {
            yield_read_budget(&mut reads_since_yield).await;
            before_io(observer, ReadIoPhase::Read)?;
            let remaining = read_budget.remaining();
            let read_len =
                usize::try_from(remaining.saturating_add(1)).map_or(buffer.len(), |value| value.min(buffer.len()));
            let read = observe_io(reader.read_async(&mut buffer[..read_len]), observer, ReadIoPhase::Read)
                .await
                .map_err(|error| {
                    self.filesystem.core().enrich(
                        FsError::from_stream_io(error, FsOperation::Read, path),
                        Some(path),
                        FsOperation::Read,
                    )
                })?;
            observer.on_read_bytes(read);
            if read == 0 {
                return Ok(bytes.into_vec());
            }
            let read = FacadeCore::quantity_from_usize(
                read,
                FsOperation::Read,
                path,
                self.filesystem.properties().info().provider_id(),
            )?;
            if let Err(error) = read_budget.try_consume(read) {
                return Err(ObservedReadError::Provider(FacadeCore::budget_error(
                    error,
                    FsOperation::Read,
                    path,
                    self.filesystem.properties().info().provider_id(),
                    "read exceeds maximum byte count",
                )));
            }
            bytes
                .try_append(&buffer[..usize::try_from(read).expect("read count originated as usize")])
                .map_err(|error| self.filesystem.core().enrich(error, Some(path), FsOperation::Read))?;
            reads_since_yield += 1;
        }
    }

    /// Reads at most `max_bytes` from a file asynchronously.
    ///
    /// Reading stops when the stream ends or the byte limit is reached. The
    /// returned outcome records which condition stopped the read and includes
    /// the resolved file information and original read options.
    ///
    /// # Parameters
    /// - `path`: File whose prefix is read.
    /// - `options`: Options used to open the reader.
    /// - `max_bytes`: Maximum number of prefix bytes to accumulate.
    ///
    /// # Returns
    /// The bytes read together with the termination reason and open-time
    /// metadata.
    ///
    /// # Errors
    /// Returns an error if read options cannot be resolved, the file cannot be
    /// opened or read, or the bounded read buffer cannot append data. Errors
    /// from the stream include filesystem context.
    pub(crate) async fn read_prefix(
        &self,
        path: &Path,
        options: ReadOptions,
        max_bytes: usize,
    ) -> FsResult<PrefixReadOutcome> {
        self.read_prefix_observed(path, options, max_bytes, &mut NoopReadIoObserver)
            .await
            .map_err(|error| match error {
                ObservedReadError::Stopped => unreachable!("no-op observer cannot stop"),
                ObservedReadError::Provider(error) => error,
            })
    }

    /// Reads a prefix while observing provider I/O boundaries.
    ///
    /// # Type Parameters
    /// - `O`: Observer that controls and records provider boundaries.
    ///
    /// # Parameters
    /// - `path`: File whose prefix is read.
    /// - `options`: Options used to open the reader.
    /// - `max_bytes`: Maximum number of prefix bytes to accumulate.
    /// - `observer`: Boundary callback and provider-poll event receiver.
    ///
    /// # Returns
    /// The bounded prefix and its original request facts.
    ///
    /// # Errors
    /// Returns an observer stop or the original filesystem error.
    pub(crate) async fn read_prefix_observed<O: ReadIoObserver>(
        &self,
        path: &Path,
        options: ReadOptions,
        max_bytes: usize,
        observer: &mut O,
    ) -> Result<PrefixReadOutcome, ObservedReadError> {
        let original_options = options.clone();
        let plan = PrefixReadPlan::new(self.filesystem.properties(), path, options, max_bytes)?;
        let resolved = self.filesystem.prepare_open_reader(path, plan.into_options())?;
        before_io(observer, ReadIoPhase::Open)?;
        let open = self.filesystem.dispatch_open_reader(path, resolved);
        let opened = observe_io(open, observer, ReadIoPhase::Open).await?;
        let mut reader = self.filesystem.validate_opened_reader(path, opened)?;
        let info = reader.info().clone();
        if max_bytes == 0 {
            return Ok(PrefixReadOutcome::new(
                Vec::new(),
                info,
                original_options,
                max_bytes,
                PrefixReadTermination::LimitReached,
            ));
        }
        let mut bytes = ReadBuffer::new(max_bytes);
        let mut buffer = [0_u8; FacadeCore::PREFIX_BUFFER_SIZE];
        let mut termination = PrefixReadTermination::LimitReached;
        let mut reads_since_yield = 0_usize;
        while bytes.len() < max_bytes {
            yield_read_budget(&mut reads_since_yield).await;
            before_io(observer, ReadIoPhase::Read)?;
            let read_len = FacadeCore::next_prefix_read_len(bytes.len(), max_bytes);
            let read = observe_io(reader.read_async(&mut buffer[..read_len]), observer, ReadIoPhase::Read)
                .await
                .map_err(|error| {
                    self.filesystem.core().enrich(
                        FsError::from_stream_io(error, FsOperation::Read, path),
                        Some(path),
                        FsOperation::Read,
                    )
                })?;
            observer.on_read_bytes(read);
            if read == 0 {
                termination = PrefixReadTermination::StreamEnded;
                break;
            }
            bytes
                .try_append(&buffer[..read])
                .map_err(|error| self.filesystem.core().enrich(error, Some(path), FsOperation::Read))?;
            reads_since_yield += 1;
        }
        Ok(PrefixReadOutcome::new(
            bytes.into_vec(),
            info,
            original_options,
            max_bytes,
            termination,
        ))
    }
}

/// Checks whether the observer allows the next provider operation.
///
/// # Type Parameters
/// - `O`: Observer receiving the boundary check.
///
/// # Parameters
/// - `observer`: Observer queried before provider work begins.
/// - `phase`: Provider operation about to be performed.
///
/// # Returns
/// Success when the provider operation may begin, or the stop marker otherwise.
fn before_io<O: ReadIoObserver>(observer: &mut O, phase: ReadIoPhase) -> Result<(), ObservedReadError> {
    match observer.before_io(phase) {
        ControlFlow::Continue(()) => Ok(()),
        ControlFlow::Break(()) => Err(ObservedReadError::Stopped),
    }
}

/// Polls one provider future and reports its actual poll transitions.
///
/// # Type Parameters
/// - `F`: Provider future whose polling is observed.
/// - `O`: Observer receiving provider poll events.
///
/// # Parameters
/// - `future`: Provider operation to poll.
/// - `observer`: Event receiver for the operation.
/// - `phase`: Kind of provider operation represented by the future.
///
/// # Returns
/// The provider future's output after it becomes ready.
async fn observe_io<F, O>(future: F, observer: &mut O, phase: ReadIoPhase) -> F::Output
where
    F: Future,
    O: ReadIoObserver,
{
    let mut future = pin!(future);
    let mut started = false;
    poll_fn(|cx: &mut Context<'_>| {
        if !started {
            started = true;
            observer.on_io_event(ReadIoEvent::Started { phase });
        }
        match future.as_mut().poll(cx) {
            Poll::Ready(output) => {
                observer.on_io_event(ReadIoEvent::Ready { phase });
                Poll::Ready(output)
            }
            Poll::Pending => {
                observer.on_io_event(ReadIoEvent::Pending { phase });
                Poll::Pending
            }
        }
    })
    .await
}
