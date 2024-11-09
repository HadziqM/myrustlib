use super::AppResult;
use indexmap::IndexMap;
use log::{debug, error};
use std::{
    process::{Child, Command},
    sync::{Arc, RwLock},
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProcessStatus {
    Running,
    #[default]
    Stopped,
}

impl std::ops::Not for ProcessStatus {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            ProcessStatus::Running => ProcessStatus::Stopped,
            ProcessStatus::Stopped => ProcessStatus::Running,
        }
    }
}

#[derive(Debug, Default)]
pub struct AppProcess {
    pub id: String,
    pub command: String,
    pub process: Option<Child>,
    pub status: ProcessStatus,
    pub args: Vec<String>,
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Failed to find the process/executable on the runtime given id: {0}")]
    NotFound(String),
    #[error("Failed to execute command : {0}")]
    SubProcess(#[from] std::io::Error),
}

pub type AppRuntimeResult<T> = Result<T, AppError>;

impl AppProcess {
    pub fn new(id: impl ToString, command: impl ToString, args: Vec<String>) -> Self {
        Self {
            id: id.to_string(),
            command: command.to_string(),
            args,
            ..Default::default()
        }
    }
}

/// To start runtime application to handle multiple process
/// can be used with UI
/// using std instead of tokio
/// ```
/// use appflow_std::runtime::{AppProcess, AppRuntime};
///
///     let data = vec![
///     AppProcess::new("agus", "hello_agus_5s.sh", vec![]),
///     AppProcess::new("agung", "hello_agung_15s.sh", vec![]),
///     AppProcess::new("andi", "hello_andi_10s.sh", vec![]),
/// ];
///
/// let runtime = AppRuntime::default();
///
/// runtime
///     .add_batch(data);
/// runtime.start_all().expect("Failed to start the runtime");
///```
pub struct AppRuntime {
    pub apps: Arc<RwLock<IndexMap<String, AppProcess>>>,
}

impl Default for AppRuntime {
    fn default() -> Self {
        Self {
            apps: Arc::new(RwLock::new(IndexMap::new())),
        }
    }
}

impl AppRuntime {
    pub fn add_process(&self, app: AppProcess) {
        debug!("Adding Process {}", app.id);

        let id = app.id.clone();

        let mut process = self.apps.write().unwrap();
        process.insert(app.id.clone(), app);
        debug!("Added Process {id} to runtime");
    }
    pub fn add_process_then_run(&self, mut app: AppProcess) -> AppRuntimeResult<()> {
        debug!("Adding Process {}", app.id);

        let id = app.id.clone();
        let child = Command::new(app.command.clone())
            .args(app.args.clone())
            .spawn()
            .log()?;
        debug!("Starting Process {id}");
        app.process = Some(child);
        app.status = ProcessStatus::Running;

        let mut process = self.apps.write().unwrap();
        process.insert(app.id.clone(), app);
        Ok(())
    }
    pub fn add_batch(&self, apps: Vec<AppProcess>) {
        for app in apps {
            self.add_process(app);
        }
    }

    pub fn start_all(&self) -> AppRuntimeResult<()> {
        let mut apps = self.apps.write().unwrap();
        for (id, app) in apps.iter_mut() {
            debug!("Starting Process {id}");
            let child = Command::new(app.command.clone())
                .args(app.args.clone())
                .spawn()
                .log()?;
            app.process = Some(child);
            app.status = ProcessStatus::Running;
        }
        Ok(())
    }

    pub fn restart_process(&self, id: impl AsRef<str>) -> AppRuntimeResult<()> {
        let id = id.as_ref();
        debug!("Restarting Process {id}");
        let mut apps = self.apps.write().unwrap();
        if let Some(app) = apps.get_mut(id) {
            if app.status == ProcessStatus::Running {
                if let Some(process) = &mut app.process {
                    process.kill().log()?;
                }
            }
            let child = Command::new(app.command.clone())
                .args(app.args.clone())
                .spawn()
                .log()?;
            app.process = Some(child);
            app.status = ProcessStatus::Running;
            debug!("Succesfully Restarting Process {id}");
            Ok(())
        } else {
            error!("Process {id} not found");
            Err(AppError::NotFound(id.to_string()))
        }
    }

    /// Using indexmap so the process start in order
    pub fn restart_all(&self) -> AppRuntimeResult<()> {
        let mut apps = self.apps.write().unwrap();
        for (id, app) in apps.iter_mut() {
            debug!("Restarting Process {id}");
            if app.status == ProcessStatus::Running {
                if let Some(process) = &mut app.process {
                    process.kill().log()?;
                }
            }
            let child = Command::new(app.command.clone())
                .args(app.args.clone())
                .spawn()
                .log()?;
            app.process = Some(child);
            app.status = ProcessStatus::Running;
            debug!("Succesfully Restarting Process {id}");
        }
        Ok(())
    }

    pub fn stop_process(&self, id: impl AsRef<str>) -> AppRuntimeResult<()> {
        let id = id.as_ref();
        let mut apps = self.apps.write().unwrap();
        if let Some(app) = apps.get_mut(id) {
            if app.status == ProcessStatus::Running {
                if let Some(process) = &mut app.process {
                    process.kill().log()?;
                }
                app.status = ProcessStatus::Stopped;
                debug!("Stopped Process {id}");
            }
            Ok(())
        } else {
            error!("Process {id} not found");
            Err(AppError::NotFound(id.to_string()))
        }
    }

    pub fn stop_all(&self) -> AppRuntimeResult<()> {
        let mut apps = self.apps.write().unwrap();
        for (id, app) in apps.iter_mut() {
            if app.status == ProcessStatus::Running {
                if let Some(process) = &mut app.process {
                    process.kill().log()?;
                }
                app.status = ProcessStatus::Stopped;
                debug!("Stopped Process {id}");
            }
        }
        Ok(())
    }

    pub fn check_status(&self, id: impl AsRef<str>) -> AppRuntimeResult<ProcessStatus> {
        let id = id.as_ref();
        let apps = self.apps.read().unwrap();
        if let Some(app) = apps.get(id) {
            return Ok(app.status.clone());
        }
        error!("Process {id} not found");
        Err(AppError::NotFound(id.to_string()))
    }

    /// List id and status
    pub fn list_status(&self) -> Vec<(String, ProcessStatus)> {
        let mut con = vec![];
        let apps = self.apps.read().unwrap();
        for (id, process) in apps.iter() {
            con.push((id.clone(), process.status.clone()));
        }
        con
    }

    pub fn update_status(&self) {
        let mut apps = self.apps.write().unwrap();
        for app in apps.values_mut() {
            if let Some(process) = &mut app.process {
                if let Ok(status) = process.try_wait() {
                    match status {
                        Some(_) => app.status = ProcessStatus::Stopped,
                        None => app.status = ProcessStatus::Running,
                    }
                    debug!("Process {} status updated", app.id);
                }
            }
        }
    }

    pub fn wait_for_exit(&self) {
        let mut apps = self.apps.write().unwrap();
        for app in apps.values_mut() {
            if let Some(process) = &mut app.process {
                process.wait().unwrap();
            }
            app.status = ProcessStatus::Stopped;
        }
    }
}

/// To stop the runtime and all it process when dropped
impl Drop for AppRuntime {
    fn drop(&mut self) {
        if self.stop_all().log().is_ok() {
            debug!("Dropped the AppRuntime succesfully");
        }
    }
}

#[cfg(test)]
mod testing {
    use macros::Wrapper;

    use super::*;
    use crate::Appflow;

    #[derive(Wrapper)]
    struct App(AppRuntime);

    impl Default for App {
        fn default() -> Self {
            let data = vec![
                AppProcess::new(
                    "hello",
                    "sh",
                    vec!["../example/sh_command/hello_5s.sh".to_string()],
                ),
                AppProcess::new(
                    "hola",
                    "sh",
                    vec!["../example/sh_command/holla_10s.sh".to_string()],
                ),
                AppProcess::new(
                    "aloha",
                    "sh",
                    vec!["../example/sh_command/aloha_15s.sh".to_string()],
                ),
            ];

            let app = AppRuntime::default();
            app.add_batch(data);

            Self(app)
        }
    }

    impl Appflow for App {
        fn main_process(&self) -> Result<(), Box<dyn std::error::Error>> {
            self.start_all()?;
            std::thread::sleep(std::time::Duration::from_secs(2));
            self.restart_all()?;
            std::thread::sleep(std::time::Duration::from_secs(6));
            self.update_status();
            println!("{:?}", self.list_status());
            self.wait_for_exit();
            self.restart_all()?;
            self.update_status();
            println!("{:?}", self.list_status());
            Ok(())
        }
    }

    #[test]
    fn name() {
        use logger::Mylogger;

        Mylogger::default().init();

        let app = App::default();
        app.init();
    }
}
