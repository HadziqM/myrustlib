use super::AppResult;
use indexmap::IndexMap;
use log::{debug, error};
use std::sync::Arc;
use thiserror::Error;
use tokio::{
    process::{Child, Command},
    sync::RwLock,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProcessStatus {
    #[default]
    Running,
    Stopped,
    Restarting,
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
    SubProcess(#[from] tokio::io::Error),
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
    pub async fn add_process(&self, mut app: AppProcess) -> AppRuntimeResult<()> {
        debug!("Adding Process {}", app.id);

        let mut child = Command::new(app.command.clone())
            .args(app.args.clone())
            .spawn()
            .log()?;

        app.process = Some(child);
        let id = app.id.clone();

        let mut process = self.apps.write().await;
        process.insert(app.id.clone(), app);
        debug!("Added Process {id} to runtime");
        Ok(())
    }
    pub async fn start_batch(&self, apps: Vec<AppProcess>) -> AppRuntimeResult<()> {
        for app in apps {
            self.add_process(app).await?;
        }
        Ok(())
    }

    pub async fn restart_process(&self, id: impl AsRef<str>) -> AppRuntimeResult<()> {
        let id = id.as_ref();
        let mut apps = self.apps.write().await;
        if let Some(app) = apps.get_mut(id) {
            if app.status == ProcessStatus::Running {
                self.stop_process(app.id.clone()).await?;
            }
            let mut child = Command::new(app.command.clone())
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

    pub async fn stop_process(&self, id: impl AsRef<str>) -> AppRuntimeResult<()> {
        let id = id.as_ref();
        let mut apps = self.apps.write().await;
        if let Some(app) = apps.get_mut(id) {
            if app.status == ProcessStatus::Running {
                if let Some(process) = &mut app.process {
                    process.kill().await.log()?;
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

    pub async fn check_status(&self, id: impl AsRef<str>) -> AppRuntimeResult<ProcessStatus> {
        let id = id.as_ref();
        let apps = self.apps.read().await;
        if let Some(app) = apps.get(id) {
            return Ok(app.status.clone());
        }
        error!("Process {id} not found");
        Err(AppError::NotFound(id.to_string()))
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
}
