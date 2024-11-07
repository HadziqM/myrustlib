#![allow(unused)]

use log::{debug, error};
use std::path::{Path, PathBuf};

/// generelize patn for system app
#[derive(Clone, Debug)]
pub struct Sysdir {
    debug: bool,
    app_name: String,
    path: Option<PathBuf>,
    file: Option<PathBuf>,
}

impl Default for Sysdir {
    fn default() -> Self {
        Self {
            debug: false,
            app_name: env!("CARGO_PKG_NAME").to_string(),
            path: None,
            file: None,
        }
    }
}

impl From<Sysdir> for PathBuf {
    fn from(s: Sysdir) -> Self {
        s.path.unwrap_or_default()
    }
}

impl AsRef<Path> for Sysdir {
    fn as_ref(&self) -> &Path {
        self.path.as_deref().unwrap_or_else(|| Path::new(""))
    }
}

impl Sysdir {
    pub fn custom_name(app_name: impl ToString) -> Self {
        Self {
            app_name: app_name.to_string(),
            ..Default::default()
        }
    }

    /// set it so when debug doesnt use system path
    /// on debug assertation still use current path
    /// Default: false
    pub fn set_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    fn _add_name(&self, sys: Option<PathBuf>) -> PathBuf {
        sys.unwrap_or(PathBuf::from(".")).join(&self.app_name)
    }
    fn path(&self, file: impl AsRef<Path>, _sys: Option<PathBuf>) -> Self {
        let mut x = self.clone();
        x.file = Some(file.as_ref().to_path_buf());
        x.path = Some(Path::new(".").join(file.as_ref()));
        if self.debug {
            return x;
        }
        #[cfg(not(debug_assertions))]
        {
            x.path = Some(self._add_name(_sys).join(file.as_ref()));
        }

        debug!("Path generated: {:?}", x.path);
        x
    }

    pub fn execute_dir(&self) -> PathBuf {
        if let Some(p) = &self.path {
            if let Some(parent) = p.parent() {
                if let Some(f) = &self.file {
                    if !parent.exists() {
                        debug!("Path {p:?} doesnt exist, creating");
                        if std::fs::create_dir_all(parent).is_err() {
                            error!(" Cant create directory {p:?}");
                            return Path::new(".").join(f);
                        }
                    }
                    return p.clone();
                }
            }
        }
        PathBuf::from("")
    }
    pub fn config_dir(&self, file: impl AsRef<Path>) -> Self {
        self.path(file, dirs::config_dir())
    }
    pub fn log_dir(&self, file: impl AsRef<Path>) -> Self {
        self.path(Path::new("logs").join(file.as_ref()), dirs::config_dir())
    }
    pub fn assets_dir(&self, file: impl AsRef<Path>) -> Self {
        self.path(Path::new("assets").join(file.as_ref()), dirs::config_dir())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name() {
        let x = Sysdir::default();
        let z = PathBuf::from(x.assets_dir("asdas.ix"));

        println!("{z:?}",);
    }
}
