//! The application shell: start, command line, window, frame loop and cadence.
pub(crate) mod cadence;
#[cfg(test)]
mod clock_tests;
pub(crate) mod cli;
pub(crate) mod command;
pub(crate) mod debug_env;
pub(crate) mod exit;
pub(crate) mod first_run;
pub(crate) mod frame_loop;
pub(crate) mod prepare;
pub(crate) mod probe;
pub(crate) mod start_ahead;
pub(crate) mod startup;
pub(crate) mod waits;
pub(crate) mod window;
