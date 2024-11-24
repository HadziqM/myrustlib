#![allow(unused)]

use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::Local;
pub use log;
use std::io::Write;

/// Logger for displaying log, can use file to write log there
/// can use webhook to print error and wrning into discord
#[derive(Clone)]
pub struct Mylogger {
    webhook_url: Option<String>,
    tag: Option<String>,
    path: String,
    file: Option<Arc<Mutex<File>>>,
    exception: Vec<String>,
}

impl Default for Mylogger {
    fn default() -> Self {
        let name = format!("{}.log", env!("CARGO_PKG_NAME"));
        Self {
            webhook_url: None,
            tag: None,
            path: name,
            file: None,
            exception: vec![
                "tokio".to_string(),
                "reqwest".to_string(),
                "hyper_utils".to_string(),
                "tracing".to_string(),
            ],
        }
    }
}

fn tags(id: impl ToString) -> String {
    format!("<@{}>", id.to_string())
}

fn timest(ts: i64) -> String {
    format!("<t:{ts}:f>")
}

impl Mylogger {
    #[cfg(feature = "discord")]
    pub fn webhook_url(url: impl ToString, tag: impl ToString) -> Self {
        Self {
            webhook_url: Some(url.to_string()),
            tag: Some(tag.to_string()),
            ..Default::default()
        }
    }

    pub fn add_exception(mut self, ex: impl ToString) -> Self {
        self.exception.push(ex.to_string());
        self
    }

    pub fn with_file(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let file = Some(
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .ok()
                .map(Mutex::new)
                .map(Arc::new)
                .expect("cant open file"),
        );
        Self {
            path: path.to_string_lossy().to_string(),
            file,
            ..Default::default()
        }
    }

    pub fn set_file_logger(mut self, path: impl AsRef<Path>) -> Self {
        let file = Some(
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .ok()
                .map(Mutex::new)
                .map(Arc::new)
                .expect("cant open file"),
        );
        self.file = file;
        self
    }
    pub fn init(self) {
        #[cfg(debug_assertions)]
        std::env::set_var("ALLOWED_PRINT_DEBUG", "1");
        log::set_boxed_logger(Box::new(self))
            .map(|()| log::set_max_level(log::LevelFilter::Debug))
            .ok();
    }

    #[cfg(feature = "discord")]
    pub async fn send_message(&self, message: &str) {
        use reqwest::Client;
        use serde_json::json;

        let client = Client::new();
        if let Some(url) = &self.webhook_url {
            client
                .post(url)
                .json(&json!({ "content": message }))
                .send()
                .await
                .ok();
        }
    }
}

impl log::Log for Mylogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        if !self.exception.iter().any(|p| metadata.target().contains(p)) {
            if let Ok(x) = std::env::var("ALLOWED_PRINT_DEBUG") {
                if x == "1" {
                    return metadata.level() <= log::Level::Debug;
                }
            }
            metadata.level() <= log::Level::Info
        }
        false
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let now = Local::now();
            let timestamp = now.format("%Y-%m-%d %H:%M:%S").to_string();
            let ts = now.timestamp();
            let file = record.file().unwrap_or("unknown");
            let line = record.line().unwrap_or(0);
            //
            let print = format!(
                "[{}] [{}] - [{}] [{}:{}] - {}",
                timestamp,
                record.level(),
                record.target(),
                file,
                line,
                record.args()
            );
            println!("{}", print);
            #[cfg(feature = "discord")]
            {
                use log::Level;
                if record.level() <= Level::Info {
                    let s = self.clone();
                    let mut print = print.clone();
                    print = print.replace(&timestamp, &timest(ts));
                    if record.level() == Level::Error {
                        print = format!("{print} {}", tags(self.tag.clone().unwrap_or_default()));
                    }
                    tokio::spawn(async move { s.send_message(&print).await });
                }
            }
            if let Some(file) = &self.file {
                let mut f = file.lock().unwrap();
                writeln!(*f, "{print}").ok();
            }
        }
    }

    fn flush(&self) {}
}

#[cfg(not(feature = "discord"))]
#[test]
fn name() {
    Mylogger::default().init();

    log::debug!("hello debug");
    log::info!("hello info");
    log::warn!("hello warn");
    log::error!("hello error");
}

#[cfg(feature = "discord")]
#[tokio::test]
async fn name_log() {
    use std::time::Duration;
    use tokio::time::sleep;

    Mylogger::webhook_url("https://discord.com/api/webhooks/1303970772330479677/sro4acV0VvNyY47hxbqhgRb7VpN2Y4UUBbPKbMGTfmEjksIIbhoYS4S4Aj4r7-5sfe0c","455622761168109569")
        .init();

    println!("hello world");
    log::debug!("hello debug");
    log::info!("hello info");
    log::warn!("hello warn");
    log::error!("hello error");

    sleep(Duration::from_secs(10)).await;
}
