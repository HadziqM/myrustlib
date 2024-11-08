use log::{debug, error, info};
use std::{fmt::Debug, process::Command, sync::mpsc, thread};

pub mod runtime;

/// application flow to hanlde application lifecycle
/// Using std instead of tokio
pub trait Appflow: Sync + Send + Clone + 'static {
    fn cleanup(&self);
    fn restart(&self) {
        info!("Restarting application...");
        info!("Cleaning Up process");
        self.cleanup();

        let current_exe = std::env::current_exe().unwrap();
        let args = std::env::args().skip(1); // Pass arguments

        if let Err(e) = Command::new(current_exe).args(args).spawn() {
            error!("Failed to restart the program: {}", e);
        }

        // Exit the current process
        std::process::exit(0);
    }
    /// use this to be main wheel, the one that lives forever
    fn main_process(&self);

    /// must be on tokio runtime
    fn init(self) {
        debug!("Initializing application...");

        let (tx, rx) = mpsc::channel();

        let m = self.clone();
        let m_tx = tx.clone();

        thread::spawn(move || {
            m.main_process();
            let _ = m_tx.send(0);
        });

        ctrlc::set_handler(move || {
            info!("Ctrl+C received, shutting down...");
            let _ = tx.send(0);
        })
        .ok();

        // witing either process to exit
        match rx.recv() {
            Ok(_) => {
                info!("Attemp to shutdown gracefully.....");
                self.cleanup();
                info!("Application has been shutdown");
            }
            Err(e) => error!("{:?}", e),
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
                // this sould be handled gracefully by my logger
                error!("{e:?}");
            }
        }
        self
    }
}
