#![allow(async_fn_in_trait)]
use log::{debug, error, info, warn};
use std::{fmt::Debug, process::Command, sync::Arc, time::Duration};
use tokio::{signal, time::sleep};

pub mod runtime;

/// application flow to hanlde application lifecycle
pub trait Appflow: 'static + Sized {
    async fn cleanup(self: Arc<Self>) {}
    async fn restart(self: Arc<Self>) {
        info!("Restarting application...");
        info!("Cleaning Up process");
        self.cleanup().await;

        let current_exe = std::env::current_exe().unwrap();
        let args = std::env::args().skip(1); // Pass arguments
                                             //
        debug!("Checking new executable accessibility...");
        let retry_count = 100;
        for _ in 0..retry_count {
            if current_exe.exists() {
                debug!("executable is ready.");
                break;
            }
            sleep(Duration::from_millis(100)).await; // Small delay to let the OS settle
        }

        if let Err(e) = Command::new(&current_exe).args(args).spawn() {
            error!("Failed to restart the program: {e}, path : {current_exe:?}");
        }

        // Exit the current process
        std::process::exit(0);
    }
    /// use this to be main wheel, the one that lives forever
    async fn main_process(self: Arc<Self>);

    /// must be on tokio runtime
    async fn init(self) {
        debug!("Initializing application...");

        let s = Arc::new(self);
        let s_clone = s.clone();

        tokio::select! {
            _ = signal::ctrl_c() => {
                s_clone.cleanup().await;
            }
            _ = s.main_process() => {},
        }
    }
}

pub trait AppResult<T, E> {
    /// log error
    fn log(self) -> Result<T, E>;
    /// log eroor as warn, make sure its false possitive
    fn log_warn(self) -> Result<T, E>;
}

impl<T, E> AppResult<T, E> for Result<T, E>
where
    E: Debug,
{
    fn log(self) -> Result<T, E> {
        match &self {
            Ok(_) => {}
            Err(e) => {
                // this sould be handled gracefully by my logger
                error!("{e:?}");
            }
        }
        self
    }
    fn log_warn(self) -> Result<T, E> {
        match &self {
            Ok(_) => {}
            Err(e) => {
                // this sould be handled gracefully by my logger
                warn!("{e:?}");
            }
        }
        self
    }
}
