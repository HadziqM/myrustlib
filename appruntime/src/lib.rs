use indexmap::IndexMap;
use log::{debug, error, warn};
use std::{fmt::Debug, sync::Arc};
use tokio::{
    process::{Child, Command},
    sync::RwLock,
    task::spawn_blocking,
};

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
/// using tokio
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
///
type MyRuntime = Arc<RwLock<IndexMap<String, AppProcess>>>;
pub struct AppRuntime {
    pub apps: MyRuntime,
}

impl Default for AppRuntime {
    fn default() -> Self {
        Self {
            apps: Arc::new(RwLock::new(IndexMap::new())),
        }
    }
}

impl AppRuntime {
    pub async fn add_process(&self, app: AppProcess) {
        debug!("Adding Process {}", app.id);

        let id = app.id.clone();

        let mut process = self.apps.write().await;
        process.insert(app.id.clone(), app);
        debug!("Added Process {id} to runtime");
    }
    pub async fn add_process_then_run(&self, mut app: AppProcess) -> AppRuntimeResult<()> {
        debug!("Adding Process {}", app.id);

        let id = app.id.clone();
        let child = Command::new(app.command.clone())
            .args(app.args.clone())
            .spawn()
            .log()?;
        debug!("Starting Process {id}");
        app.process = Some(child);
        app.status = ProcessStatus::Running;

        let mut process = self.apps.write().await;
        process.insert(app.id.clone(), app);
        Ok(())
    }
    pub async fn add_batch(&self, apps: Vec<AppProcess>) {
        for app in apps {
            self.add_process(app).await;
        }
    }

    async fn restart(app: &mut AppProcess, id: &str) -> AppRuntimeResult<()> {
        debug!("Restarting Process {id}");
        if app.status == ProcessStatus::Running {
            if let Some(process) = &mut app.process {
                process.kill().await.log()?;
            }
        }
        let mut args = app.args.clone();
        args.push("--update".to_string());
        // run process with update flag
        Command::new(app.command.clone())
            .args(args)
            .spawn()
            .log()?
            .wait()
            .await?;

        let child = Command::new(app.command.clone())
            .args(app.args.clone())
            .spawn()
            .log()?;
        app.process = Some(child);
        app.status = ProcessStatus::Running;
        debug!("Succesfully Restarting Process {id}");
        Ok(())
    }

    async fn stop(app: &mut AppProcess, id: &str) -> AppRuntimeResult<()> {
        if app.status == ProcessStatus::Running {
            if let Some(process) = &mut app.process {
                process.kill().await.log()?;
            }
            app.status = ProcessStatus::Stopped;
            debug!("Stopped Process {id}");
        }
        Ok(())
    }

    async fn ver_update(app: &mut AppProcess, id: &str) -> AppRuntimeResult<()> {
        if app.status == ProcessStatus::Running {
            if let Some(process) = &mut app.process {
                process.kill().await.log()?;
            }
        }
        let mut args = app.args.clone();
        args.push("--update".to_string());
        // run process with update flag
        Command::new(app.command.clone())
            .args(args)
            .spawn()
            .log()?
            .wait()
            .await?;

        let child = Command::new(app.command.clone())
            .args(app.args.clone())
            .spawn()
            .log()?;
        app.process = Some(child);
        app.status = ProcessStatus::Running;
        debug!("Succesfully Restarting Process {id}");
        Ok(())
    }

    pub async fn start_all(&self) -> AppRuntimeResult<()> {
        let mut apps = self.apps.write().await;
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

    pub async fn restart_process(&self, id: impl AsRef<str>) -> AppRuntimeResult<()> {
        let id = id.as_ref();
        let mut apps = self.apps.write().await;
        if let Some(app) = apps.get_mut(id) {
            Self::restart(app, id).await
        } else {
            error!("Process {id} not found");
            Err(AppError::NotFound(id.to_string()))
        }
    }

    /// Using indexmap so the process start in order
    pub async fn restart_all(&self) -> AppRuntimeResult<()> {
        let mut apps = self.apps.write().await;
        for (id, app) in apps.iter_mut() {
            Self::restart(app, id).await?;
        }
        Ok(())
    }
    pub async fn version_update_process(&self, id: impl AsRef<str>) -> AppRuntimeResult<()> {
        let id = id.as_ref();
        debug!("Restarting Process {id}");
        let mut apps = self.apps.write().await;
        if let Some(app) = apps.get_mut(id) {
            Self::ver_update(app, id).await
        } else {
            error!("Process {id} not found");
            Err(AppError::NotFound(id.to_string()))
        }
    }

    /// Using indexmap so the process start in order
    pub async fn version_update_all(&self) -> AppRuntimeResult<()> {
        let mut apps = self.apps.write().await;
        for (id, app) in apps.iter_mut() {
            Self::ver_update(app, id).await?;
        }
        Ok(())
    }

    pub async fn stop_process(&self, id: impl AsRef<str>) -> AppRuntimeResult<()> {
        let id = id.as_ref();
        let mut apps = self.apps.write().await;
        if let Some(app) = apps.get_mut(id) {
            Self::stop(app, id).await
        } else {
            error!("Process {id} not found");
            Err(AppError::NotFound(id.to_string()))
        }
    }

    pub async fn stop_all(&self) -> AppRuntimeResult<()> {
        let mut apps = self.apps.write().await;
        for (id, app) in apps.iter_mut() {
            Self::stop(app, id).await?;
        }
        Ok(())
    }

    pub async fn check_status(&self, id: impl AsRef<str>) -> AppRuntimeResult<ProcessStatus> {
        let id = id.as_ref();
        let apps = self.apps.read().await;
        if let Some(app) = apps.get(id) {
            return Ok(app.status.clone());
        }
        error!("Process {id} not found");
        Err(AppError::NotFound(id.to_string()))
    }

    /// List id and status
    pub async fn list_status(&self) -> Vec<(String, ProcessStatus)> {
        let mut con = vec![];
        let apps = self.apps.read().await;
        for (id, process) in apps.iter() {
            con.push((id.clone(), process.status.clone()));
        }
        con
    }

    pub async fn update_status(&self) {
        let mut apps = self.apps.write().await;
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

    pub async fn wait_for_exit(&self) {
        let mut apps = self.apps.write().await;
        for app in apps.values_mut() {
            if let Some(process) = &mut app.process {
                process.wait().await.log().ok();
            }
            app.status = ProcessStatus::Stopped;
        }
    }
}

/// To stop the runtime and all it process when dropped
impl Drop for AppRuntime {
    fn drop(&mut self) {
        let y = std::mem::take(self);
        spawn_blocking(|| async move {
            if y.stop_all().await.log().is_ok() {
                debug!("Dropped the AppRuntime succesfully");
            }
        });
    }
}
