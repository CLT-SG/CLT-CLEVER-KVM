//! Dedicated Input Service Thread
//!
//! Follows the same architecture as VideoService, AudioService, and CursorService:
//! one dedicated OS thread for handling input events.
//!
//! This is necessary because InputHandler contains Enigo which uses platform-specific
//! resources (like CGEventSource on macOS) that are not Send-safe, which prevents
//! using InputHandler directly across async await boundaries.
//!
//! Key design:
//! - Runs on a dedicated std::thread (not async/tokio)
//! - Receives InputEvent commands via crossbeam channel
//! - Owns the InputHandler which contains the Enigo instance
//! - Thread-safe stop signal via AtomicBool

#![allow(dead_code)]

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender, bounded};
use log::{debug, error, info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::core::{InputHandler, InputEvent};

/// Configuration for the input service
#[derive(Debug, Clone)]
pub struct InputServiceConfig {
    /// Maximum queue size for pending input events
    pub max_queue_size: usize,
}

impl Default for InputServiceConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 256,
        }
    }
}

/// The input service manages a dedicated thread for processing input events
pub struct InputService {
    /// Thread handle
    thread: Option<thread::JoinHandle<()>>,
    /// Stop signal
    running: Arc<AtomicBool>,
    /// Channel to send input events to the service
    input_tx: Sender<InputEvent>,
}

impl InputService {
    /// Start the input service on a dedicated thread
    pub fn start(config: InputServiceConfig) -> Result<Self> {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        // Channel for receiving input events
        let (input_tx, input_rx) = bounded::<InputEvent>(config.max_queue_size);

        // Spawn the dedicated input thread
        let thread = thread::Builder::new()
            .name("input-service".to_string())
            .spawn(move || {
                Self::input_loop(running_clone, input_rx);
            })?;

        info!("Input service started on dedicated thread");

        Ok(Self {
            thread: Some(thread),
            running,
            input_tx,
        })
    }

    /// Get a sender to send input events to the service
    pub fn input_tx(&self) -> &Sender<InputEvent> {
        &self.input_tx
    }

    /// Clone the input sender for use in other contexts
    pub fn clone_input_tx(&self) -> Sender<InputEvent> {
        self.input_tx.clone()
    }

    /// Stop the input service
    pub fn stop(&mut self) {
        info!("Stopping input service...");
        self.running.store(false, Ordering::SeqCst);

        // Take ownership of the thread handle and join
        if let Some(handle) = self.thread.take() {
            // Give the thread a moment to notice the stop signal
            thread::sleep(Duration::from_millis(50));
            
            if let Err(e) = handle.join() {
                error!("Failed to join input service thread: {:?}", e);
            } else {
                info!("Input service stopped");
            }
        }
    }

    /// Check if the service is still running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// The main input processing loop
    fn input_loop(running: Arc<AtomicBool>, input_rx: Receiver<InputEvent>) {
        // Create the InputHandler on this thread
        // This ensures all platform-specific resources are owned by this thread
        let mut input_handler = InputHandler::new();
        
        info!("Input loop started");

        while running.load(Ordering::SeqCst) {
            // Use recv_timeout to periodically check the running flag
            match input_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    if let Err(e) = input_handler.handle_event(event) {
                        debug!("Input event error: {}", e);
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // No event, just continue and check running flag
                    continue;
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    info!("Input channel disconnected, stopping input loop");
                    break;
                }
            }
        }

        info!("Input loop exited");
    }
}

impl Drop for InputService {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Wrapper to send input events to the service
/// This provides a convenient interface for the async connection handler
pub struct InputSender {
    tx: Sender<InputEvent>,
}

impl InputSender {
    pub fn new(tx: Sender<InputEvent>) -> Self {
        Self { tx }
    }

    /// Send an input event to the service (non-blocking)
    pub fn send(&self, event: InputEvent) -> Result<(), crossbeam_channel::SendError<InputEvent>> {
        self.tx.send(event)
    }

    /// Try to send an input event without blocking
    pub fn try_send(&self, event: InputEvent) -> Result<(), crossbeam_channel::TrySendError<InputEvent>> {
        self.tx.try_send(event)
    }
}

impl Clone for InputSender {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}
