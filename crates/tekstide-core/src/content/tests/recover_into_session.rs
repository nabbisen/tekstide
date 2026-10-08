use super::{TestSandbox, validate};
use crate::content::{RecoveredBufferInit, TextCursor, TextViewport};
use crate::project::{ProjectContentError, ProjectId, ProjectResourceLimits, ProjectSession};

fn recovered(text: &str) -> RecoveredBufferInit {
    RecoveredBufferInit {
        text: text.to_owned(),
        cursor: TextCursor::default(),
        viewport: TextViewport::default(),
        recorded_modified_at: std::time::SystemTime::UNIX_EPOCH,
        recorded_len: 0,
    }
}

fn session(sandbox: &TestSandbox) -> ProjectSession {
    let project_dir = sandbox.create_dir("project");
    let root = validate(&project_dir);
    ProjectSession::new(
        ProjectId::for_test(1),
        root.display_name,
        root.selected_path,
        root.canonical_path,
    )
}

/// RFC-027 PR-027-C: `ProjectSession::recover_text_document` inserts into the open set
/// without activating it (D1/D2's own distinction between the open set and what is
/// active) -- `open_surface`/`mode` are untouched, unlike `open_text_document`, which
/// deliberately does switch them.
#[test]
fn recover_text_document_inserts_without_activating() {
    let sandbox = TestSandbox::new("text-recover-into-session");
    sandbox.create_file_with_contents("project/notes.txt", b"saved\n");
    let mut project = session(&sandbox);
    let surface_before = project.open_surface();
    let mode_before = project.mode();

    project
        .recover_text_document("notes.txt", recovered("unsaved edit\n"))
        .expect("recovering a fresh path must succeed");

    assert_eq!(
        project.content_workspace().open_documents().count(),
        1,
        "the recovered buffer must land in the open set"
    );
    assert_eq!(
        project.open_surface(),
        surface_before,
        "recovering must not switch the open surface"
    );
    assert_eq!(
        project.mode(),
        mode_before,
        "recovering must not switch the project mode"
    );
}

/// RFC-027 PR-027-C: recovering over a path that is already open is refused, not applied
/// -- the already-open entry may hold different text than the record (the user could
/// have reopened and re-edited it since restart), so silently overwriting it would be
/// exactly the cross-document leak RFC-065's own repair was about not having.
#[test]
fn recover_text_document_refuses_a_path_already_open() {
    let sandbox = TestSandbox::new("text-recover-already-open");
    sandbox.create_file_with_contents("project/notes.txt", b"saved\n");
    let mut project = session(&sandbox);
    project
        .open_text_document("notes.txt")
        .expect("opening the real file must succeed");

    let error = project
        .recover_text_document("notes.txt", recovered("unsaved edit\n"))
        .expect_err("recovering over an already-open path must be refused");

    assert!(matches!(
        error,
        ProjectContentError::RecoveryPathAlreadyOpen { .. }
    ));
    assert_eq!(
        project.content_workspace().open_documents().count(),
        1,
        "the refused recovery must not add a second entry for the same path"
    );
}

/// RFC-027 PR-027-C: the open set's own bound applies to recovery exactly as it applies
/// to an ordinary open (RFC-065 D4) -- checked before any disk read, the same order
/// `recover_text_document`'s own doc states.
#[test]
fn recover_text_document_refuses_past_the_open_set_limit() {
    let sandbox = TestSandbox::new("text-recover-at-limit");
    sandbox.create_file_with_contents("project/already-open.txt", b"saved\n");
    sandbox.create_file_with_contents("project/recovered.txt", b"saved\n");
    let mut project = session(&sandbox);
    project.set_resource_limits(ProjectResourceLimits {
        open_document_limit: Some(1),
        ..ProjectResourceLimits::default()
    });
    project
        .open_text_document("already-open.txt")
        .expect("opening the first document must succeed, filling the limit");

    let error = project
        .recover_text_document("recovered.txt", recovered("unsaved edit\n"))
        .expect_err("recovering a new path past the limit must be refused");

    assert!(matches!(
        error,
        ProjectContentError::OpenSetAtLimit { open: 1, limit: 1 }
    ));
}
