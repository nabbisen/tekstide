//! RFC-052 PR-052-A: the hostile fixture, one builder.
//!
//! A file explorer renders names an attacker controls and walks a
//! directory structure an attacker controls. This builds a project whose
//! contents are exactly that -- every row RFC-052's checklist names -- so a
//! mechanism is judged against hostile input rather than a README.
//!
//! **Standard library only, on purpose.** The same file is compiled into
//! `tekstide-core`'s own tests *and* `#[path]`-included by the standalone
//! measurement crate at
//! `rfcs/handoffs/052-file-explorer/measurement/`, which points
//! `iced-swdir-tree` at it. One builder means the widget and our own
//! composition are measured against byte-identical input.
//!
//! Every path is under a fresh directory beneath the system temp
//! directory; nothing here reads the developer's home, environment or
//! configuration. Unix only (symlinks, modes, non-UTF-8 names).
//!
//! ```text
//! <base>/
//!   outside/                     <- the thing the escape rows point at
//!     secret.txt
//!     nested/inner.txt
//!   project/                     <- the project root
//!     control/README.md, control/src/lib.rs, control/src/nested/deep.txt
//!     escaping/<bidi>, <newline>, <non-utf8>
//!     links/escape-dir   -> ../../outside            (escapes the root)
//!     links/escape-file  -> ../../outside/secret.txt (escapes the root)
//!     links/broken       -> no-such-target           (target missing)
//!     links/in-root      -> ../control               (stays inside)
//!     breadth/f000000 .. f<N-1>                      (N entries)
//!     depth/d/d/d/... /leaf.txt                      (DEPTH_LEVELS deep)
//!     unreadable/inside.txt                          (mode 000)
//! ```

// Shared with the standalone measurement crate, which uses accessors this
// crate's tests do not.
#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// U+202E RIGHT-TO-LEFT OVERRIDE inside a name: rendered raw, this
/// displays as `invoiceexe.pdf` -- an executable dressed as a document.
pub const BIDI_NAME: &str = "invoice\u{202E}fdp.exe";
/// A newline inside a name: rendered raw it splits one row into two.
pub const NEWLINE_NAME: &str = "two\nlines.txt";
/// Directory nesting, in levels of one-byte names. 1500 levels is ~3 KB of
/// path -- deep enough to find any recursion limit, inside `PATH_MAX`.
pub const DEPTH_LEVELS: usize = 1500;
/// The breadth row's size, named by RFC-052 D8.
pub const BREADTH_ENTRIES: usize = 100_000;

/// A name whose bytes are not valid UTF-8 (legal on Linux; only `/` and NUL
/// are forbidden).
pub fn non_utf8_name() -> OsString {
    OsString::from_vec(b"bad-\xFF\xFE-name.txt".to_vec())
}

pub struct HostileFixture {
    pub base: PathBuf,
    pub project: PathBuf,
    pub outside: PathBuf,
}

impl HostileFixture {
    /// Builds every row. `breadth` is the entry count for the `breadth/`
    /// directory: [`BREADTH_ENTRIES`] for the real row, a small number for a
    /// test that only needs the other rows.
    pub fn build(label: &str, breadth: usize) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "tekstide-hostile-{label}-{}-{nonce}",
            std::process::id()
        ));
        let project = base.join("project");
        let outside = base.join("outside");

        fs::create_dir_all(outside.join("nested")).unwrap();
        fs::write(outside.join("secret.txt"), b"outside the project root\n").unwrap();
        fs::write(outside.join("nested/inner.txt"), b"also outside\n").unwrap();

        // control: an ordinary small tree that must render correctly.
        fs::create_dir_all(project.join("control/src/nested")).unwrap();
        fs::write(project.join("control/README.md"), b"# control\n").unwrap();
        fs::write(project.join("control/src/lib.rs"), b"pub fn f() {}\n").unwrap();
        fs::write(project.join("control/src/nested/deep.txt"), b"deep\n").unwrap();

        // escaping: three names a renderer must not draw raw.
        let escaping = project.join("escaping");
        fs::create_dir_all(&escaping).unwrap();
        fs::write(escaping.join(BIDI_NAME), b"bidi\n").unwrap();
        fs::write(escaping.join(NEWLINE_NAME), b"newline\n").unwrap();
        fs::write(escaping.join(non_utf8_name()), b"not utf-8\n").unwrap();

        // links: two that escape the root, one broken, one that stays in.
        let links = project.join("links");
        fs::create_dir_all(&links).unwrap();
        symlink("../../outside", links.join("escape-dir")).unwrap();
        symlink("../../outside/secret.txt", links.join("escape-file")).unwrap();
        symlink("no-such-target", links.join("broken")).unwrap();
        symlink("../control", links.join("in-root")).unwrap();

        // breadth: N entries in one directory.
        let breadth_dir = project.join("breadth");
        fs::create_dir_all(&breadth_dir).unwrap();
        for index in 0..breadth {
            fs::File::create(breadth_dir.join(format!("f{index:06}"))).unwrap();
        }

        // depth: DEPTH_LEVELS single-byte directories, a file at the bottom.
        let mut deep = project.join("depth");
        fs::create_dir_all(&deep).unwrap();
        deep.extend(std::iter::repeat_n("d", DEPTH_LEVELS));
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("leaf.txt"), b"bottom\n").unwrap();

        // unreadable: a directory that refuses a read.
        let unreadable = project.join("unreadable");
        fs::create_dir_all(&unreadable).unwrap();
        fs::write(unreadable.join("inside.txt"), b"hidden\n").unwrap();
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();

        Self {
            base,
            project,
            outside,
        }
    }

    pub fn escape_dir(&self) -> PathBuf {
        self.project.join("links/escape-dir")
    }
    pub fn escape_file(&self) -> PathBuf {
        self.project.join("links/escape-file")
    }
    pub fn broken_link(&self) -> PathBuf {
        self.project.join("links/broken")
    }
    pub fn in_root_link(&self) -> PathBuf {
        self.project.join("links/in-root")
    }
    pub fn escaping_dir(&self) -> PathBuf {
        self.project.join("escaping")
    }
    pub fn breadth_dir(&self) -> PathBuf {
        self.project.join("breadth")
    }
    pub fn depth_dir(&self) -> PathBuf {
        self.project.join("depth")
    }
    pub fn unreadable_dir(&self) -> PathBuf {
        self.project.join("unreadable")
    }
    pub fn control_dir(&self) -> PathBuf {
        self.project.join("control")
    }
}

impl Drop for HostileFixture {
    fn drop(&mut self) {
        // The unreadable directory must be readable again or removal fails.
        let _ = fs::set_permissions(self.unreadable_dir(), fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&self.base);
    }
}

/// True when `path` is inside `root` **by the filesystem**, not by spelling:
/// both sides canonicalized. A symlink whose spelling is under the root but
/// whose target is not is *outside*.
pub fn is_inside(root: &Path, path: &Path) -> bool {
    match (fs::canonicalize(root), fs::canonicalize(path)) {
        (Ok(root), Ok(path)) => path.starts_with(root),
        _ => false,
    }
}
