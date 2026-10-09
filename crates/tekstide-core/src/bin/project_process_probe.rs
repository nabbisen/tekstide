//! RFC-058 PR-058-A: a real second Tekstide process, for the reproduction.
//!
//! **This is a test-and-proof artifact, not a product feature.** It exists so
//! PR-058-A's clobber can be watched happen between two real OS processes
//! rather than reasoned about by reading `app.rs` and `recovery/record.rs`
//! (D2's own standard, the RFC-066 D5 lesson applied before the fact). It
//! drives exactly the production sequence a real boot already runs --
//! `RecentProjectStore::load_or_recover`, `ApplicationShell::
//! add_project_from_path`, `RecentProjectStore::save`,
//! `tekstide_core::recovery::write_recovery_record` -- never a
//! reimplementation of any of them.
//!
//! # Usage
//!
//! ```text
//! project_process_probe <state-root> <project-root> <relative-path> <text> [--hold]
//! ```
//!
//! `<state-root>` stands in for `AppStatePathProvider::linux_default()`'s own
//! resolved directory (a real boot reads `XDG_STATE_HOME`/`HOME`; this
//! program takes the resolved path directly, so a test harness controls it
//! without touching either process's environment). `<project-root>` is the
//! real project directory both processes open. `<relative-path>` and
//! `<text>` become a dirtied document's own recovery record, built the same
//! shape `tekstide/src/shell.rs`'s own `recovery_record_for` uses (cursor and
//! viewport at the origin -- nothing here depends on either).
//!
//! Prints the project id this process ended up with to stdout as soon as its
//! own write has landed, flushed immediately, then: without `--hold`, exits
//! at once; with `--hold`, blocks reading stdin until it sees EOF before
//! exiting. **`--hold` is what makes "two real processes" mean two
//! *overlapping* ones** (review 513) -- a harness that reads this process's
//! id line first knows the write already happened and this process has not
//! exited, and can then start a second probe while this one is still alive,
//! holding it open exactly as long as the test needs by not yet closing its
//! stdin. Exits non-zero with a message on stderr on any failure.
use std::io::{BufRead, Write};
use std::path::PathBuf;

use tekstide_core::project::recent::{AppStatePathProvider, RecentProjectStore};
use tekstide_core::recovery::{
    RecoveryFileSnapshot, RecoveryRecord, RecoveryRetentionLimits, write_recovery_record,
};
use tekstide_core::shell::ApplicationShell;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let hold = args
        .iter()
        .position(|arg| arg == "--hold")
        .map(|index| args.remove(index))
        .is_some();
    let [state_root, project_root, relative_path, text] = args.as_slice() else {
        eprintln!(
            "usage: project_process_probe <state-root> <project-root> <relative-path> <text> \
             [--hold]"
        );
        std::process::exit(2);
    };
    let state_root = PathBuf::from(state_root);
    let project_root = PathBuf::from(project_root);

    let path_provider = AppStatePathProvider::from_state_dir(state_root.clone());
    let mut store = RecentProjectStore::new(path_provider);
    let loaded = store.load_or_recover();

    let mut app_shell = ApplicationShell::new();
    app_shell.restore_recent_projects(loaded.state);

    let outcome = match app_shell.add_project_from_path(&project_root) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("project_process_probe: could not open {project_root:?}: {error}");
            std::process::exit(3);
        }
    };
    let project_id = outcome.project_id().clone();

    // The same save-after-open boot already performs (`main.rs`'s own
    // `boot()`, after its CLI-argument loop) -- without this, the *other*
    // process would never see the id this one just reused or minted, and
    // the two would not collide at all.
    if let Err(error) = store.save(&app_shell.recent_project_state()) {
        eprintln!("project_process_probe: could not save recent projects: {error}");
        std::process::exit(4);
    }

    let text_bytes = text.len() as u64;
    let record = RecoveryRecord {
        version: tekstide_core::recovery::RECOVERY_RECORD_VERSION,
        relative_path: relative_path.clone(),
        text: text.clone(),
        cursor_line: 0,
        cursor_column: 0,
        viewport_first_visible_line: 0,
        viewport_first_visible_column: 0,
        snapshot: RecoveryFileSnapshot::from_system_time(
            &project_root.join(relative_path),
            std::time::SystemTime::now(),
            text_bytes,
        ),
    };
    if let Err(error) = write_recovery_record(
        &state_root,
        project_id.as_str(),
        &record,
        RecoveryRetentionLimits::default_limits(),
    ) {
        eprintln!("project_process_probe: could not write recovery record: {error:?}");
        std::process::exit(5);
    }

    println!("{}", project_id.as_str());
    let _ = std::io::stdout().flush();

    if hold {
        // Blocks until the harness closes (or drops) this process's stdin --
        // the signal that it has seen this line and finished whatever it
        // needed this process to still be alive for. A loop of `read_line`
        // calls, each discarded, rather than one `read_to_end`: this is
        // draining a pipe to its close, not reading a file's full content,
        // and a raw-full-file-read scan
        // (`project::diff::tests::enumeration_confirms_only_the_closed_list_reads_full_file_content`)
        // cannot tell those two apart by the text of the call alone, so it
        // is written in a shape that is not even a candidate for the
        // question that scan asks.
        let stdin = std::io::stdin();
        let mut discard = String::new();
        while stdin.lock().read_line(&mut discard).unwrap_or(0) > 0 {
            discard.clear();
        }
    }
}
