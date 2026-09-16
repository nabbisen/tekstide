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

    /// RFC-017 PR-017-F: the same Tekstide application state root
    /// `recent_projects_file` is a filename under -- reused as-is for
    /// the audit store's `<tekstide-state-root>` (RFC-013's own
    /// diagram), rather than a second, independently-resolved
    /// `XDG_STATE_HOME`/`HOME` fallback for the audit path. One
    /// resolution, two consumers.
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecentProjectStoreError {
    PathUnavailable(String),
    Io(String),
    /// The file could not be parsed. `moved_to` is where it was renamed, so
    /// the board can say where it went (RFC-050 PR-050-C); `None` when the
    /// rename itself failed.
    CorruptState {
        message: String,
        moved_to: Option<PathBuf>,
    },
}

impl fmt::Display for RecentProjectStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathUnavailable(message)
            | Self::Io(message)
            | Self::CorruptState { message, .. } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for RecentProjectStoreError {}

/// RFC-051 D6′: **the one typed outcome of trying to load the list.**
///
/// `boot()` must not sequence "load, notice it failed, look for a backup, write
/// one" — two prior slices shipped a correct decision with one call site
/// unguarded, and the way not to have that problem is not to have the call site.
/// The store does the whole sequence §6 fixes — **quarantine, then recover, then
/// allow saving** — and hands back what happened.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecentProjectLoad {
    /// What the session should start with. Empty for a reset.
    pub state: RecentProjectState,
    pub outcome: RecentProjectLoadOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecentProjectLoadOutcome {
    /// The live file parsed, or did not exist at all (a first start).
    Loaded,
    /// The live file was unusable and **the backup supplied the list**. Ids are
    /// the ones the backup held, which is the whole point: a project's id is
    /// what a trust grant is matched by and what its transcript directory is
    /// named after.
    Recovered {
        /// Where the unusable live file was moved, or `None` if that rename
        /// failed — in which case it is still in place and **nothing may be
        /// saved over it** (§1).
        moved_to: Option<PathBuf>,
        message: String,
    },
    /// The live file was unusable and no usable backup existed. The session
    /// starts empty, and the user is told this rather than the recovered case
    /// (§5, D5).
    Reset {
        moved_to: Option<PathBuf>,
        message: String,
    },
}

impl RecentProjectLoadOutcome {
    pub fn moved_to(&self) -> Option<&Path> {
        match self {
            Self::Loaded => None,
            Self::Recovered { moved_to, .. } | Self::Reset { moved_to, .. } => moved_to.as_deref(),
        }
    }
}

/// What a save did, so a caller can tell "written" from "deliberately not
/// written" without inferring it from an `Ok(())`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecentProjectSave {
    /// Written. `backup` says whether the previous-good copy was updated too —
    /// false whenever this session's list did not come from a real load (§2).
    Written { backup: bool },
    /// **Nothing was written** (§1): the live file could not be read *and* could
    /// not be quarantined, so it is still there. A session runs perfectly well
    /// without persisting a list; a user's unreadable file is worth more than
    /// our empty one.
    Withheld,
}

#[derive(Clone, Debug)]
pub struct RecentProjectStore {
    state_file: PathBuf,
    /// §1: cleared when a quarantine rename failed, and never set again this
    /// session. Saving would overwrite the file recovery would have needed.
    saving_allowed: bool,
    /// §2/D2′: set only by a load that really produced a list — a session that
    /// started empty after a failed load must not make its first save the
    /// backup, which would destroy the only good copy.
    backup_allowed: bool,
}

impl RecentProjectStore {
    pub fn new(path_provider: AppStatePathProvider) -> Self {
        Self {
            state_file: path_provider.recent_projects_file(),
            saving_allowed: true,
            backup_allowed: false,
        }
    }

    /// RFC-051 D6′/§6: the whole sequence, in the order that makes it safe.
    ///
    /// 1. **Quarantine** an unusable live file by rename — an unreadable one as
    ///    well as an unparseable one, which is §1's defect: today only the
    ///    unparseable case is renamed, and `boot()` then saves an empty list
    ///    over the unreadable one.
    /// 2. **Recover** from `recent-projects.json.bak`, read and parsed **whole**
    ///    (§3). A backup that is itself unusable is a reset, not a partial
    ///    anything: a half-parsed list can resurrect a wrong path-to-id mapping,
    ///    and a wrong id re-attaches somebody's trust grant to the wrong folder.
    /// 3. Only then is saving allowed — and only after a rename that worked.
    ///
    /// **Recovery restores ids and paths; it never restores trust** (§4).
    /// `verify_restored_trust` re-checks every restored trusted project against
    /// the audit store, and that check is unchanged by this RFC.
    pub fn load_or_recover(&mut self) -> RecentProjectLoad {
        let message = match fs::read_to_string(&self.state_file) {
            Ok(content) => match RecentProjectState::from_json(&content) {
                Ok(state) => {
                    self.backup_allowed = true;
                    return RecentProjectLoad {
                        state,
                        outcome: RecentProjectLoadOutcome::Loaded,
                    };
                }
                Err(error) => format!("recent-project state could not be parsed: {error}"),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // A first start. There is no live file to protect and none to
                // recover; a real list saved later may become the backup.
                self.backup_allowed = true;
                return RecentProjectLoad {
                    state: RecentProjectState::default(),
                    outcome: RecentProjectLoadOutcome::Loaded,
                };
            }
            Err(error) => format!("recent-project state could not be read: {error}"),
        };

        // Quarantine first, before anything else touches the file (§6).
        let moved_to = self.rename_corrupt_state().ok();
        if moved_to.is_none() {
            // §1: the file we could not read is still there. Saving now would
            // destroy it, so this session persists nothing at all.
            self.saving_allowed = false;
        }

        match self.read_backup() {
            Some(state) => {
                self.backup_allowed = true;
                RecentProjectLoad {
                    state,
                    outcome: RecentProjectLoadOutcome::Recovered { moved_to, message },
                }
            }
            None => RecentProjectLoad {
                state: RecentProjectState::default(),
                outcome: RecentProjectLoadOutcome::Reset { moved_to, message },
            },
        }
    }

    /// §3: whole file or nothing. Missing, unreadable and unparseable are the
    /// same answer — there is no backup to recover from.
    fn read_backup(&self) -> Option<RecentProjectState> {
        let content = fs::read_to_string(self.backup_file()).ok()?;
        RecentProjectState::from_json(&content).ok()
    }

    pub fn backup_file(&self) -> PathBuf {
        self.state_file.with_extension("json.bak")
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
                let moved_to = self.rename_corrupt_state().ok();
                Err(RecentProjectStoreError::CorruptState {
                    message: error,
                    moved_to,
                })
            }
        }
    }

    /// RFC-051 §1/§2: writes the live file, and the previous-good copy **only**
    /// when this session's list really came from a load.
    ///
    /// Returns [`RecentProjectSave::Withheld`] without touching anything when a
    /// quarantine rename failed earlier this session: the unreadable file is
    /// still in place, and it is worth more than our empty list.
    pub fn save(
        &self,
        state: &RecentProjectState,
    ) -> Result<RecentProjectSave, RecentProjectStoreError> {
        if !self.saving_allowed {
            return Ok(RecentProjectSave::Withheld);
        }

        if let Some(parent) = self.state_file.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                RecentProjectStoreError::Io(format!(
                    "failed to create recent-project state directory: {error}"
                ))
            })?;
        }

        let content = state.to_json();
        let temp_file = self.state_file.with_extension("json.tmp");
        write_state_atomically(&temp_file, &self.state_file, &content)?;

        if !self.backup_allowed {
            // D2′: a session that started empty after a failed load writes no
            // backup at all. Otherwise its first save would overwrite the only
            // good copy with the empty list — the same destruction this RFC
            // exists to stop, one file further along.
            return Ok(RecentProjectSave::Written { backup: false });
        }

        // Best-effort, and deliberately after the live file: a backup that
        // cannot be written must not fail the save the user's action asked for.
        let backup_file = self.backup_file();
        let backup_temp = self.state_file.with_extension("json.bak.tmp");
        let backup = write_state_atomically(&backup_temp, &backup_file, &content).is_ok();
        Ok(RecentProjectSave::Written { backup })
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
