use super::{TestSandbox, root_handle, validate};
use crate::content::{
    TextCursor, TextDocument, TextDocumentEditError, TextDocumentOpenPolicy, TextDocumentState,
    TextViewport,
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
    });
    document.replace_text("changed\n").unwrap();

    assert_eq!(document.text(), "changed\n");
    assert_eq!(document.state(), TextDocumentState::Dirty);
    assert!(document.is_dirty());
    assert_eq!(document.cursor(), TextCursor { line: 3, column: 5 });
    assert_eq!(
        document.viewport(),
        TextViewport {
            first_visible_line: 2
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
