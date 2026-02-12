//! Common utilities and shared functionality
//! 
//! This module contains utility functions and shared code
//! that can be used across different parts of the application.

pub mod constants;
pub mod error_types;
pub mod logger;

pub use constants::*;
pub use logger::{init_logging, read_debug_log, read_error_log, clear_logs, get_log_paths};
