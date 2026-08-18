use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

/// RFC-023 PR-023-B: platform resolution for the configuration
/// directory (`<config base>/tekstide/`, containing `config.toml`).
/// Mirrors [`crate::project::recent::AppStatePathProvider`]'s
/// `linux_default`/`linux_from_env` shape -- an env-reading `_default`
/// convenience alongside a pure, injectable `_from_env` that is fully
/// testable regardless of host OS. `macos_from_env`/`windows_from_env`
/// exist as real, tested path-construction logic even though nothing
/// calls `macos_default`/`windows_default` from a real entry point yet
/// -- no `#[cfg(target_os = ..)]` dispatcher exists because nothing
/// needs one until a real macOS/Windows build calls it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigPathProvider {
    config_dir: PathBuf,
}

impl ConfigPathProvider {
    pub fn linux_default() -> Result<Self, ConfigPathError> {
        Self::linux_from_env(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )
    }

    /// `REQ-CONFIG-002`: `$XDG_CONFIG_HOME/tekstide`, falling back to
    /// `$HOME/.config/tekstide`.
    pub fn linux_from_env(
        xdg_config_home: Option<impl AsRef<OsStr>>,
        home: Option<impl AsRef<OsStr>>,
    ) -> Result<Self, ConfigPathError> {
        if let Some(value) = xdg_config_home.filter(|value| !value.as_ref().is_empty()) {
            return Ok(Self {
                config_dir: PathBuf::from(value.as_ref()).join("tekstide"),
            });
        }

        let Some(home) = home.filter(|value| !value.as_ref().is_empty()) else {
            return Err(ConfigPathError::new(ConfigPathErrorReason::PathUnavailable));
        };
        Ok(Self {
            config_dir: PathBuf::from(home.as_ref()).join(".config/tekstide"),
        })
    }

    pub fn macos_default() -> Result<Self, ConfigPathError> {
        Self::macos_from_env(std::env::var_os("HOME"))
    }

    /// `~/Library/Application Support/tekstide`.
    pub fn macos_from_env(home: Option<impl AsRef<OsStr>>) -> Result<Self, ConfigPathError> {
        let Some(home) = home.filter(|value| !value.as_ref().is_empty()) else {
            return Err(ConfigPathError::new(ConfigPathErrorReason::PathUnavailable));
        };
        Ok(Self {
            config_dir: PathBuf::from(home.as_ref())
                .join("Library")
                .join("Application Support")
                .join("tekstide"),
        })
    }

    pub fn windows_default() -> Result<Self, ConfigPathError> {
        Self::windows_from_env(std::env::var_os("APPDATA"))
    }

    /// `%APPDATA%\tekstide`.
    pub fn windows_from_env(appdata: Option<impl AsRef<OsStr>>) -> Result<Self, ConfigPathError> {
        let Some(appdata) = appdata.filter(|value| !value.as_ref().is_empty()) else {
            return Err(ConfigPathError::new(ConfigPathErrorReason::PathUnavailable));
        };
        Ok(Self {
            config_dir: PathBuf::from(appdata.as_ref()).join("tekstide"),
        })
    }

    pub fn from_config_dir(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }
}

/// The resolved, path-discipline-checked location of `config.toml` and
/// its containing directory. Constructing one does **not** require
/// either to exist on disk -- "a missing configuration file is not an
/// error" (RFC-023 §Format and Location) applies at this layer too, not
/// only at the loader (PR-023-C). What this **does** check, whenever
/// something exists to check: the configuration directory and file
/// never resolve, via a symlink, outside the platform configuration
/// root they were computed under -- the same discipline
/// [`crate::audit::path`] applies to `audit_dir`/`database_file` under
/// `state_root`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigStoragePath {
    config_dir: PathBuf,
    config_file: PathBuf,
}

impl ConfigStoragePath {
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn config_file(&self) -> &Path {
        &self.config_file
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigPathErrorReason {
    PathUnavailable,
    ConfigDirNotAbsolute,
    ConfigDirEscapesConfigRoot,
    ConfigDirTypeInvalid,
    ConfigFileIsSymlink,
    ConfigFileTypeInvalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigPathError {
    pub reason: ConfigPathErrorReason,
}

impl ConfigPathError {
    fn new(reason: ConfigPathErrorReason) -> Self {
        Self { reason }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ConfigPathResolver;

impl ConfigPathResolver {
    pub fn resolve(
        self,
        provider: &ConfigPathProvider,
    ) -> Result<ConfigStoragePath, ConfigPathError> {
        let config_dir = provider.config_dir();
        if !config_dir.is_absolute() {
            return Err(ConfigPathError::new(
                ConfigPathErrorReason::ConfigDirNotAbsolute,
            ));
        }

        let config_file = config_dir.join("config.toml");
        validate_existing_config_paths(config_dir, &config_file)?;

        Ok(ConfigStoragePath {
            config_dir: config_dir.to_path_buf(),
            config_file,
        })
    }
}

/// Mirrors `audit/path.rs`'s `validate_existing_audit_paths` exactly:
/// every check is `if let Ok(metadata) = fs::symlink_metadata(..)`, a
/// no-op when the path does not exist, so a first run with neither the
/// configuration directory nor the file ever created passes cleanly --
/// "missing configuration is not an error" holds at the path layer, not
/// only the loader. `config_dir`'s escape check treats its **parent**
/// (the platform configuration root -- `$XDG_CONFIG_HOME` or
/// equivalent) as the trusted anchor a symlinked `tekstide/` must not
/// resolve outside of, the same relationship `state_root` has to
/// `audit_dir`. `config_file` follows `database_file`'s stricter rule:
/// a symlink at all is rejected outright, not merely an escaping one --
/// a config file is the one thing actually parsed and applied, so
/// unlike a container directory there is no legitimate reason for it to
/// be a symlink.
fn validate_existing_config_paths(
    config_dir: &Path,
    config_file: &Path,
) -> Result<(), ConfigPathError> {
    if let Ok(metadata) = fs::symlink_metadata(config_dir) {
        if metadata.file_type().is_symlink() {
            let anchor = config_dir.parent().ok_or_else(|| {
                ConfigPathError::new(ConfigPathErrorReason::ConfigDirEscapesConfigRoot)
            })?;
            let canonical_anchor = fs::canonicalize(anchor).map_err(|_| {
                ConfigPathError::new(ConfigPathErrorReason::ConfigDirEscapesConfigRoot)
            })?;
            let canonical_dir = fs::canonicalize(config_dir).map_err(|_| {
                ConfigPathError::new(ConfigPathErrorReason::ConfigDirEscapesConfigRoot)
            })?;
            if !canonical_dir.starts_with(&canonical_anchor) {
                return Err(ConfigPathError::new(
                    ConfigPathErrorReason::ConfigDirEscapesConfigRoot,
                ));
            }
        } else if !metadata.is_dir() {
            return Err(ConfigPathError::new(
                ConfigPathErrorReason::ConfigDirTypeInvalid,
            ));
        }
    }

    if let Ok(metadata) = fs::symlink_metadata(config_file) {
        if metadata.file_type().is_symlink() {
            return Err(ConfigPathError::new(
                ConfigPathErrorReason::ConfigFileIsSymlink,
            ));
        }
        if !metadata.is_file() {
            return Err(ConfigPathError::new(
                ConfigPathErrorReason::ConfigFileTypeInvalid,
            ));
        }
    }

    Ok(())
}
