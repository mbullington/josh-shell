use std::{
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShellContextError {
    #[error("invalid environment variable name `{name}`: {message}")]
    InvalidEnvironmentName { name: String, message: String },
    #[error("invalid value for environment variable `{name}`: {message}")]
    InvalidEnvironmentValue { name: String, message: String },
    #[error("cannot change directory to {path}: {source}")]
    ChangeDirectory {
        path: String,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, Clone)]
pub struct ShellSnapshot {
    cwd: PathBuf,
    environment: Arc<BTreeMap<OsString, OsString>>,
}

impl ShellSnapshot {
    #[must_use]
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    #[must_use]
    pub fn environment(&self) -> &BTreeMap<OsString, OsString> {
        &self.environment
    }

    #[must_use]
    pub fn environment_variable(&self, name: &OsStr) -> Option<&OsStr> {
        self.environment.get(name).map(OsString::as_os_str)
    }
}

#[derive(Debug)]
struct ShellState {
    cwd: PathBuf,
    environment: BTreeMap<OsString, OsString>,
}

#[derive(Debug, Clone)]
pub struct ShellContext {
    state: Arc<RwLock<ShellState>>,
}

impl Default for ShellContext {
    fn default() -> Self {
        Self::from_process()
    }
}

impl ShellContext {
    #[must_use]
    pub fn from_process() -> Self {
        Self::new(
            env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            env::vars_os(),
        )
    }

    #[must_use]
    pub fn new(
        cwd: impl Into<PathBuf>,
        environment: impl IntoIterator<Item = (OsString, OsString)>,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(ShellState {
                cwd: cwd.into(),
                environment: environment.into_iter().collect(),
            })),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> ShellSnapshot {
        let state = self.state.read().expect("shell context lock poisoned");
        ShellSnapshot {
            cwd: state.cwd.clone(),
            environment: Arc::new(state.environment.clone()),
        }
    }

    #[must_use]
    pub fn current_directory(&self) -> PathBuf {
        self.state
            .read()
            .expect("shell context lock poisoned")
            .cwd
            .clone()
    }

    #[must_use]
    pub fn environment_variable(&self, name: &OsStr) -> Option<OsString> {
        self.state
            .read()
            .expect("shell context lock poisoned")
            .environment
            .get(name)
            .cloned()
    }

    #[must_use]
    pub fn environment_names(&self) -> Vec<String> {
        self.state
            .read()
            .expect("shell context lock poisoned")
            .environment
            .keys()
            .filter_map(|name| name.to_str().map(str::to_owned))
            .collect()
    }

    pub fn set_environment_variable(
        &self,
        name: &str,
        value: Option<OsString>,
    ) -> Result<(), ShellContextError> {
        validate_environment_name(name)?;
        if let Some(value) = value.as_deref() {
            validate_environment_value(name, value)?;
        }
        let mut state = self.state.write().expect("shell context lock poisoned");
        if let Some(value) = value {
            state.environment.insert(name.into(), value);
        } else {
            state.environment.remove(OsStr::new(name));
        }
        Ok(())
    }

    pub fn change_directory(&self, path: &OsStr) -> Result<(), ShellContextError> {
        let display = path.to_string_lossy().into_owned();
        if os_contains_nul(path) {
            return Err(ShellContextError::ChangeDirectory {
                path: display,
                source: io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"),
            });
        }
        let current = self.current_directory();
        let requested = Path::new(path);
        let target = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            current.join(requested)
        };
        let target =
            target
                .canonicalize()
                .map_err(|source| ShellContextError::ChangeDirectory {
                    path: display.clone(),
                    source,
                })?;
        if !target.is_dir() {
            return Err(ShellContextError::ChangeDirectory {
                path: display,
                source: io::Error::new(io::ErrorKind::NotADirectory, "not a directory"),
            });
        }
        self.state.write().expect("shell context lock poisoned").cwd = target;
        Ok(())
    }
}

fn validate_environment_name(name: &str) -> Result<(), ShellContextError> {
    let message = if name.is_empty() {
        Some("names cannot be empty")
    } else if name.as_bytes().contains(&0) {
        Some("names cannot contain NUL")
    } else if name.as_bytes().contains(&b'=') {
        Some("names cannot contain `=`")
    } else {
        None
    };
    if let Some(message) = message {
        Err(ShellContextError::InvalidEnvironmentName {
            name: name.to_owned(),
            message: message.into(),
        })
    } else {
        Ok(())
    }
}

fn validate_environment_value(name: &str, value: &OsStr) -> Result<(), ShellContextError> {
    if os_contains_nul(value) {
        Err(ShellContextError::InvalidEnvironmentValue {
            name: name.to_owned(),
            message: "values cannot contain NUL".into(),
        })
    } else {
        Ok(())
    }
}

fn os_contains_nul(value: &OsStr) -> bool {
    value.as_encoded_bytes().contains(&0)
}
