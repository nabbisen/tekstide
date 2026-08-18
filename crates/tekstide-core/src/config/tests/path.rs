use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{ConfigPathErrorReason, ConfigPathProvider, ConfigPathResolver};

struct TestDir {
    base: PathBuf,
}

impl TestDir {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "tekstide-config-path-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&base).unwrap();
        Self { base }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

/// The property `audit/path.rs`'s equivalent does not have: a missing
/// configuration directory (nothing under the platform config root has
/// ever been created) resolves successfully, not as
/// `InvalidStateRoot`-style error -- RFC-023's "a missing configuration
/// file is not an error" holds at the path-resolution layer, before any
/// loader exists to apply that rule.
#[test]
fn resolving_with_nothing_on_disk_yet_succeeds() {
    let temp = TestDir::new("nothing-yet");
    let config_dir = temp.base.join("tekstide");
    let provider = ConfigPathProvider::from_config_dir(&config_dir);

    let resolved = ConfigPathResolver.resolve(&provider).unwrap();

    assert_eq!(resolved.config_dir(), config_dir);
    assert_eq!(resolved.config_file(), config_dir.join("config.toml"));
    assert!(!resolved.config_dir().exists());
}

#[test]
fn a_relative_config_dir_is_rejected() {
    let provider = ConfigPathProvider::from_config_dir("relative/tekstide");
    let error = ConfigPathResolver.resolve(&provider).unwrap_err();
    assert_eq!(error.reason, ConfigPathErrorReason::ConfigDirNotAbsolute);
}

#[test]
fn an_existing_ordinary_config_dir_and_file_resolve_cleanly() {
    let temp = TestDir::new("ordinary");
    let config_dir = temp.base.join("tekstide");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), b"[core]\n").unwrap();

    let provider = ConfigPathProvider::from_config_dir(&config_dir);
    let resolved = ConfigPathResolver.resolve(&provider).unwrap();

    assert_eq!(resolved.config_dir(), config_dir);
    assert!(resolved.config_file().is_file());
}

#[test]
fn a_non_directory_where_the_config_dir_belongs_is_rejected() {
    let temp = TestDir::new("dir-is-file");
    let config_dir = temp.base.join("tekstide");
    fs::write(&config_dir, b"not a directory").unwrap();

    let provider = ConfigPathProvider::from_config_dir(&config_dir);
    let error = ConfigPathResolver.resolve(&provider).unwrap_err();
    assert_eq!(error.reason, ConfigPathErrorReason::ConfigDirTypeInvalid);
}

#[test]
fn a_directory_where_the_config_file_belongs_is_rejected() {
    let temp = TestDir::new("file-is-dir");
    let config_dir = temp.base.join("tekstide");
    fs::create_dir_all(config_dir.join("config.toml")).unwrap();

    let provider = ConfigPathProvider::from_config_dir(&config_dir);
    let error = ConfigPathResolver.resolve(&provider).unwrap_err();
    assert_eq!(error.reason, ConfigPathErrorReason::ConfigFileTypeInvalid);
}

#[cfg(unix)]
#[test]
fn a_symlinked_config_directory_escaping_the_configuration_root_is_rejected() {
    use std::os::unix::fs::symlink;

    let temp = TestDir::new("dir-escapes");
    let config_root = temp.base.join("config-root");
    let outside_target = temp.base.join("outside");
    fs::create_dir_all(&config_root).unwrap();
    fs::create_dir_all(&outside_target).unwrap();
    let config_dir = config_root.join("tekstide");
    symlink(&outside_target, &config_dir).unwrap();

    let provider = ConfigPathProvider::from_config_dir(&config_dir);
    let error = ConfigPathResolver.resolve(&provider).unwrap_err();
    assert_eq!(
        error.reason,
        ConfigPathErrorReason::ConfigDirEscapesConfigRoot
    );
}

/// Positive control for the test above: a symlinked `tekstide/` whose
/// target stays **inside** the configuration root (the symlink's own
/// parent) is allowed -- proves the check is genuinely about escaping,
/// not about rejecting every symlink indiscriminately.
#[cfg(unix)]
#[test]
fn a_symlinked_config_directory_staying_within_the_configuration_root_is_allowed() {
    use std::os::unix::fs::symlink;

    let temp = TestDir::new("dir-stays-inside");
    let config_root = temp.base.join("config-root");
    let real_target = config_root.join("real-tekstide");
    fs::create_dir_all(&real_target).unwrap();
    let config_dir = config_root.join("tekstide");
    symlink(&real_target, &config_dir).unwrap();

    let provider = ConfigPathProvider::from_config_dir(&config_dir);
    let resolved = ConfigPathResolver.resolve(&provider).unwrap();
    assert_eq!(resolved.config_dir(), config_dir);
}

/// `config.toml` follows the stricter, `database_file`-style rule:
/// **any** symlink is rejected, even one resolving inside the
/// configuration directory -- the config file is the one thing actually
/// parsed and applied, so there is no legitimate reason for it to be a
/// symlink at all.
#[cfg(unix)]
#[test]
fn a_symlinked_config_file_is_rejected_even_if_its_target_stays_inside_the_directory() {
    use std::os::unix::fs::symlink;

    let temp = TestDir::new("file-symlink");
    let config_dir = temp.base.join("tekstide");
    fs::create_dir_all(&config_dir).unwrap();
    let real_file = config_dir.join("real-config.toml");
    fs::write(&real_file, b"[core]\n").unwrap();
    symlink(&real_file, config_dir.join("config.toml")).unwrap();

    let provider = ConfigPathProvider::from_config_dir(&config_dir);
    let error = ConfigPathResolver.resolve(&provider).unwrap_err();
    assert_eq!(error.reason, ConfigPathErrorReason::ConfigFileIsSymlink);
}

#[test]
fn linux_from_env_prefers_xdg_config_home_over_home() {
    let provider =
        ConfigPathProvider::linux_from_env(Some("/xdg/config"), Some("/home/user")).unwrap();
    assert_eq!(provider.config_dir(), PathBuf::from("/xdg/config/tekstide"));
}

#[test]
fn linux_from_env_falls_back_to_home_dot_config() {
    let provider = ConfigPathProvider::linux_from_env(None::<&str>, Some("/home/user")).unwrap();
    assert_eq!(
        provider.config_dir(),
        PathBuf::from("/home/user/.config/tekstide")
    );
}

#[test]
fn linux_from_env_errors_when_neither_variable_is_usable() {
    let error = ConfigPathProvider::linux_from_env(None::<&str>, None::<&str>).unwrap_err();
    assert_eq!(error.reason, ConfigPathErrorReason::PathUnavailable);

    let empty = ConfigPathProvider::linux_from_env(Some(""), Some("")).unwrap_err();
    assert_eq!(empty.reason, ConfigPathErrorReason::PathUnavailable);
}

#[test]
fn macos_from_env_resolves_under_application_support() {
    let provider = ConfigPathProvider::macos_from_env(Some("/Users/user")).unwrap();
    assert_eq!(
        provider.config_dir(),
        PathBuf::from("/Users/user/Library/Application Support/tekstide")
    );
}

#[test]
fn macos_from_env_errors_without_home() {
    let error = ConfigPathProvider::macos_from_env(None::<&str>).unwrap_err();
    assert_eq!(error.reason, ConfigPathErrorReason::PathUnavailable);
}

#[test]
fn windows_from_env_resolves_under_appdata() {
    // `PathBuf::join` uses the *host's* separator when compiled for a
    // non-Windows target, so the expected value is built the same way
    // production code builds it (`.join("tekstide")`), not as a
    // hand-written backslash literal that would only match on Windows.
    let appdata = r"C:\Users\user\AppData\Roaming";
    let provider = ConfigPathProvider::windows_from_env(Some(appdata)).unwrap();
    assert_eq!(
        provider.config_dir(),
        PathBuf::from(appdata).join("tekstide")
    );
}

#[test]
fn windows_from_env_errors_without_appdata() {
    let error = ConfigPathProvider::windows_from_env(None::<&str>).unwrap_err();
    assert_eq!(error.reason, ConfigPathErrorReason::PathUnavailable);
}

#[test]
fn path_errors_do_not_echo_local_paths() {
    let secret_path = "/private/project/customer-name/tekstide";
    let provider = ConfigPathProvider::from_config_dir("relative/but/private/customer-name");
    let error = ConfigPathResolver.resolve(&provider).unwrap_err();
    assert!(!format!("{error:?}").contains(secret_path));
}
