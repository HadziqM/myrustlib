#![allow(unused)]

use log::{debug, error};
use std::{
    fmt::{Debug, Display},
    path::{Path, PathBuf},
};

/// generelize patn for system app
#[derive(Clone, Debug)]
pub struct Sysdir {
    app_name: String,
    path: PathBuf,
}

impl Default for Sysdir {
    fn default() -> Self {
        Self {
            app_name: env!("CARGO_PKG_NAME").to_string(),
            path: PathBuf::new(),
        }
    }
}

impl Display for Sysdir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let x = self.path.clone();
        x.fmt(f)
    }
}

impl From<Sysdir> for PathBuf {
    fn from(s: Sysdir) -> Self {
        s.path
    }
}

impl AsRef<Path> for Sysdir {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Sysdir {
    pub fn custom_name(app_name: impl ToString) -> Self {
        Self {
            app_name: app_name.to_string(),
            ..Default::default()
        }
    }

    fn config_name(&self, file: impl AsRef<Path>) -> PathBuf {
        dirs::config_dir()
            .unwrap()
            .join(&self.app_name)
            .join(file.as_ref())
    }

    fn define_path(&self, file: impl AsRef<Path>, current_dir: bool) -> Self {
        let mut x = self.clone();
        match current_dir {
            true => {
                x.path = Path::new(".").join(file.as_ref());
            }
            false => {
                x.path = dirs::config_dir()
                    .unwrap()
                    .join(&self.app_name)
                    .join(file.as_ref());
            }
        }
        x
    }

    pub fn find_path(&self, file: impl AsRef<Path>) -> Option<Self> {
        let x = file.as_ref();
        let cur_dir = self.define_path(x, true);
        let sys_dir = self.define_path(x, false);

        if cur_dir.path.exists() {
            Some(cur_dir)
        } else if sys_dir.path.exists() {
            Some(sys_dir)
        } else {
            log::error!("Cant find file on current path or sys path");
            None
        }
    }

    pub fn config_dir(&self, file: impl AsRef<Path>) -> Self {
        self.define_path(file, false)
    }
    pub fn current_dir(&self, file: impl AsRef<Path>) -> Self {
        self.define_path(file, true)
    }

    pub fn execute_dir(&self) -> PathBuf {
        let p = if self.path.is_dir() {
            Some(self.path.as_path())
        } else {
            self.path.parent()
        };
        if p.unwrap().exists() {
            match std::fs::create_dir_all(p.unwrap()) {
                Ok(_) => return self.path.clone(),
                Err(_) => return Path::new(".").join(self.path.file_name().unwrap()),
            }
        }
        PathBuf::from("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name() {
        let x = Sysdir::default();
        println!("{}", x.config_dir("myconfig.txt"));
    }
}
