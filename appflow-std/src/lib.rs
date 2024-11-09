use log::{debug, error, info};
use std::{
    fmt::Debug,
    process::Command,
    sync::{mpsc, Arc},
    thread,
};

pub mod runtime;

/// application flow to hanlde application lifecycle
/// Using std instead of tokio
pub trait Appflow: Sync + Send + Sized + 'static {
    /// clean up process, default to doesnt do anything
    fn cleanup(&self) {}
    /// restart application
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
    /// therror will be logged and caught automatically
    fn main_process(&self) -> Result<(), Box<dyn std::error::Error>>;

    /// Initialize the application
    fn init(self) {
        debug!("Initializing application...");

        let (tx, rx) = mpsc::channel();

        let m = Arc::new(self);
        let m_tx = tx.clone();
        let m_clone = m.clone();

        thread::spawn(move || {
            info!("Starting main process...");
            m_clone
                .main_process()
                .expect("Failed to start the main process");
            let _ = m_tx.send(0);
        });

        ctrlc::set_handler(move || {
            info!("SIGINT received, shutting down...");
            let _ = tx.send(0);
        })
        .ok();

        // witing either process to exit
        match rx.recv() {
            Ok(_) => {
                info!("Attemp to shutdown gracefully.....");
                m.cleanup();
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
