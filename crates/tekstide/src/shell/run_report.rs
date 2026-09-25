//! RFC-056 PR-056-D: the controls on the AgentRun Report — classify the run,
//! write notes, export the report — and the keys that reach them.
//!
//! **Reachable by a person, with no environment variable** (D10): every action
//! here is a visible button *and* a key, the same shape the Change Review
//! decision buttons have, and nothing here is gated on anything but the run
//! existing.
//!
//! **Nothing but a person's keystrokes or paste writes the notes** (D4). The
//! only path from this module to `AgentRun` notes is `field_submit`'s call to
//! `ProjectSession::set_agent_run_notes` with the text the user typed;
//! `the_notes_of_a_run_are_written_from_one_place_and_nothing_else_writes_them`
//! holds every other caller of that setter, and of `AgentRun::set_notes`, to
//! the crate that owns them.

use super::*;

use tekstide_core::agent::{
    REPORT_MAX_CHANGED_PATHS, ReportExportRefusal, RunReportSources, TranscriptExcerpt,
    render_run_report, run_report_contents, write_run_report,
};
use tekstide_core::domain::{
    AgentRun, AgentRunId, RUN_CUSTOM_CLASSIFICATION_MAX_CHARS, RUN_NOTES_MAX_CHARS,
    RunClassification,
};
use tekstide_core::project::{ProjectId, RunAnnotationError, RunRecordWrite};

/// Which line of text the field is collecting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RunReportFieldKind {
    CustomClassification,
    Notes,
    ExportPath,
}

/// The one open text field on the report. A field is a buffer and a purpose:
/// nothing is applied until it is submitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunReportField {
    pub(super) kind: RunReportFieldKind,
    pub(super) buffer: String,
    /// For an export: what the file will contain, read **once, when the field
    /// opened**, so rendering never reads a transcript.
    pub(super) export_contents: Option<tekstide_core::agent::RunReportContents>,
}

/// What the last action said, shown until the next one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RunReportNotice {
    Exported {
        path: String,
        bytes: u64,
    },
    ExportRefused(ReportExportRefusal),
    BlankLabel,
    /// The run has no record folder, so the change lives in memory only.
    NotRecorded,
    /// The record could not be written; the next pass tries again.
    RecordWriteFailed,
}

fn field_cap(kind: RunReportFieldKind) -> usize {
    match kind {
        RunReportFieldKind::CustomClassification => RUN_CUSTOM_CLASSIFICATION_MAX_CHARS,
        RunReportFieldKind::Notes => RUN_NOTES_MAX_CHARS,
        RunReportFieldKind::ExportPath => MAX_PATH_FIELD_CHARS,
    }
}

/// The report's subject: what `Ctrl+Alt+R` opens and the view renders.
fn displayed_run(state: &State) -> Option<(ProjectId, AgentRunId)> {
    let project = state.app_shell.state().active_project()?;
    (project.mode() == ProjectMode::Content
        && project.open_surface() == ProjectOpenSurface::AgentRunDetail)
        .then_some(())?;
    let run = project.latest_agent_run_for_display()?;
    Some((project.id().clone(), run.id.clone()))
}

// ---- keys -------------------------------------------------------------------

/// A `MainArea` consumer, like every other surface's: it checks the open
/// surface itself, so it can never read a key meant for another.
pub(super) fn handle_agent_run_report_key(
    state: &mut State,
    key: &input::KeyPress,
) -> Task<Message> {
    if state.modal.is_some() || displayed_run(state).is_none() {
        return Task::none();
    }
    if state.run_report_field.is_some() {
        return field_key(state, key);
    }
    if key.modifiers.control() || key.modifiers.alt() {
        return Task::none();
    }
    if let keyboard::Key::Character(character) = &key.key {
        match character.as_str() {
            "1" => classify(state, Some(RunClassification::Coding)),
            "2" => classify(state, Some(RunClassification::Review)),
            "3" => classify(state, Some(RunClassification::Documentation)),
            "4" => classify(state, Some(RunClassification::Testing)),
            "5" => classify(state, Some(RunClassification::Refactoring)),
            "6" => classify(state, Some(RunClassification::Release)),
            "x" => classify(state, None),
            "c" => open_field(state, RunReportFieldKind::CustomClassification),
            "n" => open_field(state, RunReportFieldKind::Notes),
            "e" => open_field(state, RunReportFieldKind::ExportPath),
            _ => {}
        }
    }
    Task::none()
}

fn field_key(state: &mut State, key: &input::KeyPress) -> Task<Message> {
    match &key.key {
        keyboard::Key::Character(typed) => {
            if key.modifiers.control() && typed.as_ref() == "v" {
                return iced::clipboard::read().map(Message::RunReportFieldPasteResolved);
            }
            if !key.modifiers.control() && !key.modifiers.alt() {
                push_to_field(state, typed);
            }
        }
        keyboard::Key::Named(keyboard::key::Named::Space) => push_to_field(state, " "),
        keyboard::Key::Named(keyboard::key::Named::Backspace) => {
            if let Some(field) = state.run_report_field.as_mut() {
                field.buffer.pop();
            }
        }
        keyboard::Key::Named(keyboard::key::Named::Enter) => {
            let starts_a_new_line = state
                .run_report_field
                .as_ref()
                .is_some_and(|field| field.kind == RunReportFieldKind::Notes)
                && key.modifiers.shift();
            if starts_a_new_line {
                push_to_field(state, "\n");
            } else {
                field_submit(state);
            }
        }
        keyboard::Key::Named(keyboard::key::Named::Escape) => field_cancel(state),
        _ => {}
    }
    Task::none()
}

/// The one place a field grows, from typing or from a paste: the cap is
/// enforced once. A line-shaped field takes no newline; a paste's `\r\n` is a
/// newline like any other.
fn push_to_field(state: &mut State, text: &str) {
    let Some(field) = state.run_report_field.as_mut() else {
        return;
    };
    let multi_line = field.kind == RunReportFieldKind::Notes;
    let remaining = field_cap(field.kind).saturating_sub(field.buffer.chars().count());
    let normalised = text.replace("\r\n", "\n").replace('\r', "\n");
    field.buffer.extend(
        normalised
            .chars()
            .filter(|character| multi_line || *character != '\n')
            .take(remaining),
    );
}

pub(super) fn field_paste_resolved(state: &mut State, content: Option<String>) {
    if let Some(content) = content {
        push_to_field(state, &content);
    }
}

// ---- actions ----------------------------------------------------------------

pub(super) fn classify(state: &mut State, classification: Option<RunClassification>) {
    if state.modal.is_some() {
        return;
    }
    let Some((project_id, run_id)) = displayed_run(state) else {
        return;
    };
    state.run_report_notice = None;
    let outcome = state
        .app_shell
        .state_mut()
        .project_mut(&project_id)
        .map(|project| project.set_agent_run_classification(&run_id, classification));
    apply_outcome(state, outcome);
}

pub(super) fn open_field(state: &mut State, kind: RunReportFieldKind) {
    if state.modal.is_some() {
        return;
    }
    let Some((project_id, run_id)) = displayed_run(state) else {
        return;
    };
    let Some(project) = state.app_shell.state().project(&project_id) else {
        return;
    };
    let Some(run) = project.agent_run_or_restored(&run_id) else {
        return;
    };
    let (buffer, export_contents) = match kind {
        RunReportFieldKind::Notes => (run.notes().unwrap_or_default().to_owned(), None),
        RunReportFieldKind::CustomClassification => (
            match &run.classification {
                Some(RunClassification::Custom(label)) => label.clone(),
                _ => String::new(),
            },
            None,
        ),
        RunReportFieldKind::ExportPath => {
            let sources = gather_report_sources(project, run);
            let contents = run_report_contents(&sources.as_sources(run));
            (String::new(), Some(contents))
        }
    };
    state.run_report_notice = None;
    state.run_report_field = Some(RunReportField {
        kind,
        buffer,
        export_contents,
    });
}

pub(super) fn field_cancel(state: &mut State) {
    state.run_report_field = None;
}

pub(super) fn field_submit(state: &mut State) {
    if state.modal.is_some() {
        return;
    }
    let Some(field) = state.run_report_field.clone() else {
        return;
    };
    let Some((project_id, run_id)) = displayed_run(state) else {
        state.run_report_field = None;
        return;
    };
    state.run_report_notice = None;
    match field.kind {
        RunReportFieldKind::CustomClassification => {
            let outcome = state
                .app_shell
                .state_mut()
                .project_mut(&project_id)
                .map(|project| {
                    project.set_agent_run_classification(
                        &run_id,
                        Some(RunClassification::Custom(field.buffer.clone())),
                    )
                });
            if matches!(
                outcome,
                Some(Err(RunAnnotationError::BlankClassificationLabel))
            ) {
                // The field stays open: nothing was decided.
                state.run_report_notice = Some(RunReportNotice::BlankLabel);
                return;
            }
            state.run_report_field = None;
            apply_outcome(state, outcome);
        }
        RunReportFieldKind::Notes => {
            // **The text the user typed, and nothing else** (D4).
            let outcome = state
                .app_shell
                .state_mut()
                .project_mut(&project_id)
                .map(|project| project.set_agent_run_notes(&run_id, &field.buffer));
            state.run_report_field = None;
            apply_outcome(state, outcome);
        }
        RunReportFieldKind::ExportPath => {
            let state_root = resolve_agent_run_state_dir();
            export_report(state, &project_id, &run_id, &field.buffer, state_root);
        }
    }
}

fn apply_outcome(state: &mut State, outcome: Option<Result<RunRecordWrite, RunAnnotationError>>) {
    state.run_report_notice = match outcome {
        Some(Ok(RunRecordWrite::NoRunDirectory)) => Some(RunReportNotice::NotRecorded),
        Some(Ok(RunRecordWrite::Failed)) => Some(RunReportNotice::RecordWriteFailed),
        _ => None,
    };
}

// ---- the report --------------------------------------------------------------

/// What a report is made from, owned for the moment it is built and dropped
/// with it. Never stored (D5).
struct GatheredSources {
    changed_paths: Vec<std::path::PathBuf>,
    changed_paths_omitted: u64,
    transcript_tail: Option<(Vec<u8>, u64)>,
    generated_at: tekstide_core::domain::DomainTimestamp,
}

impl GatheredSources {
    fn as_sources<'a>(&'a self, run: &'a AgentRun) -> RunReportSources<'a> {
        RunReportSources {
            run,
            changed_paths: &self.changed_paths,
            changed_paths_omitted: self.changed_paths_omitted,
            transcript: self
                .transcript_tail
                .as_ref()
                .map(|(tail, total_len)| TranscriptExcerpt {
                    tail,
                    total_len: *total_len,
                }),
            generated_at: &self.generated_at,
        }
    }
}

fn gather_report_sources(
    project: &tekstide_core::project::ProjectSession,
    run: &AgentRun,
) -> GatheredSources {
    let (changed_paths, changed_paths_omitted) =
        project.changed_paths_of_run(&run.id, REPORT_MAX_CHANGED_PATHS);
    // Whatever transcript still exists: a run whose transcript retention
    // removed has none, and the report says so.
    let transcript_tail = agent_run_transcript_window(project, run)
        .ok()
        .map(|(_, window)| (window.content().to_vec(), window.total_len()));
    GatheredSources {
        changed_paths,
        changed_paths_omitted,
        transcript_tail,
        generated_at: tekstide_core::domain::DomainTimestamp::now_utc(),
    }
}

/// Builds the report **now**, writes it where the user asked, and keeps
/// nothing: the string is dropped when this returns, and the only thing left is
/// the notice saying where it went.
pub(super) fn export_report(
    state: &mut State,
    project_id: &ProjectId,
    run_id: &AgentRunId,
    destination_text: &str,
    state_root: Option<std::path::PathBuf>,
) {
    let Some(project) = state.app_shell.state().project(project_id) else {
        return;
    };
    let Some(run) = project.agent_run_or_restored(run_id) else {
        return;
    };
    let gathered = gather_report_sources(project, run);
    let report = render_run_report(&gathered.as_sources(run));
    let destination = std::path::PathBuf::from(destination_text.trim());
    let result = write_run_report(
        &destination,
        state_root.as_deref().unwrap_or(std::path::Path::new("")),
        &report,
    );
    match result {
        Ok(bytes) => {
            state.run_report_field = None;
            state.run_report_notice = Some(RunReportNotice::Exported {
                path: destination.display().to_string(),
                bytes,
            });
        }
        // The field stays open: the path can be corrected without retyping the
        // rest.
        Err(refusal) => state.run_report_notice = Some(RunReportNotice::ExportRefused(refusal)),
    }
}

// ---- view -------------------------------------------------------------------

fn classification_button<'a>(
    state: &'a State,
    current: Option<&RunClassification>,
    value: RunClassification,
    key: &str,
) -> Element<'a, Message> {
    let marker = if current == Some(&value) {
        "[x] "
    } else {
        "[ ] "
    };
    crate::theme::button(
        state.theme,
        text(format!("{marker}{}", state.catalog.get(key))).size(state.theme.font_size_body()),
    )
    .on_press(Message::RunReportClassifyPressed(Some(value)))
    .into()
}

fn plain_button<'a>(state: &'a State, key: &str, message: Message) -> Element<'a, Message> {
    crate::theme::button(
        state.theme,
        text(state.catalog.get(key)).size(state.theme.font_size_body()),
    )
    .on_press(message)
    .into()
}

/// The classification, notes and export controls, above the report's own
/// scrolling content so they stay in view.
pub(super) fn controls_view<'a>(state: &'a State, run: &'a AgentRun) -> Element<'a, Message> {
    let body = state.theme.font_size_body();
    let status = state.theme.font_size_status();
    let mut lines: Vec<Element<'a, Message>> = Vec::new();

    lines.push(
        text(state.catalog.get("agent-run-report-classification-heading"))
            .size(body)
            .into(),
    );
    let current = run.classification.as_ref();
    let custom_label = match current {
        Some(RunClassification::Custom(label)) => Some(label.as_str()),
        _ => None,
    };
    let custom_marker = if custom_label.is_some() {
        "[x] "
    } else {
        "[ ] "
    };
    let buttons: Vec<Element<'a, Message>> = vec![
        classification_button(
            state,
            current,
            RunClassification::Coding,
            "agent-run-report-class-coding",
        ),
        classification_button(
            state,
            current,
            RunClassification::Review,
            "agent-run-report-class-review",
        ),
        classification_button(
            state,
            current,
            RunClassification::Documentation,
            "agent-run-report-class-documentation",
        ),
        classification_button(
            state,
            current,
            RunClassification::Testing,
            "agent-run-report-class-testing",
        ),
        classification_button(
            state,
            current,
            RunClassification::Refactoring,
            "agent-run-report-class-refactoring",
        ),
        classification_button(
            state,
            current,
            RunClassification::Release,
            "agent-run-report-class-release",
        ),
        crate::theme::button(
            state.theme,
            text(format!(
                "{custom_marker}{}",
                state.catalog.get("agent-run-report-class-custom")
            ))
            .size(body),
        )
        .on_press(Message::RunReportOpenFieldPressed(
            RunReportFieldKind::CustomClassification,
        ))
        .into(),
        plain_button(
            state,
            "agent-run-report-class-clear",
            Message::RunReportClassifyPressed(None),
        ),
    ];
    lines.push(row(buttons).spacing(8).wrap().vertical_spacing(4).into());
    // A fixed classification is shown by its `[x]` marker above; the line below
    // says the other two states, so "none yet" and a custom label are stated.
    match current {
        None => lines.push(
            text(state.catalog.get("agent-run-report-classification-none"))
                .size(status)
                .into(),
        ),
        Some(RunClassification::Custom(label)) => lines.push(
            text(
                state.catalog.get_with_args(
                    "agent-run-report-classification-custom",
                    &CatalogArgs::new()
                        .untrusted("label", &tekstide_core::text_safety::quote_untrusted(label)),
                ),
            )
            .size(status)
            .into(),
        ),
        Some(_) => {}
    }

    lines.push(
        text(state.catalog.get("agent-run-report-notes-heading"))
            .size(body)
            .into(),
    );
    match run.notes() {
        None => lines.push(
            text(state.catalog.get("agent-run-report-notes-none"))
                .size(status)
                .into(),
        ),
        Some(notes) => {
            for note_line in notes.lines() {
                lines.push(
                    text(
                        tekstide_core::text_safety::quote_untrusted(note_line)
                            .as_str()
                            .to_owned(),
                    )
                    .size(body)
                    .into(),
                );
            }
        }
    }
    lines.push(
        row![
            plain_button(
                state,
                "agent-run-report-notes-edit-button",
                Message::RunReportOpenFieldPressed(RunReportFieldKind::Notes),
            ),
            plain_button(
                state,
                "agent-run-report-export-button",
                Message::RunReportOpenFieldPressed(RunReportFieldKind::ExportPath),
            ),
        ]
        .spacing(8)
        .into(),
    );

    if let Some(field) = &state.run_report_field {
        lines.push(field_view(state, field));
    }
    if let Some(notice) = &state.run_report_notice {
        lines.push(
            text(notice_text(&state.catalog, notice))
                .size(status)
                .into(),
        );
    }
    column(lines).spacing(8).into()
}

fn field_view<'a>(state: &'a State, field: &'a RunReportField) -> Element<'a, Message> {
    let body = state.theme.font_size_body();
    let status = state.theme.font_size_status();
    let mut lines: Vec<Element<'a, Message>> = Vec::new();
    let (prompt_key, max) = match field.kind {
        RunReportFieldKind::CustomClassification => (
            "agent-run-report-field-custom",
            RUN_CUSTOM_CLASSIFICATION_MAX_CHARS as u64,
        ),
        RunReportFieldKind::Notes => ("agent-run-report-field-notes", RUN_NOTES_MAX_CHARS as u64),
        RunReportFieldKind::ExportPath => ("agent-run-report-field-export", 0),
    };
    lines.push(
        text(
            state
                .catalog
                .get_with_args(prompt_key, &CatalogArgs::new().number("max", max)),
        )
        .size(body)
        .into(),
    );
    if let Some(contents) = field.export_contents {
        lines.push(
            text(
                state.catalog.get_with_args(
                    "agent-run-report-export-contains",
                    &CatalogArgs::new()
                        .number("count", contents.changed_paths as u64)
                        .number("bytes", contents.transcript_bytes_quoted as u64),
                ),
            )
            .size(status)
            .into(),
        );
        lines.push(
            text(state.catalog.get("agent-run-report-export-outside"))
                .size(status)
                .into(),
        );
    }
    // What is being typed, escaped at the widget like every untrusted string,
    // with a caret so an empty field is visibly a field.
    for buffer_line in field.buffer.split('\n') {
        lines.push(
            text(format!(
                "| {}",
                tekstide_core::text_safety::quote_untrusted(buffer_line).as_str()
            ))
            .size(body)
            .into(),
        );
    }
    lines.push(
        row![
            plain_button(
                state,
                "agent-run-report-field-save",
                Message::RunReportFieldSubmitPressed
            ),
            plain_button(
                state,
                "agent-run-report-field-cancel",
                Message::RunReportFieldCancelPressed
            ),
        ]
        .spacing(8)
        .into(),
    );
    lines.push(
        text(state.catalog.get("agent-run-report-field-hint"))
            .size(status)
            .into(),
    );
    column(lines).spacing(4).into()
}

pub(super) fn notice_text(catalog: &Catalog, notice: &RunReportNotice) -> String {
    match notice {
        RunReportNotice::Exported { path, bytes } => catalog.get_with_args(
            "agent-run-report-notice-exported",
            &CatalogArgs::new()
                .number("bytes", *bytes)
                .untrusted("path", &tekstide_core::text_safety::quote_untrusted(path)),
        ),
        RunReportNotice::ExportRefused(refusal) => catalog.get(match refusal {
            ReportExportRefusal::NotAbsolute => "agent-run-report-notice-not-absolute",
            ReportExportRefusal::FolderMissing => "agent-run-report-notice-folder-missing",
            ReportExportRefusal::AlreadyExists => "agent-run-report-notice-already-exists",
            ReportExportRefusal::InsideStateDirectory => "agent-run-report-notice-inside-state",
            ReportExportRefusal::WriteFailed => "agent-run-report-notice-write-failed",
        }),
        RunReportNotice::BlankLabel => catalog.get("agent-run-report-notice-blank-label"),
        RunReportNotice::NotRecorded => catalog.get("agent-run-report-notice-not-recorded"),
        RunReportNotice::RecordWriteFailed => catalog.get("agent-run-report-notice-record-failed"),
    }
}
