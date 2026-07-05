use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::RecentProjectState;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppStatePathProvider {
    state_dir: PathBuf,
}

impl AppStatePathProvider {
    pub fn linux_default() -> Result<Self, RecentProjectStoreError> {
        Self::linux_from_env(std::env::var_os("XDG_STATE_HOME"), std::env::var_os("HOME"))
    }

    pub fn linux_from_env(
        xdg_state_home: Option<impl AsRef<OsStr>>,
        home: Option<impl AsRef<OsStr>>,
    ) -> Result<Self, RecentProjectStoreError> {
        if let Some(value) = xdg_state_home.filter(|value| !value.as_ref().is_empty()) {
            return Ok(Self {
                state_dir: PathBuf::from(value.as_ref()).join("tekstide"),
            });
        }

        let Some(home) = home.filter(|value| !value.as_ref().is_empty()) else {
            return Err(RecentProjectStoreError::PathUnavailable(
                "HOME is unavailable; recent-project state will not be persisted".to_owned(),
            ));
        };

        Ok(Self {
            state_dir: PathBuf::from(home.as_ref()).join(".local/state/tekstide"),
        })
    }

    pub fn from_state_dir(state_dir: impl Into<PathBuf>) -> Self {
        Self {
            state_dir: state_dir.into(),
        }
    }

    pub fn recent_projects_file(&self) -> PathBuf {
        self.state_dir.join("recent-projects.json")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecentProjectStoreError {
    PathUnavailable(String),
    Io(String),
    CorruptState(String),
}

impl fmt::Display for RecentProjectStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathUnavailable(message) | Self::Io(message) | Self::CorruptState(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl std::error::Error for RecentProjectStoreError {}

#[derive(Clone, Debug)]
pub struct RecentProjectStore {
    state_file: PathBuf,
}

impl RecentProjectStore {
    pub fn new(path_provider: AppStatePathProvider) -> Self {
        Self {
            state_file: path_provider.recent_projects_file(),
        }
    }

    pub fn load(&self) -> Result<RecentProjectState, RecentProjectStoreError> {
        let content = match fs::read_to_string(&self.state_file) {
            Ok(content) => content,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(RecentProjectState::default());
            }
            Err(error) => {
                return Err(RecentProjectStoreError::Io(format!(
                    "failed to read recent-project state: {error}"
                )));
            }
        };

        match RecentProjectState::from_json(&content) {
            Ok(state) => Ok(state),
            Err(error) => {
                let _ = self.rename_corrupt_state();
                Err(RecentProjectStoreError::CorruptState(error))
            }
        }
    }

    pub fn save(&self, state: &RecentProjectState) -> Result<(), RecentProjectStoreError> {
        if let Some(parent) = self.state_file.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                RecentProjectStoreError::Io(format!(
                    "failed to create recent-project state directory: {error}"
                ))
            })?;
        }

        let temp_file = self.state_file.with_extension("json.tmp");
        write_state_atomically(&temp_file, &self.state_file, &state.to_json())
    }

    fn rename_corrupt_state(&self) -> io::Result<PathBuf> {
        let corrupt_path = next_corrupt_path(&self.state_file);
        fs::rename(&self.state_file, &corrupt_path)?;
        Ok(corrupt_path)
    }
}

fn write_state_atomically(
    temp_file: &Path,
    state_file: &Path,
    content: &str,
) -> Result<(), RecentProjectStoreError> {
    {
        let mut file = File::create(temp_file).map_err(|error| {
            RecentProjectStoreError::Io(format!("failed to create temporary state file: {error}"))
        })?;
        file.write_all(content.as_bytes()).map_err(|error| {
            RecentProjectStoreError::Io(format!("failed to write temporary state file: {error}"))
        })?;
        let _ = file.sync_data();
    }

    fs::rename(temp_file, state_file).map_err(|error| {
        let _ = fs::remove_file(temp_file);
        RecentProjectStoreError::Io(format!("failed to replace recent-project state: {error}"))
    })?;

    if let Some(parent) = state_file.parent()
        && let Ok(directory) = File::open(parent)
    {
        let _ = directory.sync_all();
    }

    Ok(())
}

fn next_corrupt_path(state_file: &Path) -> PathBuf {
    let first = state_file.with_extension("json.corrupt");
    if !first.exists() {
        return first;
    }

    for index in 1_u32.. {
        let candidate = state_file.with_extension(format!("json.corrupt-{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("unbounded corrupt filename search should always return")
}
