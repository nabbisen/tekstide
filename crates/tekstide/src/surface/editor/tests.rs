use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tekstide_core::content::{TextDocument, TextDocumentOpenPolicy, TextDocumentState};
use tekstide_core::project::root::{
    ProjectRootHandle, ProjectRootValidator, SymlinkPolicy, ValidProjectRoot,
};
use tekstide_core::project::{ProjectContentStatus, ProjectId, ProjectSession};

use super::{body_text, chrome_line, document_state_symbol, empty_lines, open_error_line};
use crate::i18n::{Catalog, LocalePreference};

fn real_locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn real_catalog() -> Catalog {
    Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()))
}

/// A real, temp-dir-backed project root -- `TextDocument`'s only
/// constructor is `open()`, which needs a real `ProjectRootHandle`, the
/// same reason `tekstide-core`'s own `content::tests` uses this exact
/// shape.
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tekstide-editor-surface-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        Self { root }
    }

    fn write_file(&self, relative_path: &str, contents: &str) {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, contents).unwrap();
    }

    fn root_handle(&self) -> ProjectRootHandle {
        let valid: ValidProjectRoot = ProjectRootValidator
            .validate(&self.root, SymlinkPolicy::FailClosed)
            .expect("sandbox root must validate");
        let project = ProjectSession::new(
            ProjectId::new_uuid(),
            "editor-surface-fixture",
            valid.selected_path,
            valid.canonical_path,
        );
        ProjectRootHandle::from_project_session(&project)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn open(sandbox: &Sandbox, relative_path: &str) -> TextDocument {
    TextDocument::open(
        &sandbox.root_handle(),
        relative_path,
        TextDocumentOpenPolicy::default(),
    )
    .expect("fixture file must open")
}

/// **The text area renders raw.** A document containing `U+202E` must
/// show the raw character -- the opposite property from every other
/// surface in this crate, per RFC-016's editor exception.
#[test]
fn body_text_preserves_a_bidi_override_character_raw() {
    let sandbox = Sandbox::new("raw-bidi");
    sandbox.write_file("evil.txt", "echo proj\u{202E}gpj.exe");
    let document = open(&sandbox, "evil.txt");

    let body = body_text(&document);

    assert!(
        body.contains('\u{202E}'),
        "the raw override character must survive unescaped, got {body:?}"
    );
    assert!(
        !body.contains("<U+202E>"),
        "the text area must never contain an escape marker, got {body:?}"
    );
    assert_eq!(body, "echo proj\u{202E}gpj.exe");
}

/// **Ablated in the opposite direction from every other surface's own
/// bidi test**: a test asserting the escaped form appears in the text
/// area must fail. Verified by construction here rather than by
/// temporarily editing `body_text` -- `quote_untrusted` is simply never
/// called by [`body_text`], so there is no code path that could produce
/// an escape marker in its output to assert against. The manual ablation
/// (wrapping `body_text`'s return in `quote_untrusted` and re-running
/// [`body_text_preserves_a_bidi_override_character_raw`]) was performed
/// once during review and reverted; recorded in `qa-evidence.md` rather
/// than kept as a second permanent test, since a permanently-failing
/// assertion cannot itself be part of the passing suite.
#[test]
fn asserting_the_escaped_form_would_fail_because_body_text_never_escapes() {
    let sandbox = Sandbox::new("no-escape-path");
    sandbox.write_file("evil.txt", "proj\u{202E}gpj.exe");
    let document = open(&sandbox, "evil.txt");

    let body = body_text(&document);
    assert!(
        !body.contains("<U+202E>"),
        "body_text must never produce the escape marker this crate's other surfaces do"
    );
}

/// A freshly opened document is `Clean` -- the only state reachable
/// without editing, which this read-only slice does not do. The other
/// four symbols are tested directly against [`document_state_symbol`]
/// below, since it is a pure function that does not need a real document
/// in each state to exercise.
#[test]
fn chrome_line_reports_clean_for_a_freshly_opened_document() {
    let sandbox = Sandbox::new("clean-state");
    sandbox.write_file("readme.md", "hello");
    let document = open(&sandbox, "readme.md");

    assert_eq!(document.state(), TextDocumentState::Clean);
    let line = chrome_line(&real_catalog(), &document);
    assert!(line.contains("readme.md"));
    assert!(!line.contains("unsaved"));
    assert!(!line.contains("changed on disk"));
    assert!(!line.contains("conflict"));
    assert!(!line.contains("save error"));
}

/// Every `TextDocumentState` variant maps to its own compile-time
/// symbol -- checked directly since `document_state_symbol` takes the
/// enum value itself, not a `TextDocument`, so no sandbox is needed to
/// exercise all five.
#[test]
fn every_document_state_maps_to_a_distinct_symbol() {
    let symbols = [
        document_state_symbol(TextDocumentState::Clean),
        document_state_symbol(TextDocumentState::Dirty),
        document_state_symbol(TextDocumentState::ExternalChanged),
        document_state_symbol(TextDocumentState::Conflict),
        document_state_symbol(TextDocumentState::SaveError),
    ];
    let unique: std::collections::HashSet<_> = symbols.iter().collect();
    assert_eq!(
        unique.len(),
        symbols.len(),
        "symbols must all be distinct: {symbols:?}"
    );
}

/// **The bidi-override case for chrome, tested specifically.** The
/// header shows the file's own path -- attacker-influenced, the same
/// class as an explorer node name -- and must render escaped.
#[test]
fn chrome_line_escapes_a_bidi_override_in_the_path() {
    let sandbox = Sandbox::new("bidi-path");
    sandbox.write_file("proj\u{202E}gpj.exe", "content");
    let document = open(&sandbox, "proj\u{202E}gpj.exe");

    let line = chrome_line(&real_catalog(), &document);

    assert!(
        line.contains("<U+202E>"),
        "expected the escaped marker in {line:?}"
    );
    assert!(
        !line.contains('\u{202E}'),
        "the raw override character must never reach the chrome line, got {line:?}"
    );
}

/// `TextDocumentOpenError`'s `Display` embeds the target's relative
/// path in every variant (including the 4 MiB refusal) -- escaped
/// exactly like the chrome path above.
#[test]
fn open_error_line_escapes_the_message() {
    let catalog = real_catalog();
    let line = open_error_line(
        &catalog,
        "file is too large to edit: proj\u{202E}gpj.exe is 5000000 bytes, limit is 4194304 bytes",
    )
    .expect("an OpenError message always renders a line");

    assert!(
        line.contains("<U+202E>"),
        "expected the escaped marker in {line:?}"
    );
    assert!(!line.contains('\u{202E}'));
    assert!(
        line.contains("4194304"),
        "the real policy bound must be visible, not a second one"
    );
}

/// **The 4 MiB refusal is rendered**, not silently empty, and uses the
/// real policy's own bound (not a second one this module introduces).
#[test]
fn opening_a_file_over_the_policy_bound_is_refused_and_rendered() {
    let sandbox = Sandbox::new("too-large");
    let oversized = "x".repeat(TextDocumentOpenPolicy::default().max_editable_bytes as usize + 1);
    sandbox.write_file("huge.txt", &oversized);

    let result = TextDocument::open(
        &sandbox.root_handle(),
        "huge.txt",
        TextDocumentOpenPolicy::default(),
    );
    let Err(error) = result else {
        panic!("opening an oversized file must be refused");
    };

    let catalog = real_catalog();
    let line = open_error_line(&catalog, &error.to_string()).expect("must render a line");
    assert!(line.contains("too large"));
    assert!(
        line.contains(
            &TextDocumentOpenPolicy::default()
                .max_editable_bytes
                .to_string()
        ),
        "the real policy bound must appear: {line:?}"
    );
}

/// No document open, no error -- the plain empty-state notice, not a
/// blank view.
#[test]
fn no_document_and_no_error_renders_the_empty_notice() {
    let catalog = real_catalog();
    let lines = empty_lines(&catalog, &ProjectContentStatus::Empty);
    assert_eq!(lines, vec![catalog.get("editor-empty")]);
}

/// **No `text_document_state_label` call anywhere in this module.** The
/// fourth of RFC-019's four named hardcoded-English producers, carried
/// forward from PR-019-B as this slice's obligation to discharge.
/// Checked by a source-text scan, the same shape
/// `no_hardcoded_english_label_function_is_called_in_this_module` uses
/// for the other three.
#[test]
fn no_hardcoded_english_label_function_is_called_in_this_module() {
    let source = include_str!("../editor.rs");
    assert!(
        !source.contains("text_document_state_label("),
        "text_document_state_label must not be called in surface/editor.rs -- route through \
         Catalog instead"
    );
}
