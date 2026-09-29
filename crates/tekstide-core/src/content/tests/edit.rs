use super::{TestSandbox, root_handle, validate};
use crate::content::{
    EditOperation, TextCursor, TextDocument, TextDocumentEditError, TextDocumentOpenPolicy,
    TextDocumentState, TextViewport, UNDO_MAX_DEPTH,
};
use crate::project::ProjectId;
use std::fs;

#[test]
fn edit_transitions_document_to_dirty_without_saving() {
    let sandbox = TestSandbox::new("text-edit-dirty");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/src/lib.rs", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "src/lib.rs", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    document.set_cursor(TextCursor { line: 3, column: 5 });
    document.set_viewport(TextViewport {
        first_visible_line: 2,
        first_visible_column: 0,
    });
    document.replace_text("changed\n").unwrap();

    assert_eq!(document.text(), "changed\n");
    assert_eq!(document.state(), TextDocumentState::Dirty);
    assert!(document.is_dirty());
    assert_eq!(document.cursor(), TextCursor { line: 3, column: 5 });
    assert_eq!(
        document.viewport(),
        TextViewport {
            first_visible_line: 2,
            first_visible_column: 0,
        }
    );
    assert_eq!(
        fs::read_to_string(project_dir.join("src/lib.rs")).unwrap(),
        "original\n",
        "PR-006-C must not save edited buffers"
    );
}

#[test]
fn replacing_text_with_same_contents_keeps_clean_document_clean() {
    let sandbox = TestSandbox::new("text-edit-same");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"same\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    document.replace_text("same\n").unwrap();

    assert_eq!(document.state(), TextDocumentState::Clean);
}

#[test]
fn replacing_text_with_nul_is_rejected_and_keeps_existing_buffer() {
    let sandbox = TestSandbox::new("text-edit-nul");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    let error = document
        .replace_text("changed\0text")
        .expect_err("NUL-containing replacement text should be rejected");

    assert_eq!(error, TextDocumentEditError::ContainsNul);
    assert_eq!(document.text(), "original\n");
    assert_eq!(document.state(), TextDocumentState::Clean);
}

/// RFC-057 D3, risk document §3 row two: text that changed away from the
/// opened content and then back to it returns the document to `Clean` --
/// not permanently `Dirty` because it changed at some point, the gap
/// `replace_text`'s one-way latch left before this fix.
#[test]
fn replacing_text_back_to_the_opened_content_returns_to_clean() {
    let sandbox = TestSandbox::new("text-edit-back-to-clean");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    document.replace_text("changed\n").unwrap();
    assert_eq!(document.state(), TextDocumentState::Dirty);

    document.replace_text("original\n").unwrap();
    assert_eq!(
        document.state(),
        TextDocumentState::Clean,
        "text back to exactly what was opened must return to Clean, not stay Dirty"
    );
}

/// The same fix, guarded correctly: a document already in
/// `ExternalChanged`/`Conflict`/`SaveError` must not be silently flipped
/// back to `Clean` just because its text happens to match what was
/// opened -- each of those states needs its own resolution (the reload
/// dialog, or a retried save), never a coincidence of content.
#[test]
fn replacing_text_back_to_the_opened_content_does_not_override_an_external_change_state() {
    let sandbox = TestSandbox::new("text-edit-back-to-clean-guarded");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    document.replace_text("changed\n").unwrap();
    fs::write(project_dir.join("file.txt"), "external\n").unwrap();
    let policy = TextDocumentOpenPolicy::linux_mvp();
    let decision = document.refresh_external_state(&root, policy).unwrap();
    assert_eq!(
        decision,
        crate::content::ExternalChangeDecision::Conflict,
        "test precondition: a dirty document with a real external write under it becomes Conflict"
    );
    assert_eq!(document.state(), TextDocumentState::Conflict);

    document.replace_text("original\n").unwrap();
    assert_eq!(
        document.state(),
        TextDocumentState::Conflict,
        "a Conflict must not be silently cleared just because the text coincided with what was \
         originally opened -- it still needs its own resolution (the reload dialog)"
    );
}

/// RFC-057 D3: `record_edit_operation`/`undo_operation`/`redo_operation`'s
/// own stack semantics -- undo pops what redo pushes back, and a real new
/// edit (not undo/redo itself) abandons the old redo future, the rule
/// every text editor uses.
#[test]
fn recording_an_edit_after_an_undo_clears_the_redo_stack() {
    let sandbox = TestSandbox::new("text-edit-undo-redo-stack");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    let op = EditOperation::Insert {
        at: TextCursor::default(),
        inserted: "x".to_owned(),
    };
    document.record_edit_operation(op.clone());
    assert!(document.can_undo());
    assert!(!document.can_redo());

    assert_eq!(document.undo_operation(), Some(op));
    assert!(!document.can_undo());
    assert!(document.can_redo());

    document.record_edit_operation(EditOperation::Insert {
        at: TextCursor::default(),
        inserted: "y".to_owned(),
    });
    assert!(
        !document.can_redo(),
        "a real new edit after an undo must abandon the old redo future"
    );
}

/// RFC-057 D3, risk document §3 row one: bounded depth, **stated when
/// reached** -- past `UNDO_MAX_DEPTH` the oldest recorded edit is
/// dropped and `undo_depth_bound_reached` latches true, staying true even
/// once the stack has room again (the dropped edit does not come back).
#[test]
fn recording_past_the_bound_drops_the_oldest_edit_and_latches_the_reached_flag() {
    let sandbox = TestSandbox::new("text-edit-undo-bound");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    for index in 0..UNDO_MAX_DEPTH {
        document.record_edit_operation(EditOperation::Insert {
            at: TextCursor::default(),
            inserted: index.to_string(),
        });
    }
    assert!(
        !document.undo_depth_bound_reached(),
        "exactly at the bound, nothing has been dropped yet"
    );

    document.record_edit_operation(EditOperation::Insert {
        at: TextCursor::default(),
        inserted: "one past the bound".to_owned(),
    });
    assert!(
        document.undo_depth_bound_reached(),
        "one edit past the bound must drop the oldest and say so"
    );

    // The oldest ("0") is gone; undoing `UNDO_MAX_DEPTH` times reaches
    // "1", the second-recorded edit, never "0".
    let mut last = None;
    for _ in 0..UNDO_MAX_DEPTH {
        last = document.undo_operation();
    }
    assert_eq!(
        last,
        Some(EditOperation::Insert {
            at: TextCursor::default(),
            inserted: "1".to_owned(),
        }),
        "the oldest recorded edit (\"0\") must have been dropped, not merely uncounted"
    );
    assert!(!document.can_undo());

    // Undoing back below the bound must not un-latch the flag: the
    // dropped edit stays dropped.
    assert!(document.undo_depth_bound_reached());
}
