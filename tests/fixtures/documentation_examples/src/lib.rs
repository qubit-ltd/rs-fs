//! Executable guide examples; this fixture is separate from production
//! dependencies.
pub mod async_recovery;
pub mod sync_recovery;

#[cfg(test)]
#[path = "../../../common/async_recording_spi.rs"]
mod async_recording_spi;
#[cfg(test)]
#[path = "../../../common/poll_support.rs"]
mod poll_support;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod sync_tests;
