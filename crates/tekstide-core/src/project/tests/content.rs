use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::content::TextDocumentState;
use crate::project::{ProjectContentStatus, ProjectId, ProjectSession};

fn test_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("tekstide-{name}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&root).expect("test root should be created");
    root
}

fn cleanup_root(root: PathBuf) {
    let _ = std::fs::remove_dir_all(root);
}

fn project_at(root: &std::path::Path) -> ProjectSession {
    ProjectSession::new(ProjectId::for_test(1), "Project", root, root)
}

/// **status-mapping-honesty-fixes, Fix 2's own required proof**: a real
/// file, changed on disk after a *clean* document opened it (no local
/// edits, so nothing would be lost), reports `ExternalChanged` -- not
/// `Conflict` -- once fixed. Mirrors RFC-019 PR-019-D's own "real file,
/// real external write, real operation" shape; no `SaveDecision` or
/// `TextDocumentSaveError` value is synthesised anywhere in this test.
#[test]
fn a_clean_document_saved_over_a_real_external_change_reports_external_changed_not_conflict() {
    let root = test_root("content-clean-external-change");
    std::fs::write(root.join("note.txt"), "original\n").expect("fixture file should be written");
    let mut project = project_at(&root);
    project
        .open_text_document("note.txt")
        .expect("a clean document should open");

    // No local edit -- the document stays Clean.
    std::fs::write(root.join("note.txt"), "external\n").expect("external write should succeed");

    project
        .save_active_text_document()
        .expect_err("a save over a real external change must be refused");

    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::ExternalChanged,
        "no local edits existed, so the document's own state must not claim a conflict"
    );
    assert_eq!(
        project.content_workspace().status(),
        &ProjectContentStatus::ExternalChanged,
        "the workspace-level status this slice fixes must agree with the document's own state, \
         not report the more alarming Conflict for a change that lost nothing"
    );

    cleanup_root(root);
}

/// **The genuine-conflict case must still report `Conflict`** -- the risk
/// in narrowing a status is over-narrowing it. A dirty buffer really
/// would lose a local edit on top of the real external write below.
#[test]
fn a_dirty_document_saved_over_a_real_external_change_still_reports_conflict() {
    let root = test_root("content-genuine-conflict");
    std::fs::write(root.join("note.txt"), "original\n").expect("fixture file should be written");
    let mut project = project_at(&root);
    project
        .open_text_document("note.txt")
        .expect("a clean document should open");
    project
        .replace_active_text("local edit\n")
        .expect("a local edit should be accepted");

    std::fs::write(root.join("note.txt"), "external\n").expect("external write should succeed");

    project
        .save_active_text_document()
        .expect_err("a save over a real external change must be refused");

    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::Conflict,
        "a dirty buffer really would lose the local edit -- this must still read Conflict"
    );
    assert_eq!(
        project.content_workspace().status(),
        &ProjectContentStatus::Conflict,
        "the fix must not weaken the genuine-conflict case while fixing the clean-change one"
    );

    cleanup_root(root);
}
