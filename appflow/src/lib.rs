#![allow(async_fn_in_trait)]
use log::{debug, error, info, warn};
use std::{fmt::Debug, process::Command, sync::Arc};
use tokio::signal;

pub mod runtime;

#[cfg(feature = "update")]
mod upp {
    pub use reqwest::{
        header::{HeaderMap, HeaderValue, ACCEPT},
        StatusCode,
    };
    pub use serde::{Deserialize, Serialize};
    pub use std::{fs, os::unix::fs::PermissionsExt};
    pub use thiserror::Error;
}
#[cfg(feature = "update")]
use upp::*;

#[cfg(feature = "update")]
pub struct GithubUpdater {
    pub repo: String,
    pub owner: String,
    pub token: Option<String>,
    pub app_name: String,
}

#[cfg(feature = "update")]
#[derive(Serialize, Deserialize)]
pub struct ApiResponse {
    pub name: String,
    pub assets: Vec<ApiResponseAsset>,
}

#[cfg(feature = "update")]
#[derive(Serialize, Deserialize)]
pub struct ApiResponseAsset {
    pub name: String,
    pub size: u32,
    pub content_type: String,
    pub url: String,
}

#[cfg(feature = "update")]
#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("Error from Reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Error from Tokio filesystem: {0}")]
    IO(#[from] std::io::Error),
    #[error("Custom Error from Updater: {0}")]
    Custom(String),
}

#[cfg(feature = "update")]
impl GithubUpdater {
    pub async fn get_update_info(&self) -> Result<(ApiResponse, HeaderMap), UpdateError> {
        let client = reqwest::Client::new();

        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            self.owner, self.repo
        );

        debug!("Fetching: {}", url);

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "User-Agent",
            reqwest::header::HeaderValue::from_static("Rust-Updater"),
        );
        if let Some(token) = &self.token {
            debug!("Using token: {}", token);

            headers.insert(
                "Authorization",
                reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
            );
        }

        let x = client
            .get(url)
            .headers(headers.clone())
            .send()
            .await?
            .text()
            .await?;

        match serde_json::from_str::<ApiResponse>(&x) {
            Ok(x) => Ok((x, headers)),
            Err(e) => {
                log::error!(" the resulting data was: {x}");
                Err(UpdateError::Custom(e.to_string()))
            }
        }
    }

    pub async fn update(&self) -> Result<(), UpdateError> {
        let (update_info, header) = self.get_update_info().await?;
        update_info.update_current_exe(&self.app_name, header).await
    }
}

#[cfg(feature = "update")]
impl ApiResponse {
    pub async fn update_current_exe(
        &self,
        name_asset: impl ToString,
        mut headers: HeaderMap,
    ) -> Result<(), UpdateError> {
        let client = reqwest::Client::new();
        let x = self
            .assets
            .iter()
            .find(|&y| y.name == name_asset.to_string());
        if let Some(asset) = x {
            debug!("Found asset {}", asset.name);
            debug!("Downloading {}", asset.url);

            headers.insert(ACCEPT, HeaderValue::from_static("application/octet-stream"));

            let res = client.get(&asset.url).headers(headers).send().await?;
            if res.status() != StatusCode::OK {
                return Err(UpdateError::Custom(format!("response status: {res:?}")));
            }
            let body = res.bytes().await?;

            let body_str = std::str::from_utf8(&body).unwrap_or("<non-UTF-8 content>");
            debug!("Downloaded body: {}", body_str);

            let current_exe = std::env::current_exe().unwrap();
            let temp_exe = current_exe.with_extension("temp");

            debug!("Writing to {}", temp_exe.display());

            fs::write(&temp_exe, &body)?;
            // Replace the current executable with the new one
            //
            debug!("Replacing {}", current_exe.display());

            fs::rename(&temp_exe, &current_exe)?;

            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                fs::set_permissions(&current_exe, fs::Permissions::from_mode(0o755))?;
                return Err(Command::new(&current_exe).exec().into());
            }

            #[cfg(windows)]
            {
                let args = std::env::args().skip(1); // Pass arguments

                if let Err(e) = Command::new(&current_exe).args(args).spawn() {
                    log::error!("Failed to restart the program: {e}, path : {current_exe:?}");
                }
                return Ok(());
            }
        }
        Err(UpdateError::Custom("No asset found".to_string()))
    }
}

/// application flow to hanlde application lifecycle
pub trait Appflow: 'static + Sized {
    async fn cleanup(self: Arc<Self>) {}

    #[cfg(feature = "update")]
    fn update_config(self: Arc<Self>) -> GithubUpdater;

    #[cfg(feature = "update")]
    async fn update(self: Arc<Self>) {
        let updater = self.update_config();
        updater.update().await.unwrap();
        std::process::exit(0);
    }

    async fn restart(self: Arc<Self>) {
        info!("Restarting application...");
        info!("Cleaning Up process");
        self.cleanup().await;

        let current_exe = std::env::current_exe().unwrap();
        let args = std::env::args().skip(1); // Pass arguments
                                             //
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
