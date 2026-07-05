use crate::project::ProjectId;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const RECENT_PROJECT_STATE_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecentProjectState {
    pub state_version: u32,
    pub projects: Vec<RecentProject>,
}

impl Default for RecentProjectState {
    fn default() -> Self {
        Self {
            state_version: RECENT_PROJECT_STATE_VERSION,
            projects: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecentProject {
    pub project_id: ProjectId,
    pub display_name: String,
    pub root_path: PathBuf,
    pub canonical_root_path: PathBuf,
    pub last_opened_at: Timestamp,
    pub last_activity: Timestamp,
    pub last_trust_state_summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecentProjectAvailability {
    Available,
    FolderMissing,
    CannotReadFolder,
    PathChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoredRecentProject {
    pub recent_project: RecentProject,
    pub availability: RecentProjectAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Timestamp(String);

impl Timestamp {
    pub fn now_utc() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self(format_unix_seconds_utc(seconds))
    }

    pub fn from_persisted(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(value: &str) -> bool {
        let bytes = value.as_bytes();
        bytes.len() == 20
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes[10] == b'T'
            && bytes[13] == b':'
            && bytes[16] == b':'
            && bytes[19] == b'Z'
            && bytes.iter().enumerate().all(|(index, byte)| {
                matches!(index, 4 | 7 | 10 | 13 | 16 | 19) || byte.is_ascii_digit()
            })
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if Self::validate(&value) {
            Ok(Self(value))
        } else {
            Err(serde::de::Error::custom(
                "timestamp must use YYYY-MM-DDTHH:MM:SSZ UTC format",
            ))
        }
    }
}

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

impl RecentProjectState {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self)
            .expect("recent-project state serialization should not fail")
            + "\n"
    }

    pub fn from_json(content: &str) -> Result<Self, String> {
        let state = serde_json::from_str::<Self>(content).map_err(|error| error.to_string())?;
        state.validate()?;
        Ok(state)
    }

    fn validate(&self) -> Result<(), String> {
        if self.state_version != RECENT_PROJECT_STATE_VERSION {
            return Err(format!(
                "unsupported recent-project state_version: {}",
                self.state_version
            ));
        }

        let mut project_ids = HashSet::new();
        let mut canonical_roots = HashSet::new();

        for project in &self.projects {
            if !project_ids.insert(project.project_id.clone()) {
                return Err(format!("duplicate project_id: {}", project.project_id));
            }
            if !canonical_roots.insert(project.canonical_root_path.clone()) {
                return Err(format!(
                    "duplicate canonical_root_path: {}",
                    project.canonical_root_path.display()
                ));
            }
        }

        Ok(())
    }
}

impl RecentProject {
    pub fn new(
        project_id: ProjectId,
        display_name: impl Into<String>,
        root_path: impl Into<PathBuf>,
        canonical_root_path: impl Into<PathBuf>,
        now: Timestamp,
        last_trust_state_summary: impl Into<String>,
    ) -> Self {
        Self {
            project_id,
            display_name: display_name.into(),
            root_path: root_path.into(),
            canonical_root_path: canonical_root_path.into(),
            last_opened_at: now.clone(),
            last_activity: now,
            last_trust_state_summary: last_trust_state_summary.into(),
        }
    }
}

pub fn assess_recent_project_availability(project: &RecentProject) -> RecentProjectAvailability {
    let Ok(metadata) = fs::metadata(&project.root_path) else {
        return RecentProjectAvailability::FolderMissing;
    };

    if !metadata.is_dir() {
        return RecentProjectAvailability::CannotReadFolder;
    }

    let Ok(canonical_path) = fs::canonicalize(&project.root_path) else {
        return RecentProjectAvailability::CannotReadFolder;
    };

    if canonical_path != project.canonical_root_path {
        return RecentProjectAvailability::PathChanged;
    }

    if fs::read_dir(&canonical_path).is_err() {
        return RecentProjectAvailability::CannotReadFolder;
    }

    RecentProjectAvailability::Available
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

fn format_unix_seconds_utc(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, u64, u64) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month as u64, day as u64)
}

#[cfg(test)]
mod tests;
