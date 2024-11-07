#![allow(unused, async_fn_in_trait)]
use log::{debug, error, info};
use std::{fmt::Debug, process::Command};
use thiserror::Error;
use tokio::signal;

pub mod runtime;

/// application flow to hanlde application lifecycle
pub trait Appflow: Send + Sync + Clone + 'static {
    async fn cleanup(&self);
    async fn restart(&self) {
        info!("Restarting application...");
        info!("Cleaning Up process");
        self.cleanup().await;

        let current_exe = std::env::current_exe().unwrap();
        let args = std::env::args().skip(1); // Pass arguments

        if let Err(e) = Command::new(current_exe).args(args).spawn() {
            error!("Failed to restart the program: {}", e);
        }

        // Exit the current process
        std::process::exit(0);
    }
    /// use this to be main wheel, the one that lives forever
    async fn main_process(&self);

    /// must be on tokio runtime
    async fn init(self) {
        debug!("Initializing application...");

        tokio::select! {
            _ = signal::ctrl_c() => {
                self.cleanup().await;
            }
            _ = self.main_process() => {},
        }
    }
}

pub trait AppResult<T, E> {
    fn log(self) -> Result<T, E>;
}

impl<T, E> AppResult<T, E> for Result<T, E>
where
    E: Debug,
{
    fn log(self) -> Result<T, E> {
        match &self {
            Ok(_) => {}
            Err(e) => {
                /// this sould be handled gracefully by my logger
                error!("{e:?}");
            }
        }
        self
    }
}
