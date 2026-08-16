//! RFC-015 PR-015-B/PR-015-C: window, layer composition, chrome, the
//! theme/i18n seams (PR-015-B), and real input routing (PR-015-C).
//! **No surfaces yet** -- PR-015-D adds the Project Board. Input
//! *classification* lives in [`crate::input`]; this module is the one
//! place that turns a classified [`input::RoutedInput`] into an actual
//! state change, via [`update`].
//!
//! **Layer composition** follows RFC-015's layer model:
//!
//! | Layer | Contents | Trust |
//! | --- | --- | --- |
//! | Chrome | top bar, status bar | Trusted |
//! | Content | placeholder (no surface yet) | untrusted content will land here from PR-015-D |
//! | Modal | layer-composition demo | Trusted, exclusive |
//!
//! Composed via `stack`/`opaque`, the mechanism the RFC-014 spike proved
//! (C8). Real dialogs are RFC-022's job; this slice's modal occupant is
//! still the PR-015-B placeholder (`implementation-handoff.md` §8's
//! explicit allowance), but PR-015-C makes it genuinely dismissible via
//! real input, since a placeholder that never closes cannot exercise
//! the focus-trap and modal-exclusivity properties this slice must
//! prove. Opening it remains env-gated (`TEKSTIDE_LAYER_DEMO`, read once
//! at boot) -- there is still no real trigger to open a dialog
//! (RFC-022), only a real way to close one now that input exists.
//!
//! **Modal exclusivity is structural**, via [`input::ModalAbsent`]: see
//! [`subscription`] and `input`'s module doc. While `state.modal` is
//! `Some`, the *only* subscription active is [`modal_subscription`],
//! which has no path to producing `input::SurfaceInput` or
//! `input::TextStream` at all -- not "produced and ignored."
//!
//! **No shell-local state mirrors core state** (`implementation-handoff.md`
//! §2). [`State`] holds exactly one [`ApplicationShell`] -- the sole
//! source of model state -- plus purely presentational fields
//! (`catalog`, `theme`, `focus`, `modal`), none of which duplicate a
//! value already inside it.

use iced::futures::SinkExt;
use iced::widget::{center, column, container, opaque, row, stack, text};
use iced::{Background, Border, Element, Length, Subscription, Task, keyboard};

use tekstide_core::command::AppCommand;
use tekstide_core::domain::TerminalId;
use tekstide_core::navigation::{KeybindingPolicy, NavigationAction};
use tekstide_core::project::{ProjectMode, ProjectOpenSurface};
use tekstide_core::route::AppRoute;
use tekstide_core::runtime::terminal::{
    LinuxTerminalRuntime, TerminalInputDecision, TerminalInputDecisionReason, TerminalInputPolicy,
    TerminalInputSource, TerminalLaunchError, TerminalRuntimeHandle, TerminalTrustedUiState,
};
use tekstide_core::shell::ApplicationShell;

use crate::i18n::{Catalog, CatalogArgs};
use crate::input::{self, FocusZone, RoutedInput, TextStream};
use crate::measurement::{self, Measurement};
use crate::theme::Theme;

/// RFC-015 PR-015-F: the synthetic typing-measurement surface's preloaded
/// content -- a real ~1,500-line source file, not a lorem-ipsum
/// placeholder, so the layout cost the measurement exercises is the same
/// shape a real editor would see. The RFC-014 spike crate
/// (`tekstide-gui-spike`, `publish = false`, deleted 2026-08-04 -- see
/// `rfcs/handoffs/014-desktop-gui-substrate-and-terminal-rendering/spike-crate-deletion.md`)
/// set the precedent of `include_str!`-ing this file directly out of
/// `tekstide-core`; `tekstide` is a published crate, and a package
/// tarball can never contain a sibling crate's source, so this is
/// a static, committed snapshot living inside this crate instead --
/// discovered by `cargo package`'s verification step during 0.4.0 RC prep,
/// not by inspection. Only loaded into `State.typing_doc` when actually
/// measuring `Typing` (see `State::new`); otherwise never referenced.
const TYPING_MEASUREMENT_DOCUMENT: &str = include_str!("../typing-measurement-sample.rs");

/// The two focusable targets of the layer-composition demo modal --
/// still scaffolding (see the module doc), but now real enough for a
/// genuine focus-trap test: while `state.modal` is `Some`, Tab/Shift+Tab
/// must cycle only between these two, never `state.focus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalButton {
    Acknowledge,
    Dismiss,
}

impl ModalButton {
    const ORDER: [ModalButton; 2] = [ModalButton::Acknowledge, ModalButton::Dismiss];

    fn next(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|button| *button == self)
            .unwrap_or(0);
        Self::ORDER[(index + 1) % Self::ORDER.len()]
    }

    fn previous(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|button| *button == self)
            .unwrap_or(0);
        Self::ORDER[(index + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
}

/// RFC-018 PR-018-C: the accept/reject targets of the real paste
/// confirmation dialog -- a distinct type from `ModalButton` even
/// though the two-item cycle shape is identical, since "Acknowledge"/
/// "Dismiss" do not describe what this dialog's buttons decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PasteConfirmButton {
    Accept,
    Reject,
}

impl PasteConfirmButton {
    const ORDER: [PasteConfirmButton; 2] = [PasteConfirmButton::Accept, PasteConfirmButton::Reject];

    fn next(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|button| *button == self)
            .unwrap_or(0);
        Self::ORDER[(index + 1) % Self::ORDER.len()]
    }

    fn previous(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|button| *button == self)
            .unwrap_or(0);
        Self::ORDER[(index + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
}

/// RFC-018 PR-018-C: the pasted content a confirmation dialog is
/// deciding about. `content` is the exact `String` `TerminalPasteResolved`
/// received -- already within `paste_bytes_within_bound`'s cap, since a
/// paste this large was already evaluated against that bound before
/// `RequiresConfirmation` could be reached -- so `content.as_bytes()`
/// is exactly what Accept writes. No second, possibly-diverged copy of
/// "what will be pasted" exists.
#[derive(Debug)]
pub(crate) struct PasteConfirmationModal {
    target: TerminalId,
    content: String,
    line_count: usize,
    focus: PasteConfirmButton,
}

/// RFC-019 PR-019-D: the reload/dismiss targets of the real external-
/// change conflict dialog -- the same two-item cycle shape as
/// `PasteConfirmButton`, a distinct type for the same reason: "Reload"/
/// "Dismiss" do not describe what the paste dialog's buttons decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExternalChangeButton {
    Reload,
    Dismiss,
}

impl ExternalChangeButton {
    const ORDER: [ExternalChangeButton; 2] =
        [ExternalChangeButton::Reload, ExternalChangeButton::Dismiss];

    fn next(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|button| *button == self)
            .unwrap_or(0);
        Self::ORDER[(index + 1) % Self::ORDER.len()]
    }

    fn previous(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|button| *button == self)
            .unwrap_or(0);
        Self::ORDER[(index + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
}

/// RFC-019 PR-019-D: which document a save was refused for -- captured
/// from `active_document()`'s path at the moment `save_active_document`
/// returned `BlockedExternalChange`, not re-read later, since "Reload"
/// re-opens exactly this path.
///
/// RFC-019 PR-019-E: `had_local_edits` disambiguates the two situations a
/// `BlockedExternalChange` refusal covers -- a dirty buffer really would
/// lose local edits on Reload, but a *clean* document that merely
/// changed on disk has none to lose -- because the modal's own wording
/// needs to say which case this is, not because
/// `ProjectContentWorkspace::save_active_document` conflates them
/// (status-mapping-honesty-fixes Fix 2 removed that conflation;
/// `ProjectContentStatus` itself now distinguishes `Conflict` from
/// `ExternalChanged`). `TextDocument::save()` already computes the
/// distinction this field needs (`self.state` becomes `Conflict` only
/// `if self.is_dirty()`, `ExternalChanged` otherwise --
/// `content::document`'s own `block_external_change`), so this reads
/// that real, already-computed distinction rather than inventing a
/// second one.
#[derive(Debug)]
pub(crate) struct ExternalChangeModal {
    relative_path: std::path::PathBuf,
    had_local_edits: bool,
    focus: ExternalChangeButton,
}

/// RFC-022 PR-022-E: which button the approval dialog's focus is on.
/// `ApproveOnce`/`Reject` only -- `task-breakdown-pr-plan.md`'s own
/// PR-022-E review gate names Approve/Reject and the cooperative-limit
/// wording, not an edit-argv flow (`ApprovalCoordinator::decide_with_edited_argv`
/// exists in `tekstide-core` but has no caller from this slice's own
/// gate), so this type does not carry a third button for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApprovalDialogButton {
    ApproveOnce,
    Reject,
}

#[allow(dead_code)]
impl ApprovalDialogButton {
    const ORDER: [ApprovalDialogButton; 2] = [
        ApprovalDialogButton::ApproveOnce,
        ApprovalDialogButton::Reject,
    ];

    fn next(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|button| *button == self)
            .unwrap_or(0);
        Self::ORDER[(index + 1) % Self::ORDER.len()]
    }

    fn previous(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|button| *button == self)
            .unwrap_or(0);
        Self::ORDER[(index + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
}

/// RFC-022 PR-022-E: the security surface -- see
/// `what-the-dialog-must-not-lie-about.md` and response 221. Holds the
/// full `ApprovalRequest` rather than a handful of extracted fields the
/// way `ExternalChangeModal` does: this dialog genuinely needs nearly
/// every field (`display_command`/`cwd` to render, `risk_level`/
/// `risk_reasons` to disclose, `id`/`agent_run_id` to call `decide`
/// against once a decision is made), so extracting a subset would just
/// be a second, partial copy of the same struct.
///
/// **Not yet wired into `ModalContent`.** Review request 220 (open
/// question 3: does this dialog interrupt whatever the user is doing)
/// is still open -- the trigger path (when/how `state.modal` becomes
/// `Some(ModalContent::Approval(..))`) is exactly what that answer
/// decides, so it is deliberately not built yet. This struct and its
/// rendering are provable and correct independent of that answer, which
/// is why they are built now rather than waiting.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct ApprovalDialog {
    request: tekstide_core::domain::ApprovalRequest,
    focus: ApprovalDialogButton,
}

/// RFC-018 PR-018-C: `state.modal` must remain the *one* value
/// `input::ModalAbsent`/`SubscriptionMode::for_modal` gate on, so a
/// second real modal kind has to live inside this same type rather than
/// a second `Option` field on `State` -- a parallel field would not be
/// covered by that structural exclusivity at all, the same reasoning
/// that keeps `terminal_launch_notice`/`terminal_paste_notice` as
/// presentational state distinct from anything `ModalAbsent` gates.
#[derive(Debug)]
pub(crate) enum ModalContent {
    LayerDemo {
        focus: ModalButton,
    },
    PasteConfirmation(PasteConfirmationModal),
    /// RFC-019 PR-019-D: `TextDocument::save()` has no force-overwrite
    /// bypass -- `BlockedExternalChange` is unconditional once the disk
    /// snapshot has diverged from `last_known_snapshot`, regardless of
    /// local dirty state. This modal is the only way past it: Reload
    /// re-opens (discarding local edits), Dismiss/Escape leaves the file
    /// untouched.
    ExternalChange(ExternalChangeModal),
}

impl Default for ModalContent {
    fn default() -> Self {
        // Defaulting to the less destructive-sounding target, the same
        // reasoning the RFC-014 spike's `DialogButton::Deny` default
        // used -- this modal has no real consequence either way, but the
        // convention is cheap to keep consistent.
        Self::LayerDemo {
            focus: ModalButton::Dismiss,
        }
    }
}

/// Response 134 Required: measurement and the demo modal must be mutually
/// exclusive, or `subscription()`'s measurement branch (checked first,
/// ahead of `SubscriptionMode::for_modal` entirely -- see `subscription`'s
/// doc) would skip modal exclusivity while the modal is still on screen,
/// silently reopening the "produced-then-ignored" gap PR-015-C closed.
/// Measurement wins: it is a bounded, self-terminating diagnostic run
/// PR-015-C's structural property has no reason to apply to in the first
/// place, whereas the demo modal exists only to be screenshotted
/// interactively. A pure function, not inlined into `State::new`, so the
/// exclusivity itself is testable without racing on process-global
/// `TEKSTIDE_LAYER_DEMO`/`TEKSTIDE_MEASURE_CRITERION` env vars against
/// concurrently-running tests that also construct a `State`.
fn modal_for_state(measurement_active: bool, layer_demo_requested: bool) -> Option<ModalContent> {
    if measurement_active {
        None
    } else {
        layer_demo_requested.then(ModalContent::default)
    }
}

pub struct State {
    app_shell: ApplicationShell,
    catalog: Catalog,
    theme: Theme,
    focus: FocusZone,
    modal: Option<ModalContent>,
    /// RFC-015 PR-015-F: `None` unless `TEKSTIDE_MEASURE_CRITERION` names
    /// a recognized criterion -- see `measurement`'s module doc.
    measurement: Option<Measurement>,
    /// RFC-015 PR-015-F: the typing-measurement surface's live content.
    /// Empty unless actually measuring `Typing`.
    typing_doc: String,
    /// RFC-017 PR-017-E, renamed by the terminal-launch-UX handoff (was
    /// `terminal_demo` -- misleading once `Ctrl+Alt+T` populates it too,
    /// not only `TEKSTIDE_TERMINAL_DEMO`). Every live `TerminalPane` this
    /// GUI is currently tracking, whichever of [`launch_terminal`]'s
    /// callers put it here. Rendering state only; *which* slot each
    /// pane's session occupies is asked of `tekstide-core` fresh each
    /// time (`active_project_terminal_sessions`), not cached alongside
    /// these panes.
    terminal_panes: Vec<crate::surface::terminal::TerminalPane>,
    /// Terminal launch UX handoff: the most recent launch refusal, if
    /// any -- shell-local, transient UI feedback (like `modal`), not
    /// core state; the underlying fact (how many sessions exist, what
    /// the limit is) is read fresh from `tekstide-core` each time a
    /// launch is attempted, never cached here. Cleared at the start of
    /// every new launch attempt, so it never outlives the situation that
    /// produced it.
    terminal_launch_notice: Option<TerminalLaunchRefusal>,
    /// RFC-022 PR-022-D: the same shell-local, transient shape as
    /// `terminal_launch_notice`, for `LaunchAgentRun`'s own refusals --
    /// most commonly `ExecutableUnavailable` ("no AI CLI found"), the
    /// honest, common first-run state, not a bug to route around (see
    /// response 218). Cleared at the start of every new launch attempt.
    agent_run_launch_notice: Option<AgentRunLaunchRefusal>,
    /// RFC-018 PR-018-B: the most recent paste refusal, if any -- same
    /// shell-local, transient shape as `terminal_launch_notice`. Never
    /// holds a successful paste (`TerminalPasteRefusal` cannot represent
    /// `Allow` -- see its own doc comment), and is cleared at the start
    /// of every new paste attempt.
    terminal_paste_notice: Option<TerminalPasteRefusal>,
    /// RFC-019 PR-019-B: which row of the *currently rendered* explorer
    /// listing the keyboard cursor is on. Shell-local UI state, not a
    /// duplicate of core's -- core has no concept of "which row a
    /// keyboard cursor is over," only `selected_explorer_path` (which
    /// directory is scanned). The direct analogue of `PasteConfirmButton`/
    /// `ModalButton`'s focus index: a rendering/interaction concern, not
    /// part of the document model `TextCursor`/`TextViewport` cover.
    /// Reset to `0` every time a scan succeeds, so it can never point
    /// past the end of a freshly-replaced row list.
    explorer_highlight: usize,
}

impl State {
    pub fn new(mut app_shell: ApplicationShell, catalog: Catalog) -> Self {
        let measurement = Measurement::from_env();
        let typing_doc = if matches!(
            measurement.as_ref().map(Measurement::criterion),
            Some(measurement::Criterion::Typing)
        ) {
            TYPING_MEASUREMENT_DOCUMENT.to_string()
        } else {
            String::new()
        };

        let modal = modal_for_state(
            measurement.is_some(),
            std::env::var("TEKSTIDE_LAYER_DEMO").is_ok(),
        );
        // RFC-017 PR-017-F: the store itself (not just the event) is
        // opened only inside `launch_terminal_demo_panes`, behind the
        // same `TEKSTIDE_TERMINAL_DEMO` gate -- response 152 Required 1:
        // opening unconditionally here created
        // `$XDG_STATE_HOME/tekstide/audit/audit.sqlite3` (empty, but
        // with the full schema) on every ordinary launch, which made
        // the README's "ordinary use still does not create this file"
        // false. Not stored on `State` either way -- see that
        // function's doc comment for why a persistent field isn't
        // justified yet.
        // RFC-017 PR-017-G: `TerminalFlood` gets its own, separate
        // launch path -- not `launch_terminal_demo_panes` -- because
        // that function also opens the real, durable audit store
        // (PR-017-F), and this measurement path must not exercise that
        // unrelated I/O while timing, the same non-contamination
        // principle every other criterion already follows.
        //
        // `TEKSTIDE_TERMINAL_FLOOD_DEMO` (response 155 item 5) launches
        // the exact same pane-plus-flood scenario **without** enabling
        // measurement -- the control the non-contamination proof needs:
        // identical workload, instrumentation the only difference,
        // rather than comparing against a genuinely idle baseline that
        // cannot separate the two. Same disclosed, checked-but-usually-
        // absent shape every other demo/measurement env var here uses.
        let mut audit_health = tekstide_core::audit::AuditHealth::default();
        let terminal_panes = if measurement.as_ref().map(Measurement::criterion)
            == Some(measurement::Criterion::TerminalFlood)
            || std::env::var("TEKSTIDE_TERMINAL_FLOOD_DEMO").is_ok()
        {
            launch_measurement_terminal_pane(&mut app_shell)
        } else {
            launch_terminal_demo_panes(&mut app_shell, &mut audit_health)
        };

        Self {
            app_shell,
            catalog,
            theme: Theme::default(),
            focus: FocusZone::MainArea,
            modal,
            measurement,
            typing_doc,
            terminal_panes,
            terminal_launch_notice: None,
            agent_run_launch_notice: None,
            terminal_paste_notice: None,
            explorer_highlight: 0,
        }
    }

    pub fn window_title(&self) -> String {
        self.catalog.get("app-title")
    }

    /// Whether `content_area` should substitute the synthetic
    /// typing-measurement view for the real content (RFC-015 PR-015-F).
    /// `false` for `ModeSwitch`: C4 measures the *real* content's view
    /// cost, so nothing is substituted for it.
    pub fn is_measuring_typing(&self) -> bool {
        matches!(
            self.measurement.as_ref().map(Measurement::criterion),
            Some(measurement::Criterion::Typing)
        )
    }

    /// Whether `main.rs`'s view wrapper should time this render as a
    /// view-build-cost sample (RFC-015 PR-015-F/PR-015-E). `Startup`
    /// times its one frame via `frames()` instead and needs no view-cost
    /// log; with no measurement active (the default), this is always
    /// `false`. Broader than [`Self::is_measuring_typing`]: `ModeSwitch`
    /// also needs view-cost timing, but over the real content, not a
    /// substituted one.
    pub fn is_measuring_view_cost(&self) -> bool {
        matches!(
            self.measurement.as_ref().map(Measurement::criterion),
            Some(measurement::Criterion::Typing) | Some(measurement::Criterion::ModeSwitch)
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Input(RoutedInput),
    ModalFocusNext,
    ModalFocusPrevious,
    ModalActivate,
    ModalDismiss,
    /// RFC-015 PR-015-F: a synthetic measurement keystroke arrived; the
    /// `Instant` is when the measurement subscription first saw it, not
    /// when `update` gets around to handling it -- the gap between the
    /// two is exactly the input-to-state-change sample.
    MeasuredKey(std::time::Instant),
    /// RFC-015 PR-015-F: periodic check for whether the measurement run
    /// has reached its sample target and should self-exit.
    MeasurementTick,
    /// RFC-015 PR-015-F: a frame was painted during `Startup` measurement.
    MeasurementFrame(std::time::Instant),
    /// RFC-015 PR-015-E: a synthetic C4 measurement keystroke arrived;
    /// same timing convention as `MeasuredKey`, but its handler dispatches
    /// the real `AppCommand::ToggleActiveProjectMode` instead of
    /// appending to a synthetic document.
    MeasuredModeSwitch(std::time::Instant),
    /// RFC-017 PR-017-G: a synthetic C3 measurement keystroke arrived;
    /// same timing convention as `MeasuredKey`, but its handler writes a
    /// real byte directly into the one real, live `TerminalPane` this
    /// criterion launches, under a concurrent bounded background output
    /// flood -- see `measurement`'s module doc for why this one skips
    /// the view-build half of the decomposition.
    MeasuredTerminalInput(std::time::Instant),
    /// RFC-017 Amendment 1, PR-A1-C: replaces `TerminalPollTick` (a
    /// fixed 50ms timer polling every pane) with an event carrying only
    /// which one pane has something to report -- see
    /// [`terminal_wake_subscription`]. **Carries a `TerminalId` and
    /// nothing else, deliberately**: `Message` derives `Debug` and
    /// `Clone`, so a payload of raw PTY bytes here would be formattable
    /// and duplicable outside the one reviewed ingress -- the exact
    /// shape P2 exists to deny, the reasoning response 205 settled this
    /// slice's design on. Fires on real data arriving, and once more on
    /// EOF/termination even if the pane produced nothing (a silently
    /// exited shell must still be noticed) -- see `WakeNotifier`'s own
    /// module doc in `tekstide-core`. Handled per-pane
    /// (`handle_terminal_woke`), not by iterating every tracked pane the
    /// way the old tick's handler did; a pane not named by this message
    /// is not touched.
    TerminalWoke(tekstide_core::domain::TerminalId),
    /// RFC-018 PR-018-B: a real clipboard read, triggered by
    /// `Ctrl+Shift+V`, has resolved. `target` is the terminal that was
    /// keyboard-focused when the key was pressed, captured then rather
    /// than looked up now -- `content` is `None` if the clipboard was
    /// empty or unreadable. Deliberately carries no policy decision:
    /// `TerminalInputPolicy::evaluate` is called fresh in this message's
    /// own handler, against state as it stands *now*, so a modal opening
    /// or focus moving during the async round trip is judged against
    /// reality rather than a stale snapshot from before the read.
    TerminalPasteResolved {
        target: TerminalId,
        content: Option<String>,
    },
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Input(RoutedInput::Shell(shell_input)) => {
            let action = shell_input.action();
            if let Some(command) = app_command_for(action) {
                // Terminal launch UX handoff: `LaunchTerminal` is the one
                // command needing real I/O (spawning a process) alongside
                // the route/mode change `dispatch` itself performs --
                // `tekstide-core` has no I/O to do that with, so this is
                // the one place `update` acts before, not only through,
                // `dispatch`. Attempted regardless of what `dispatch`
                // will do next; the mode switch always happens, so a
                // refused launch still lands the user where they can see
                // the notice (`terminal_workspace_view`) explaining why.
                if command == AppCommand::LaunchTerminal {
                    state.terminal_launch_notice = None;
                    if let Err(refusal) = attempt_terminal_launch(state) {
                        state.terminal_launch_notice = Some(refusal);
                    }
                }
                // RFC-022 PR-022-D: the same shape as `LaunchTerminal`
                // just above -- real I/O before `dispatch`, attempted
                // regardless of what `dispatch` will do next, so a
                // refused launch still lands the user in Terminal
                // Immersion where the notice is visible.
                if command == AppCommand::LaunchAgentRun {
                    state.agent_run_launch_notice = None;
                    if let Err(refusal) = attempt_agent_run_launch(state) {
                        state.agent_run_launch_notice = Some(refusal);
                    }
                }
                state.app_shell.dispatch(command);
                // RFC-019 PR-019-B: any command that could have changed
                // which project or mode is active is a point where the
                // explorer tree might now need a scan it has never had --
                // `scan_active_project_explorer_directory` had no
                // production caller before this slice (confirmed by
                // enumeration), so nothing else triggers the first one.
                ensure_explorer_scanned(state);
            }
            // RFC-018 PR-018-B: `PasteIntoTerminal` maps to no
            // `AppCommand` (there is no core route/mode change), so it
            // is handled here, the same way `LaunchTerminal`'s real I/O
            // half is -- but unlike that command, this one needs a real
            // `Task` (the clipboard read), so it returns early rather
            // than falling through to the function's own `Task::none()`.
            if action == NavigationAction::PasteIntoTerminal {
                return attempt_paste_into_terminal(state);
            }
            // RFC-019 PR-019-D: `SaveActiveDocument` needs the same real
            // I/O `LaunchTerminal` needs, but no `Task` round trip --
            // `save_active_document` is bounded, synchronous local disk
            // I/O, the same shape `ensure_explorer_scanned`'s scan
            // already is.
            if action == NavigationAction::SaveActiveDocument {
                attempt_save_active_document(state);
            }
        }
        Message::Input(RoutedInput::Surface(surface_input)) => {
            // RFC-019 PR-019-B: the explorer tree is the first real
            // consumer of this routed input (PR-015-D's own note above
            // named this exact arm as where "select a row and open it"
            // would eventually land). Every other zone still has nothing
            // to consume it.
            if surface_input.target() == FocusZone::Sidebar {
                handle_explorer_key(state, surface_input.key());
            }
            // RFC-019 PR-019-D: the editor is the second real consumer --
            // a key routed to `MainArea` while in Content mode with an
            // active document edits it. `apply_edit_key` decides *what*
            // the next text is (append-only, see its own doc comment for
            // why); this arm only decides *whether* a key reaches it at
            // all.
            if surface_input.target() == FocusZone::MainArea {
                handle_editor_key(state, surface_input.key());
            }
        }
        Message::Input(RoutedInput::Terminal(text_stream)) => {
            // `terminal_stream_targets_a_live_terminal` gets its first
            // real caller this slice: the demo panes are now registered
            // `TerminalSession`s on the real active project (RFC-017
            // PR-017-E), so the check RFC-015 wrote against the real
            // project model finally has something real to check.
            //
            // The modal-exclusivity gate itself now lives in
            // `write_terminal_input` (RFC-018 PR-018-B), the one place
            // both this arm and the paste path actually write -- see
            // that function's doc comment for why it is checked there
            // and not here.
            if terminal_stream_targets_a_live_terminal(&state.app_shell, &text_stream)
                && let Some(bytes) = text_stream.to_pty_bytes()
            {
                write_terminal_input(state, text_stream.target(), &bytes);
            }
        }
        Message::TerminalPasteResolved { target, content } => {
            // RFC-018 PR-018-B: the policy decision cannot be made until
            // now -- `evaluate` needs real, current state (the active
            // project's id, whichever terminal is *now* focused, whether
            // a modal has opened since the key was pressed), none of
            // which existed synchronously at the moment `Ctrl+Shift+V`
            // fired. No active project is a silent no-op, the same
            // precedent `attempt_terminal_launch` already sets: state
            // changed enough during the round trip that there is nothing
            // left to address the paste to.
            let Some(project) = state.app_shell.state().active_project() else {
                return Task::none();
            };
            // Response 169 Required: refuses rather than truncates, and
            // does so *before* `evaluate` is ever called. Truncating
            // first let truncation change the classification itself (a
            // paste whose only newline sits past the cap truncated to
            // `SingleLine` and `Allow`ed) and, worse, would silently
            // write a prefix of what the user actually copied. `evaluate`
            // always sees the paste's real, complete content now, or the
            // paste is refused before it is ever called.
            let Some(content) = content_within_bound(content) else {
                state.terminal_paste_notice = Some(TerminalPasteRefusal::TooLarge);
                return Task::none();
            };
            let project_id = project.id().clone();
            let target_handle = TerminalRuntimeHandle::new(target.clone(), project_id.clone());
            let active_handle = active_terminal_focus(state)
                .map(|id| TerminalRuntimeHandle::new(id, project_id.clone()));

            // No classification here -- every `Allow`/`Block`/
            // `RequiresConfirmation` originates from `evaluate`, exactly
            // as it does for the RFC-009 boundary this renders.
            let decision = TerminalInputPolicy.evaluate(
                &target_handle,
                active_handle.as_ref(),
                TerminalInputSource::Paste,
                content.as_bytes(),
                trusted_ui_state(state),
            );

            match decision {
                TerminalInputDecision::Allow { .. } => {
                    write_terminal_input(state, &target, content.as_bytes());
                }
                // RFC-018 PR-018-C: renders the real confirmation dialog
                // rather than blocking -- `evaluate`'s own classification
                // (`classify_paste`) only ever reaches `RequiresConfirmation`
                // via `TerminalPasteClass::Multiline`, so `line_count` is
                // always a real, positive count here. `content` moves into
                // the modal unchanged: it is exactly what `ModalActivate`'s
                // `Accept` path writes, not a second copy that could
                // diverge from what the dialog actually showed.
                TerminalInputDecision::RequiresConfirmation { .. } => {
                    let line_count = content.lines().count().max(1);
                    state.modal = Some(ModalContent::PasteConfirmation(PasteConfirmationModal {
                        target,
                        content,
                        line_count,
                        focus: PasteConfirmButton::Reject,
                    }));
                }
                TerminalInputDecision::Block { reason, .. } => {
                    // RFC-018 PR-018-D: only a real, `evaluate`-produced
                    // `Block` is `paste_blocked` -- `valid_paste_blocked`
                    // requires `outcome == Blocked`, and neither
                    // `RequiresConfirmation` (a real, deferred decision,
                    // now rendered into the dialog above rather than
                    // forced into blocking) nor `TooLarge` (a shell-level
                    // resource bound that never reached `evaluate` at
                    // all, so it has no `TerminalInputDecisionReason` to
                    // report) is one. Auditing either would misrepresent
                    // *why* nothing was written.
                    let mut audit_store = open_real_audit_store(&state.app_shell);
                    let mut audit_health = tekstide_core::audit::AuditHealth::default();
                    if let Some(store) = audit_store.as_mut() {
                        let _ =
                            tekstide_core::audit::AuditCoordinator::new(store, &mut audit_health)
                                .record_paste_blocked(project_id, target.clone());
                    }
                    state.terminal_paste_notice = Some(TerminalPasteRefusal::Blocked(reason));
                }
            }
        }
        Message::Input(RoutedInput::FocusNext) => state.focus = state.focus.next(),
        Message::Input(RoutedInput::FocusPrevious) => state.focus = state.focus.previous(),
        Message::ModalFocusNext => match state.modal.as_mut() {
            Some(ModalContent::LayerDemo { focus }) => *focus = focus.next(),
            Some(ModalContent::PasteConfirmation(modal)) => modal.focus = modal.focus.next(),
            Some(ModalContent::ExternalChange(modal)) => modal.focus = modal.focus.next(),
            None => {}
        },
        Message::ModalFocusPrevious => match state.modal.as_mut() {
            Some(ModalContent::LayerDemo { focus }) => *focus = focus.previous(),
            Some(ModalContent::PasteConfirmation(modal)) => modal.focus = modal.focus.previous(),
            Some(ModalContent::ExternalChange(modal)) => modal.focus = modal.focus.previous(),
            None => {}
        },
        // RFC-018 PR-018-C: the layer-demo placeholder still has no
        // decision to record (RFC-022's real dialogs own that). The
        // paste dialog does: `ModalActivate` writes real bytes through
        // `write_terminal_input` -- PR-018-B's one ingress, not a new
        // one -- only when focus is on `Accept`; any other focus, or
        // `ModalDismiss` (Escape) regardless of focus, closes the modal
        // without writing anything. Escape defaulting to "not pasting"
        // holds structurally: `ModalDismiss`'s arm never touches the
        // write path at all, for either modal kind.
        Message::ModalActivate => match state.modal.take() {
            Some(ModalContent::PasteConfirmation(modal))
                if modal.focus == PasteConfirmButton::Accept =>
            {
                write_terminal_input(state, &modal.target, modal.content.as_bytes());
            }
            // RFC-019 PR-019-D: Reload re-opens the document fresh --
            // `open_active_project_text_document` takes disk's current
            // content and drops local edits, the only way past a
            // conflict `TextDocument::save()` itself provides. Any other
            // focus (Dismiss), or `ModalDismiss` (Escape) below, closes
            // the modal without touching the file at all -- the same
            // "every dismissal path defaults to not overwriting" shape
            // the paste dialog's own Reject/Escape arms already hold.
            Some(ModalContent::ExternalChange(modal))
                if modal.focus == ExternalChangeButton::Reload =>
            {
                let _ = state
                    .app_shell
                    .open_active_project_text_document(&modal.relative_path);
            }
            Some(ModalContent::LayerDemo { .. })
            | Some(ModalContent::PasteConfirmation(_))
            | Some(ModalContent::ExternalChange(_))
            | None => {}
        },
        Message::ModalDismiss => {
            state.modal = None;
        }
        Message::MeasuredKey(sent_at) => {
            if let Some(measurement) = state.measurement.as_mut() {
                measurement.record_input(sent_at);
            }
            state.typing_doc.push('x');
        }
        Message::MeasurementTick => {
            if state.measurement.as_ref().is_some_and(Measurement::is_done) {
                // Response 155 item 3 / response 156: printed once,
                // right before exit, rather than left to accumulate
                // unread -- the same "ad hoc, computed externally"
                // evidence convention every other measurement figure
                // here already uses (no percentile computation lives in
                // this binary either). `bytes_read_total` paired with
                // `elapsed_secs` is the precondition check: divide the
                // two externally to get observed in-app throughput, and
                // compare against the flood script's own standalone
                // throughput -- if far lower, the flood never reached
                // rate inside the application and the run is void.
                if let (Some(pane), Some(measurement)) =
                    (state.terminal_panes.first(), state.measurement.as_ref())
                {
                    // RFC-017 Amendment 1, PR-A1-C: `dropped_bytes_total`
                    // no longer appears here -- the field it read was
                    // dead state since PR-A1-B (nothing incremented it
                    // once `poll()` stopped calling
                    // `read_available_bounded_for`), and this slice
                    // removes that function entirely, so there is no
                    // code left anywhere that could ever produce a
                    // non-zero count. Removed rather than kept printing
                    // a permanent, structurally-guaranteed `0`.
                    eprintln!(
                        "terminal_flood bytes_read_total {} elapsed_secs {:.3}",
                        pane.bytes_read_total(),
                        measurement.elapsed().as_secs_f64(),
                    );
                }
                std::process::exit(0);
            }
        }
        Message::MeasurementFrame(at) => {
            if let Some(measurement) = state.measurement.as_mut() {
                measurement.record_startup_frame(at);
                if measurement.is_done() {
                    std::process::exit(0);
                }
            }
        }
        Message::MeasuredModeSwitch(sent_at) => {
            if let Some(measurement) = state.measurement.as_mut() {
                measurement.record_input(sent_at);
            }
            // The real command a `Ctrl+Alt+M` press would dispatch --
            // measuring the actual state mutation and the view rebuild
            // it causes, not a synthetic stand-in. Requires a real
            // active project (see `pr-015-e-mode-switching.md`'s C4
            // section); a measurement run without one records real,
            // if uninformative, ~0-cost samples rather than panicking.
            state
                .app_shell
                .dispatch(tekstide_core::command::AppCommand::ToggleActiveProjectMode);
        }
        Message::MeasuredTerminalInput(sent_at) => {
            // Response 154 Finding 1: `record_input` must come *after*
            // `write_input`, not before -- the interval it computes is
            // supposed to span this message's own work. Recording first
            // measured only iced's event-to-update dispatch latency (the
            // same quantity `Typing` already measures), with the write
            // itself silently excluded. The real `write_input` call is
            // the one a routed keystroke would eventually make --
            // bypassing only the `TextStream`/routing-target lookup
            // step, the same kind of bypass `MeasuredModeSwitch` already
            // established. `launch_measurement_terminal_pane` guarantees
            // exactly one pane exists whenever this criterion is active;
            // a measurement run somehow missing one records real, if
            // uninformative, samples against nothing rather than
            // panicking, matching `MeasuredModeSwitch`'s own "no active
            // project" fallback.
            // RFC-017 Amendment 1, PR-A1-D, response 209's required fix:
            // write a fresh, never-reused marker to the pty instead of
            // the bare `MEASURED_KEY_CHARACTER` -- `check_echo_visible`
            // (called from `handle_terminal_woke` on this pane's next
            // real wake) looks for *this* marker's own first appearance,
            // immune to the redraw-duplication finding a bare repeated
            // character's occurrence count was vulnerable to. See
            // `Measurement::next_echo_marker`'s own doc.
            let marker = state
                .measurement
                .as_mut()
                .map(measurement::Measurement::next_echo_marker);
            if let Some(pane) = state.terminal_panes.first_mut() {
                let bytes = marker
                    .as_deref()
                    .unwrap_or(measurement::MEASURED_KEY_CHARACTER);
                pane.write_input(bytes.as_bytes());
            }
            if let Some(measurement) = state.measurement.as_mut() {
                if let Some(marker) = marker {
                    measurement.note_measured_send(sent_at, marker);
                }
                measurement.record_input(sent_at);
            }
        }
        Message::TerminalWoke(terminal_id) => {
            handle_terminal_woke(state, &terminal_id);
        }
    }
    Task::none()
}

/// RFC-017 Amendment 1, PR-A1-C: the per-pane replacement for the old
/// tick handler's loop body. Same two-pass shape the tick handler
/// already used and for the same reason (pass 1 touches only
/// `state.terminal_panes` -- `check_exit`/`poll`, no `tekstide-core`
/// access; pass 2, factored into [`record_terminal_exit`], only touches
/// `state.app_shell` -- the status transition, slot release, audit
/// write -- avoiding a simultaneous `&state.app_shell` and
/// `&mut state.terminal_panes` borrow), just scoped to the one pane this
/// wake named instead of iterating every tracked pane on a fixed
/// schedule. Exit detection stays folded into the same handler that
/// polls, per the terminal-launch-UX handoff's own reasoning -- "wire it
/// into the existing poll path... which is where a non-blocking exit
/// check belongs" -- unchanged by moving from a tick to a wake.
fn handle_terminal_woke(state: &mut State, terminal_id: &tekstide_core::domain::TerminalId) {
    let already_exited = active_project_terminal_sessions(state)
        .iter()
        .any(|session| {
            session.id == *terminal_id
                && matches!(
                    session.status(),
                    tekstide_core::domain::TerminalStatus::Exited
                        | tekstide_core::domain::TerminalStatus::Failed
                )
        });
    if already_exited {
        // Already reflected in core -- "stop polling that pane's PTY."
        return;
    }

    let Some(pane) = state
        .terminal_panes
        .iter_mut()
        .find(|pane| pane.terminal_id() == terminal_id)
    else {
        // The pane this wake named closed between the wake firing and
        // this message reaching `update` -- nothing left to poll.
        return;
    };

    if let Some(outcome) = pane.check_exit() {
        record_terminal_exit(state, terminal_id.clone(), outcome);
        return;
    }

    // Response 156's discriminator: this handler's own wall time (the
    // PTY read plus VTE parse) -- unlike `record_input`'s figure, this
    // does not itself need a quiet machine to be informative.
    let started = std::time::Instant::now();
    pane.poll();
    if let Some(measurement) = state.measurement.as_mut() {
        measurement.record_tick_handler(started.elapsed(), pane.bytes_read_total());
        // RFC-017 Amendment 1, PR-A1-D: this wake may be the one
        // carrying a `MeasuredTerminalInput` send's own echo --
        // `Message::TerminalWoke` cannot say so itself (deliberately, by
        // response 205's design), so this checks the grid directly.
        // Gated on `should_check_echo`, not called unconditionally --
        // see that method's own doc for why "only while pending" alone
        // was still not enough under a real flood.
        if measurement.should_check_echo() {
            measurement.check_echo_visible(&pane.rendered_text());
        }
    }
}

/// The exit-recording half of [`handle_terminal_woke`], factored out so
/// it can be called for the one terminal a single wake found exited,
/// without needing the old tick handler's `Vec` of possibly several.
fn record_terminal_exit(
    state: &mut State,
    terminal_id: tekstide_core::domain::TerminalId,
    outcome: tekstide_core::runtime::terminal::TerminationOutcome,
) {
    let Some(project_id) = state.app_shell.state().active_project_id().cloned() else {
        return;
    };
    let mut audit_store = open_real_audit_store(&state.app_shell);
    let mut audit_health = tekstide_core::audit::AuditHealth::default();
    // `mark_terminal_exited` always transitions to `Exited` and records
    // the real exit code; any other outcome (signalled, or an ambiguous
    // `Failed`/`OrphanedUnknown` shape `try_wait` cannot itself produce
    // here) transitions to `Failed` via the more general status API
    // instead -- "the session bar stops lying" either way, best-effort
    // past this point for the same reason `launch_terminal`'s own
    // post-registration steps are.
    let _ = match &outcome {
        tekstide_core::runtime::terminal::TerminationOutcome::Exited { exit_status } => state
            .app_shell
            .state_mut()
            .mark_terminal_exited(&terminal_id, Some(*exit_status)),
        _ => state.app_shell.state_mut().transition_terminal_status(
            &terminal_id,
            tekstide_core::domain::TerminalStatus::Failed,
        ),
    };
    let _ = state
        .app_shell
        .state_mut()
        .assign_terminal_visible_slot(&terminal_id, tekstide_core::domain::VisibleSlot::Hidden);
    if let Some(store) = audit_store.as_mut() {
        let _ = tekstide_core::audit::AuditCoordinator::new(store, &mut audit_health)
            .record_plain_terminal_terminated(project_id, terminal_id, &outcome);
    }
}

/// Terminal launch UX handoff: why a real launch was refused -- a typed
/// answer the shell can render, never a panic and never a silent no-op.
/// `SessionLimitExceeded` is the one the review gate names explicitly;
/// `Launch`/`Registration` cover the rarer, more mechanical failure
/// paths (the shell couldn't be spawned at all; registration failed for
/// a reason other than the limit -- structurally near-impossible today,
/// since [`launch_terminal`] only calls `attach_terminal_session` after
/// confirming there is an active project to own the session).
#[derive(Debug, Clone, PartialEq, Eq)]
enum TerminalLaunchRefusal {
    SessionLimitExceeded { limit: u32 },
    Launch(tekstide_core::runtime::terminal::TerminalLaunchError),
    Registration(tekstide_core::project::ProjectTerminalError),
}

/// A compile-time literal symbol, not the displayed word -- the words
/// live in `en.ftl`'s `terminal-launch-refused` select expression, the
/// same division of labour `session_bar::slot_symbol`/`route_symbol`
/// already use.
fn terminal_launch_refusal_symbol(refusal: &TerminalLaunchRefusal) -> &'static str {
    match refusal {
        TerminalLaunchRefusal::SessionLimitExceeded { .. } => "limit",
        TerminalLaunchRefusal::Launch(_) | TerminalLaunchRefusal::Registration(_) => "error",
    }
}

/// The one catalog lookup a refusal's full text takes -- factored out,
/// same reason `session_bar::entry_text` is, so a test can assert over
/// what actually renders rather than over the symbol name alone.
fn terminal_launch_refusal_text(catalog: &Catalog, refusal: &TerminalLaunchRefusal) -> String {
    let mut args =
        CatalogArgs::new().trusted_symbol("reason", terminal_launch_refusal_symbol(refusal));
    if let TerminalLaunchRefusal::SessionLimitExceeded { limit } = refusal {
        args = args.number("limit", *limit);
    }
    catalog.get_with_args("terminal-launch-refused", &args)
}

/// The **one ingress** for creating and registering a real, PTY-backed
/// terminal session (RFC-017 PR-017-B/C's "no parallel construction
/// path" requirement, extended by the terminal-launch-UX handoff from
/// the filter/pane to session *creation* itself): [`launch_terminal_demo_panes`]'s
/// env-gated bootstrap and a real `Ctrl+Alt+T` press both call this same
/// function, never a second copy.
///
/// **`terminal_session_limit` is enforced in `tekstide-core`**
/// (`add_terminal_session`), not here -- this function's own pre-check
/// (below) exists only to avoid spawning a real process we already know
/// will be refused, not as the actual enforcement; `attach_terminal_session`'s
/// own check is what a caller cannot bypass by skipping this one.
///
/// On success: registers the session, transitions it `Starting` ->
/// `Running` (immediately -- a `TerminalPane::launch` success means the
/// shell has been spawned; leaving sessions at `Starting` forever, as
/// every launch path did before this handoff, is the same "session bar
/// stops lying" truthfulness gap `Exited` detection closes, just the
/// other end of it), assigns `target_slot`, and records a `Started`
/// `plain_terminal_observation` (best-effort, matching every other
/// producer call in this crate: an audit write failing must never fail
/// the terminal launch it observes).
fn launch_terminal(
    app_shell: &mut ApplicationShell,
    project_id: tekstide_core::project::ProjectId,
    title: impl Into<String>,
    root: std::path::PathBuf,
    target_slot: tekstide_core::domain::VisibleSlot,
    audit_store: Option<&mut tekstide_core::audit::AuditStore>,
    audit_health: &mut tekstide_core::audit::AuditHealth,
) -> Result<crate::surface::terminal::TerminalPane, TerminalLaunchRefusal> {
    if let Some(project) = app_shell.state().active_project()
        && let Some(limit) = project.resource_limits().terminal_session_limit
        && project.terminal_sessions().len() as u32 >= limit
    {
        return Err(TerminalLaunchRefusal::SessionLimitExceeded { limit });
    }

    let (pane, session) = crate::surface::terminal::TerminalPane::launch(
        project_id.clone(),
        title,
        root,
        std::path::PathBuf::from("/bin/sh"),
    )
    .map_err(TerminalLaunchRefusal::Launch)?;
    let terminal_id = session.id.clone();
    app_shell
        .state_mut()
        .attach_terminal_session(session)
        .map_err(TerminalLaunchRefusal::Registration)?;
    // Best-effort past this point: the session is already attached and
    // real, so a `Running`/slot-assignment failure would be a
    // `tekstide-core` invariant this function cannot itself repair --
    // the pane is still returned, rendering something real rather than
    // discarding a live process over a bookkeeping mismatch.
    let _ = app_shell
        .state_mut()
        .transition_terminal_status(&terminal_id, tekstide_core::domain::TerminalStatus::Running);
    let _ = app_shell
        .state_mut()
        .assign_terminal_visible_slot(&terminal_id, target_slot);

    if let Some(store) = audit_store {
        let _ = tekstide_core::audit::AuditCoordinator::new(store, audit_health)
            .record_plain_terminal_started(project_id, terminal_id);
    }

    Ok(pane)
}

/// RFC-022 PR-022-D: `TerminalLaunchRefusal`'s sibling for `LaunchAgentRun`
/// -- a typed answer the shell can render, never a panic and never a
/// silent no-op. `RunLimitExceeded` mirrors `TerminalLaunchRefusal::SessionLimitExceeded`
/// exactly, for the same reason (see [`attempt_agent_run_launch`]'s own
/// doc comment on why its pre-check is not the real enforcement). The
/// other variants wrap the real `tekstide-core` error at the coarsest
/// boundary that still lets the renderer distinguish "no AI CLI found"
/// (`Validation`'s `ExecutableUnavailable` case -- the honest, common
/// first-run state per response 218) and "this project is untrusted"
/// (`Validation`'s `WorkspaceDiscoveryBlocked` case) from every other,
/// rarer failure.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AgentRunLaunchRefusal {
    RunLimitExceeded { limit: u32 },
    Validation(tekstide_core::agent::AgentRunLaunchValidationError),
    PlanTransition(tekstide_core::domain::AgentRunTransitionError),
    Runtime(tekstide_core::project::ProjectAgentRuntimeLaunchError),
    Registration(TerminalLaunchError),
}

/// A compile-time literal symbol, not the displayed word -- the words
/// live in `en.ftl`'s `agent-run-launch-refused` select expression, the
/// same division of labour `terminal_launch_refusal_symbol` already
/// uses.
fn agent_run_launch_refusal_symbol(refusal: &AgentRunLaunchRefusal) -> &'static str {
    use tekstide_core::agent::AgentRunLaunchValidationError;
    match refusal {
        AgentRunLaunchRefusal::RunLimitExceeded { .. } => "limit",
        AgentRunLaunchRefusal::Validation(
            AgentRunLaunchValidationError::ExecutableUnavailable { .. },
        ) => "not-found",
        AgentRunLaunchRefusal::Validation(
            AgentRunLaunchValidationError::WorkspaceDiscoveryBlocked { .. },
        ) => "workspace-blocked",
        AgentRunLaunchRefusal::Validation(_)
        | AgentRunLaunchRefusal::PlanTransition(_)
        | AgentRunLaunchRefusal::Runtime(_)
        | AgentRunLaunchRefusal::Registration(_) => "error",
    }
}

/// The one catalog lookup a refusal's full text takes -- same factoring
/// as `terminal_launch_refusal_text`.
fn agent_run_launch_refusal_text(catalog: &Catalog, refusal: &AgentRunLaunchRefusal) -> String {
    let mut args =
        CatalogArgs::new().trusted_symbol("reason", agent_run_launch_refusal_symbol(refusal));
    if let AgentRunLaunchRefusal::RunLimitExceeded { limit } = refusal {
        args = args.number("limit", *limit);
    }
    catalog.get_with_args("agent-run-launch-refused", &args)
}

/// RFC-022 PR-022-D: the same `AppStatePathProvider::linux_default()`
/// resolution [`open_real_audit_store`] already uses -- RFC-013's own
/// "one resolution, N consumers" convention, extended to a third
/// consumer here rather than inventing new XDG resolution logic in this
/// crate. `None` degrades to "no transcript capture for this launch,"
/// not a launch refusal: `TranscriptCaptureMode::LocalBounded` (what
/// `AgentRunLaunchRequest::new` defaults to) does not reject launch when
/// unavailable -- only `RequiredLocalBounded` does, and this slice does
/// not ask for that.
fn open_real_agent_run_state_root() -> Option<std::path::PathBuf> {
    let path_provider =
        tekstide_core::project::recent::AppStatePathProvider::linux_default().ok()?;
    let state_dir = path_provider.state_dir().to_path_buf();
    std::fs::create_dir_all(&state_dir).ok()?;
    Some(state_dir)
}

/// RFC-022 PR-022-D: the real `Ctrl+Alt+A` path, and `launch_agent_run_with_runtime`'s
/// first production caller (response 218's own required outcome for
/// this slice). Points at [`tekstide_core::agent::AiCliProfile::claude_code_linux_default`],
/// a real, code-defined profile for a genuinely installed AI CLI at
/// `Supervised` compatibility -- not the reference adapter (stays
/// test-only, see `what-the-dialog-must-not-lie-about.md` §4) and not
/// an unconditional stub refusal (response 218: a typed refusal is the
/// honest outcome only when nothing genuinely resolves, not a stand-in
/// for real behaviour).
///
/// `agent_run_limit` is enforced for real in `tekstide-core`
/// (`ProjectSession::attach_agent_launch_plan`); the pre-check below
/// exists only to avoid spawning a real subprocess we already know will
/// be refused -- the same non-authoritative shape [`launch_terminal`]'s
/// own pre-check documents for `terminal_session_limit`, doubly
/// important here since (per response 217) this is the first slice
/// where a user action can spawn a real, transcript-capturing, audited
/// process: whatever limit is enforced is the only thing between a
/// held-down keybinding and unbounded adapter processes.
///
/// No active project is a silent no-op, matching [`attempt_terminal_launch`]'s
/// own precedent.
fn attempt_agent_run_launch(state: &mut State) -> Result<(), AgentRunLaunchRefusal> {
    attempt_agent_run_launch_with_profile(
        state,
        tekstide_core::agent::AiCliProfile::claude_code_linux_default(),
    )
}

/// [`attempt_agent_run_launch`] split out with the profile as a
/// parameter -- the same testability shape [`launch_terminal`]'s own
/// explicit `shell: PathBuf` parameter uses (hardcoded to `/bin/sh` by
/// its one real caller, injectable by tests). Tests use this to exercise
/// the real launch plumbing against a controlled profile without
/// depending on what happens to be installed on the machine running the
/// suite, and without ever pointing it at the real, live product this
/// profile is modelled on.
fn attempt_agent_run_launch_with_profile(
    state: &mut State,
    profile: tekstide_core::agent::AiCliProfile,
) -> Result<(), AgentRunLaunchRefusal> {
    let state_root = open_real_agent_run_state_root();

    let plan = {
        let Some(project) = state.app_shell.state().active_project() else {
            return Ok(());
        };
        if let Some(limit) = project.resource_limits().agent_run_limit
            && project.agent_runs().len() as u32 >= limit
        {
            return Err(AgentRunLaunchRefusal::RunLimitExceeded { limit });
        }

        let mut request = tekstide_core::agent::AgentRunLaunchRequest::new(
            project.id().clone(),
            &profile.id,
            "Interactive Claude Code session",
        );
        if let Some(state_root) = state_root {
            request = request.with_local_bounded_transcript(state_root);
        }

        let validation = tekstide_core::agent::AgentRunLaunchValidator
            .validate(project, &profile, &request)
            .map_err(AgentRunLaunchRefusal::Validation)?;
        tekstide_core::agent::AgentRunLaunchPlan::from_validation(validation, "Claude Code")
            .map_err(AgentRunLaunchRefusal::PlanTransition)?
    };

    let project_id = plan.spec().project_id().clone();
    let mut runtime = LinuxTerminalRuntime::new();
    let (agent_run_id, _events) = state
        .app_shell
        .state_mut()
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .map_err(AgentRunLaunchRefusal::Runtime)?;

    // The run this call just attached always has a terminal id --
    // `launch_prepared_agent_run_with_runtime` attaches the terminal
    // before returning `Ok` -- but this reads the real record rather
    // than assuming it, the same discipline every other post-launch
    // fact in this module is read fresh rather than cached.
    let terminal_id = state
        .app_shell
        .state()
        .active_project()
        .and_then(|project| {
            project
                .agent_runs()
                .iter()
                .find(|run| run.id == agent_run_id)
        })
        .and_then(|run| run.terminal_id.clone());
    let Some(terminal_id) = terminal_id else {
        return Ok(());
    };

    let handle = TerminalRuntimeHandle::new(terminal_id, project_id);
    let pane = crate::surface::terminal::TerminalPane::from_launched(runtime, handle)
        .map_err(AgentRunLaunchRefusal::Registration)?;
    state.terminal_panes.push(pane);
    Ok(())
}

/// RFC-019 PR-019-B: the explorer tree needs a scan to render, and
/// `scan_active_project_explorer_directory` had no production caller
/// anywhere before this slice. A bounded, synchronous, local directory
/// listing does not need a `Task` round trip the way the clipboard read
/// in [`attempt_paste_into_terminal`] does -- it is called directly,
/// after any command that could have changed which project or mode is
/// active. A scan failure is recorded as `ProjectExplorerStatus::Error`
/// by `ProjectContentWorkspace` itself and rendered by
/// `surface::explorer::view`, not silently dropped; the `Result` here is
/// therefore intentionally not propagated further.
fn ensure_explorer_scanned(state: &mut State) {
    let Some(project) = state.app_shell.state().active_project() else {
        return;
    };
    if project.mode() != ProjectMode::Content {
        return;
    }
    if project.content_workspace().explorer_scan().is_some() {
        return;
    }
    let _ = state
        .app_shell
        .scan_active_project_explorer_directory(std::path::PathBuf::new());
    state.explorer_highlight = 0;
}

/// RFC-019 PR-019-B: the explorer tree's own keyboard navigation --
/// Up/Down move the highlight among the rows [`crate::surface::explorer::view`]
/// is currently rendering, Enter on a directory (or the synthetic parent
/// row) re-scans into it. A no-op outside Content mode or without a
/// scan yet -- there is nothing to navigate. Enter on a file row is
/// PR-019-C's job (opening a document); nothing here does it.
///
/// Every borrow of `state.app_shell` ends before `action` is bound, so
/// the mutations below (`state.explorer_highlight`, the real rescan
/// call) do not conflict with it -- computing an owned `Action` first,
/// then matching on it, is what makes that true rather than merely
/// hoped for.
fn handle_explorer_key(state: &mut State, key: &input::KeyPress) {
    enum Action {
        MoveUp,
        MoveDown,
        Navigate(std::path::PathBuf),
        Open(std::path::PathBuf),
        None,
    }

    let Some(project) = state.app_shell.state().active_project() else {
        return;
    };
    if project.mode() != ProjectMode::Content {
        return;
    }
    let Some(scan) = project.content_workspace().explorer_scan() else {
        return;
    };
    let row_count = crate::surface::explorer::visible_rows(scan).len();
    if row_count == 0 {
        return;
    }

    let action = match &key.key {
        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Action::MoveDown,
        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Action::MoveUp,
        keyboard::Key::Named(keyboard::key::Named::Enter) => {
            let rows = crate::surface::explorer::visible_rows(scan);
            match rows.get(state.explorer_highlight) {
                Some(crate::surface::explorer::ExplorerRow::Parent) => Action::Navigate(
                    scan.directory
                        .selected_relative_path
                        .parent()
                        .map(std::path::Path::to_path_buf)
                        .unwrap_or_default(),
                ),
                Some(crate::surface::explorer::ExplorerRow::Node(node))
                    if node.kind == tekstide_core::project::root::ExplorerNodeKind::Directory =>
                {
                    Action::Navigate(node.relative_path.clone())
                }
                // RFC-019 PR-019-C: a file row's Enter now opens it --
                // PR-019-B left this arm absent entirely (a no-op by
                // omission from the match, not by an explicit `_`),
                // since there was no editor yet to open it into.
                Some(crate::surface::explorer::ExplorerRow::Node(node))
                    if node.kind == tekstide_core::project::root::ExplorerNodeKind::File =>
                {
                    Action::Open(node.relative_path.clone())
                }
                _ => Action::None,
            }
        }
        _ => Action::None,
    };

    match action {
        Action::MoveDown => {
            state.explorer_highlight = (state.explorer_highlight + 1).min(row_count - 1);
        }
        Action::MoveUp => {
            state.explorer_highlight = state.explorer_highlight.saturating_sub(1);
        }
        Action::Navigate(path) => {
            let _ = state.app_shell.scan_active_project_explorer_directory(path);
            state.explorer_highlight = 0;
        }
        Action::Open(path) => {
            let _ = state.app_shell.open_active_project_text_document(path);
        }
        Action::None => {}
    }
}

/// RFC-019 PR-019-D: turns a key routed to `MainArea` into an edit,
/// applying [`crate::surface::editor::apply_edit_key`]'s decision (or
/// doing nothing if it returns `None` -- not an edit key, or nothing to
/// remove). A no-op outside Content mode or without an active document,
/// the same shape [`handle_explorer_key`] uses for its own zone.
fn handle_editor_key(state: &mut State, key: &input::KeyPress) {
    let Some(project) = state.app_shell.state().active_project() else {
        return;
    };
    if project.mode() != ProjectMode::Content {
        return;
    }
    let Some(document) = project.content_workspace().active_document() else {
        return;
    };
    let text = document.text().to_string();
    let cursor = document.cursor();

    // RFC-006 Amendment 1: an edit key wins over a navigation key when
    // both could apply (they never do here -- the two functions'
    // `Named` arms do not overlap -- but the order states which of the
    // two this arm treats as primary). `apply_edit_key` writes both
    // halves together (`replace_active_project_text` then
    // `set_active_project_cursor`) so the cursor always lands exactly
    // where the edit left it, never recomputed from a second, possibly
    // stale read of `document.cursor()` after the text already changed.
    if let Some(edit) = crate::surface::editor::apply_edit_key(&text, cursor, &key.key) {
        let _ = state.app_shell.replace_active_project_text(edit.text);
        let _ = state.app_shell.set_active_project_cursor(edit.cursor);
        return;
    }
    if let Some(new_cursor) = crate::surface::editor::navigate_cursor(&text, cursor, &key.key) {
        let _ = state.app_shell.set_active_project_cursor(new_cursor);
    }
}

/// RFC-019 PR-019-D: `Ctrl+S`'s real handler. status-mapping-honesty-fixes
/// (response 196) amended this function's own scope boundary to correct
/// it: it used to re-read `ProjectContentStatus::Conflict` off
/// `workspace.status()` after the save call returned, coupling the
/// shell to however core happened to *summarise* the failure rather than
/// to the failure itself -- which is exactly why fixing `save_active_document`'s
/// own status-mapping defect (Fix 2) broke this function. The reason a
/// save failed is already on the value the call returns
/// (`ProjectContentError::Save(error)` carries `error.decision()`), so
/// this reads it directly. Any other failure is left to `editor::view`'s
/// own `chrome_line`/state rendering (`TextDocumentState::SaveError`),
/// the same "rendered by the surface, not a second notice" shape
/// `terminal_launch_notice` deliberately does *not* use here.
fn attempt_save_active_document(state: &mut State) {
    let Err(tekstide_core::project::ProjectContentError::Save(error)) =
        state.app_shell.save_active_project_text_document()
    else {
        return;
    };
    if error.decision() != tekstide_core::content::SaveDecision::BlockedExternalChange {
        return;
    }
    let Some(project) = state.app_shell.state().active_project() else {
        return;
    };
    let Some(document) = project.content_workspace().active_document() else {
        return;
    };
    // RFC-019 PR-019-E: a `BlockedExternalChange` refusal covers both a
    // genuine conflict (local edits would be lost) and a clean document
    // that merely changed on disk (nothing would be lost) --
    // `TextDocument::save()` already computed which one this is
    // (`block_external_change`'s own `if self.is_dirty()` branch), so
    // this reads `document.state()` rather than re-deriving it.
    let had_local_edits = document.state() == tekstide_core::content::TextDocumentState::Conflict;
    let relative_path = document.target().selected_relative_path.clone();
    state.modal = Some(ModalContent::ExternalChange(ExternalChangeModal {
        relative_path,
        had_local_edits,
        focus: ExternalChangeButton::Dismiss,
    }));
}

/// Terminal launch UX handoff: the real `Ctrl+Alt+T` path, calling
/// [`launch_terminal`] once for `VisibleSlot::Primary` -- a fresh launch
/// always becomes the new `Primary` (bumping whichever session held it
/// to `Hidden`, `assign_terminal_visible_slot`'s own existing behaviour)
/// so the user can type into it immediately, matching
/// [`active_terminal_focus`]'s "only `Primary` receives keystrokes"
/// rule. Rooted in the **real project directory**, not a scratch temp
/// dir -- unlike the diagnostic demo/measurement paths, a terminal a
/// user actually asked for must open where their project is.
///
/// No active project is a silent no-op (`Ok(())`, nothing pushed),
/// matching every other `AppCommand`'s existing precedent
/// (`OpenActiveProjectWorkspace`, `ToggleActiveProjectMode`) -- not a
/// refusal worth a typed notice, since there is nothing yet to refuse.
fn attempt_terminal_launch(state: &mut State) -> Result<(), TerminalLaunchRefusal> {
    let Some(project) = state.app_shell.state().active_project() else {
        return Ok(());
    };
    let project_id = project.id().clone();
    let root = project.canonical_root_path().clone();

    let mut audit_store = open_real_audit_store(&state.app_shell);
    let mut audit_health = tekstide_core::audit::AuditHealth::default();
    let pane = launch_terminal(
        &mut state.app_shell,
        project_id,
        "terminal",
        root,
        tekstide_core::domain::VisibleSlot::Primary,
        audit_store.as_mut(),
        &mut audit_health,
    )?;
    state.terminal_panes.push(pane);
    Ok(())
}

/// RFC-018 PR-018-B: the one production call site for
/// `TerminalPane::write_input`. Both `RoutedInput::Terminal` (typed
/// keystrokes) and `Message::TerminalPasteResolved` (paste, once
/// `evaluate` has decided `Allow`) call this rather than writing
/// directly, so `state.modal.is_none()` is checked in exactly one
/// place -- `pr-018-b-paste-ingress.md`'s own warning against the
/// alternative: "two arms, two guards, and the second one drifts."
///
/// Defense in depth, not the modal-exclusivity boundary itself:
/// `non_modal_subscription` structurally cannot produce
/// `RoutedInput::Terminal` while a modal is open (see `input`'s module
/// doc), and the paste path re-derives `TerminalTrustedUiState` from
/// `state.modal` fresh before `evaluate` is ever called. Checked here
/// anyway, at the one place bytes would actually reach a PTY, rather
/// than trusting either upstream property alone -- ablated in
/// `shell::tests`.
fn write_terminal_input(state: &mut State, target: &TerminalId, bytes: &[u8]) -> bool {
    if state.modal.is_some() {
        return false;
    }
    let Some(pane) = state
        .terminal_panes
        .iter_mut()
        .find(|pane| pane.terminal_id() == target)
    else {
        return false;
    };
    pane.write_input(bytes);
    true
}

/// RFC-018 PR-018-B: `Ctrl+Shift+V`'s handler. `iced` has no synchronous
/// clipboard access, so the policy decision cannot be made here -- this
/// function's only job is identifying *which* terminal the paste
/// targets before the async round trip starts. No active project or no
/// terminal currently focused is a silent no-op: there is nothing to
/// address a paste to, the same "no active project" precedent
/// `attempt_terminal_launch` already sets, and reading the clipboard at
/// all when there is nowhere to paste would be pointless I/O.
fn attempt_paste_into_terminal(state: &mut State) -> Task<Message> {
    state.terminal_paste_notice = None;
    let Some(target) = active_terminal_focus(state) else {
        return Task::none();
    };
    iced::clipboard::read().map(move |content| Message::TerminalPasteResolved {
        target: target.clone(),
        content,
    })
}

/// RFC-018 PR-018-B: clipboard content is untrusted, arbitrary-length,
/// and attacker-influenced (`implementation-handoff.md` "things that
/// will bite") -- bounded here, before `TerminalInputPolicy::evaluate`
/// ever sees it, so a multi-megabyte clipboard cannot become an
/// unbounded PTY write. Not a classification decision (that stays
/// `evaluate`'s alone): a raw resource bound on what gets read at all.
///
/// **256 KiB, reasoned on paste's own terms (response 169 non-blocking:
/// the previous version reused `read_available_bounded_for`'s 64 KiB
/// cap, which is not itself a settled number -- `future-work.md` names
/// it as needing a real block/grow/drop-and-report decision -- so
/// borrowing it tied paste sizing to a number already slated to
/// change).** A paste into an interactive shell is a command, a
/// heredoc, a short script, or a config snippet a user is inspecting or
/// editing -- not a document or a log file, which belong in an editor,
/// not a PTY. 256 KiB is generous for any of those (many multiples of a
/// realistic shell script) while still bounding what a single keypress
/// can commit to writing.
///
/// **Refuses rather than truncates (response 169 Required).** A cap
/// applied by truncating before classification lets truncation change
/// the classification itself, and silently writes a prefix of what the
/// user actually copied -- see `Message::TerminalPasteResolved`'s
/// handler for the full reasoning. `evaluate` must see the paste's
/// real, complete bytes, never a prefix, so content over the cap is
/// refused whole rather than shortened.
const MAX_PASTE_BYTES: usize = 256 * 1024;

/// `None` if `content` exceeds [`MAX_PASTE_BYTES`] -- the caller must
/// refuse the whole paste, not write (or preview) a truncated prefix of
/// it. Returns the real `String`, not bytes: `evaluate` takes `&[u8]`
/// via `.as_bytes()`, but PR-018-C's confirmation dialog needs the
/// `String` itself (for its preview and, on Accept, for the exact bytes
/// it writes), and keeping one owned value rather than converting back
/// and forth avoids a second, possibly-diverging copy of "what the user
/// pasted."
fn content_within_bound(content: Option<String>) -> Option<String> {
    let content = content.unwrap_or_default();
    if content.len() > MAX_PASTE_BYTES {
        None
    } else {
        Some(content)
    }
}

/// RFC-018 PR-018-C, Open Question 2 answered: **preview**, not only
/// describe. RFC-018 itself frames preview as "more useful," to be
/// decided once escaping is in place -- it now is
/// (`text_safety::quote_untrusted`), so the choice is about usefulness
/// rather than risk, as the RFC asked. Bounded to 500 characters
/// (`str::chars`, never a raw byte index -- see
/// `paste_confirmation_modal_view`'s doc comment for why truncation
/// happens on the raw content, before escaping) -- enough to recognise
/// a real command, heredoc, or config snippet at a glance without
/// rendering a fixed-size dialog around up to 256 KiB of content.
const PASTE_PREVIEW_CHAR_LIMIT: usize = 500;

/// RFC-018 PR-018-B/PR-018-C: the one place `state.modal` becomes a
/// `TerminalTrustedUiState` -- kept singular per
/// `pr-018-b-paste-ingress.md` ("keep that derivation in one
/// function... when RFC-022's approval dialog arrives it becomes a
/// second contributor to the same state"). Now that PR-018-C's real
/// paste dialog exists, it maps to `PasteConfirmationActive` --
/// distinguished from the `TEKSTIDE_LAYER_DEMO` placeholder, which still
/// maps to `SecurityDialogActive`, the most generic of the five
/// variants, since it represents no real dialog kind of its own.
/// Revisit again when RFC-022's approval dialog becomes a third
/// contributor.
fn trusted_ui_state(state: &State) -> TerminalTrustedUiState {
    match &state.modal {
        None => TerminalTrustedUiState::Inactive,
        Some(ModalContent::PasteConfirmation(_)) => TerminalTrustedUiState::PasteConfirmationActive,
        // RFC-019 PR-019-D: the external-change conflict dialog is not a
        // terminal-input concern at all -- it falls into the same
        // generic bucket `LayerDemo` does, for the same reason: neither
        // represents a real *terminal-paste* dialog kind of its own. It
        // still must map to something other than `Inactive`, though --
        // modal exclusivity (this state's only real consumer today)
        // needs every open modal to read as active, not just the two
        // paste-specific ones.
        Some(ModalContent::LayerDemo { .. }) | Some(ModalContent::ExternalChange(_)) => {
            TerminalTrustedUiState::SecurityDialogActive
        }
    }
}

/// RFC-018 PR-018-B: a paste that did not reach the PTY, and why -- a
/// typed answer the shell can render, matching `TerminalLaunchRefusal`'s
/// own "the user pressed a key and is owed a visible answer" shape.
/// Deliberately cannot represent `Allow`: constructed only at the two
/// call sites that produce one (`update`'s `TerminalPasteResolved`
/// handler), so a successful paste can never accidentally be stored as
/// a notice -- the same "make the invalid state unrepresentable" shape
/// `ModalAbsent` and `VerifiedCwd` already use. **No longer represents
/// `RequiresConfirmation`** (PR-018-C): that decision now opens the real
/// dialog instead of being forced into a block.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TerminalPasteRefusal {
    Blocked(TerminalInputDecisionReason),
    /// Response 169 Required: content over [`MAX_PASTE_BYTES`] is
    /// refused whole, before `evaluate` is ever called -- not a
    /// `TerminalInputDecision` at all, so it has no reason code this
    /// variant could carry.
    TooLarge,
}

/// A compile-time literal symbol, not the displayed word -- the words
/// live in `en.ftl`'s `terminal-paste-refused` select expression, the
/// same division of labour `terminal_launch_refusal_symbol` already
/// uses.
fn terminal_paste_refusal_symbol(refusal: &TerminalPasteRefusal) -> &'static str {
    match refusal {
        TerminalPasteRefusal::TooLarge => "too-large",
        TerminalPasteRefusal::Blocked(reason) => match reason {
            TerminalInputDecisionReason::ControlContainingPasteBlocked => "control",
            TerminalInputDecisionReason::WrongProject
            | TerminalInputDecisionReason::WrongTerminal => "wrong-target",
            TerminalInputDecisionReason::PasteBlockedByTrustedUi(_) => "trusted-ui",
            // Structurally unreachable through `evaluate` today -- this
            // reason is only ever attached to `RequiresConfirmation`,
            // never `Block`. Kept as an explicit arm rather than a
            // wildcard so a future change to `evaluate` that did pair
            // it with `Block` would force a deliberate choice here,
            // not inherit one silently.
            TerminalInputDecisionReason::MultilinePasteRequiresConfirmation => "multiline",
        },
    }
}

fn terminal_paste_refusal_text(catalog: &Catalog, refusal: &TerminalPasteRefusal) -> String {
    catalog.get_with_args(
        "terminal-paste-refused",
        &CatalogArgs::new().trusted_symbol("reason", terminal_paste_refusal_symbol(refusal)),
    )
}

/// RFC-017 PR-017-E: launches real, filtered PTY sessions for Terminal
/// Mode's main area to render, gated behind `TEKSTIDE_TERMINAL_DEMO` --
/// the same env-gated-demo convention as `TEKSTIDE_LAYER_DEMO`/
/// `TEKSTIDE_MEASURE_CRITERION`. Three scratch, temp-dir sessions are
/// launched -- `Primary`, `Secondary` (matching
/// `visible_terminal_limit`'s default of 2, and `TerminalPanePolicy`'s
/// own `max_visible_panes`), and one deliberately assigned `Hidden` from
/// the start so the hidden-session decision this slice makes
/// (`surface::terminal`'s module doc: retained in memory, still polled)
/// has something real to demonstrate.
///
/// **Terminal launch UX handoff: now a thin caller of [`launch_terminal`]**,
/// the same function `attempt_terminal_launch`'s real `Ctrl+Alt+T` path
/// calls -- "the demo becomes a caller that launches N terminals through
/// the same function a keybinding calls," not a second construction
/// path for PTY-backed sessions (the exact shape PR-017-B/C spent two
/// slices proving absent).
///
/// Requires an active project (a CLI project-path argument); returns an
/// empty `Vec` (silently -- this is a diagnostic path, not a
/// user-facing feature) otherwise, if the env var is unset, or if a
/// given pane's launch/registration is refused.
///
/// The store itself is opened here, after both early-return gates,
/// rather than unconditionally in `State::new` -- response 152
/// Required 1: an unconditional open creates the database file (schema
/// and all) on every launch regardless of this env var, which is
/// exactly the claim the README's privacy section exists to get right.
/// `open_real_audit_store` returning `None` (no writable state root) is
/// a no-op, the same "checked but usually harmless to skip" shape the
/// demo gate itself already uses.
fn launch_terminal_demo_panes(
    app_shell: &mut ApplicationShell,
    audit_health: &mut tekstide_core::audit::AuditHealth,
) -> Vec<crate::surface::terminal::TerminalPane> {
    if std::env::var("TEKSTIDE_TERMINAL_DEMO").is_err() {
        return Vec::new();
    }
    let Some(project_id) = app_shell.state().active_project_id().cloned() else {
        return Vec::new();
    };

    let mut audit_store = open_real_audit_store(app_shell);
    let mut panes = Vec::new();
    for (index, slot) in [
        tekstide_core::domain::VisibleSlot::Primary,
        tekstide_core::domain::VisibleSlot::Secondary,
        tekstide_core::domain::VisibleSlot::Hidden,
    ]
    .into_iter()
    .enumerate()
    {
        let root = std::env::temp_dir().join(format!(
            "tekstide-terminal-demo-{}-{index}",
            std::process::id()
        ));
        if std::fs::create_dir_all(&root).is_err() {
            continue;
        }
        let Ok(pane) = launch_terminal(
            app_shell,
            project_id.clone(),
            format!("terminal demo {}", index + 1),
            root,
            slot,
            audit_store.as_mut(),
            audit_health,
        ) else {
            continue;
        };
        panes.push(pane);
    }
    panes
}

/// RFC-017 PR-017-G: the background output flood for the `TerminalFlood`
/// measurement criterion -- **bounded by wall-clock time, not an
/// unbounded `while true`** loop. The RFC-014 spike crate's own
/// `send_flood_script_once` (`tekstide-gui-spike`, deleted 2026-08-04 --
/// see
/// `rfcs/handoffs/014-desktop-gui-substrate-and-terminal-rendering/spike-crate-deletion.md`;
/// superseded, RFC-014 PR-014-E's C3 precedent) backgrounds an infinite
/// loop that only ever stops if something kills it; RFC-017's own review
/// gate asks for "bounded
/// background output" specifically, so this loop instead computes its
/// own end time once and checks the wall clock only every 2,000
/// iterations, self-terminating after 30 seconds -- response 154's "to
/// tighten" note: an earlier 120s bound was disclosed-but-avoidable
/// margin (a stray reparented-to-init process on the owner's machine
/// for up to two minutes past an early exit); 30s is still generous
/// over RFC-014/RFC-015's own C2/C4 precedent (1,100 repeats at a 15ms
/// pace finished in ~17 seconds) without ever needing this process to
/// kill it.
///
/// **Response 155's own finding, fixed here: the first version checked
/// `$(date +%s)` in the loop *condition*, so every single output line
/// cost a `fork`+`exec` of `date`.** Measured at 121.7 KiB/s -- 173×
/// below the same loop's throughput with the per-iteration fork
/// removed (20.6 MiB/s, verified by the reviewer) -- never exceeding
/// the 64KiB read cap per 5ms poll window, so no backpressure, no
/// chunk-boundary stress: a trickle much closer to idle than to flood,
/// under a script whose own name said otherwise. Checking the clock
/// only every 2,000 iterations keeps the `date` cost negligible while
/// still bounding real duration close to 30s (worst case, a few
/// thousand extra lines past the bound before the next check; harmless
/// -- the bound exists to avoid an indefinite process, not to be exact
/// to the millisecond). Backgrounded (`&`) so the shell stays
/// interactive for the measured keystrokes written into the same pty
/// concurrently.
const FLOOD_SCRIPT: &str = "i=0; end=$(( $(date +%s) + 30 )); \
    while :; do \
    printf 'tekstide-flood-%08d-filler-filler-filler-filler-filler\\n' \"$i\"; \
    i=$((i+1)); \
    [ $((i % 2000)) -eq 0 ] && [ \"$(date +%s)\" -ge \"$end\" ] && break; \
    done &\n";

/// RFC-017 PR-017-G: launches exactly one live, filtered PTY terminal
/// pane for the `TerminalFlood` criterion (or for
/// `TEKSTIDE_TERMINAL_FLOOD_DEMO`'s non-contamination control, the same
/// scenario with measurement deliberately absent -- see `State::new`),
/// registers it with `tekstide-core` and switches the active project
/// into `ProjectMode::TerminalImmersion` (the real `AppCommand`, not a
/// shell-local shortcut -- `ProjectSession::new` always starts in
/// `Content`, so dispatching `ToggleActiveProjectMode` once is enough)
/// so the pane genuinely renders every `view()` cycle exactly as a real
/// interactive session would, then starts [`FLOOD_SCRIPT`] with one real
/// `write_input` call before returning.
///
/// Deliberately separate from [`launch_terminal_demo_panes`] (see
/// `State::new`'s call site for why) and requires an active project --
/// **panics** rather than silently measuring nothing if one is missing,
/// since running this scenario with no real pane to write into would
/// otherwise produce samples (or a control run) that look real but
/// measure a no-op; the operator error (forgetting the CLI project-path
/// argument) should be loud, not a quietly meaningless log file.
fn launch_measurement_terminal_pane(
    app_shell: &mut ApplicationShell,
) -> Vec<crate::surface::terminal::TerminalPane> {
    let project_id = app_shell.state().active_project_id().cloned().expect(
        "TEKSTIDE_MEASURE_CRITERION=terminal_flood / TEKSTIDE_TERMINAL_FLOOD_DEMO requires an \
         active project -- pass a project path on the CLI",
    );
    let root = std::env::temp_dir().join(format!(
        "tekstide-terminal-flood-measure-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root)
        .expect("must be able to create a scratch directory for the measurement terminal");
    let (mut pane, session) = crate::surface::terminal::TerminalPane::launch(
        project_id,
        "terminal flood measurement",
        root,
        std::path::PathBuf::from("/bin/sh"),
    )
    .expect("must be able to launch a real shell for the measurement terminal");
    let terminal_id = session.id.clone();
    app_shell
        .state_mut()
        .attach_terminal_session(session)
        .expect("a freshly launched session must attach cleanly");
    app_shell
        .state_mut()
        .assign_terminal_visible_slot(&terminal_id, tekstide_core::domain::VisibleSlot::Primary)
        .expect("the sole measurement pane must be assignable to Primary");
    app_shell.dispatch(tekstide_core::command::AppCommand::ToggleActiveProjectMode);
    // RFC-017 Amendment 1, PR-A1-D: `TEKSTIDE_TERMINAL_FLOOD_QUIET` skips
    // starting `FLOOD_SCRIPT` -- a diagnostic-only, checked-but-usually-
    // absent toggle (same shape as every other measurement env var here)
    // for a zero-contention baseline against the same pane-launch/
    // dispatch/instrumentation path a real flood run uses, so an
    // elevated figure under flood can be attributed to the flood itself
    // rather than argued from the flood run's numbers alone.
    if std::env::var("TEKSTIDE_TERMINAL_FLOOD_QUIET").is_err() {
        pane.write_input(FLOOD_SCRIPT.as_bytes());
    }
    vec![pane]
}

/// RFC-017 PR-017-F: resolves and opens the real, durable audit store
/// at `<tekstide-state-root>/audit/` -- `<tekstide-state-root>` is the
/// exact same directory `main.rs`'s `RecentProjectStore` already
/// resolves (`AppStatePathProvider::linux_default`), reused rather than
/// independently re-derived, per RFC-013's own diagram ("one resolution,
/// two consumers," `AppStatePathProvider::state_dir`'s own doc comment).
/// `None` on any failure (no `HOME`/`XDG_STATE_HOME`, the directory
/// cannot be created, or the store fails to open) -- the same
/// fail-silent, log-nothing-to-the-user shape appropriate for a
/// diagnostic/observability path that must never block the app from
/// starting.
fn open_real_audit_store(app_shell: &ApplicationShell) -> Option<tekstide_core::audit::AuditStore> {
    let path_provider =
        tekstide_core::project::recent::AppStatePathProvider::linux_default().ok()?;
    let project_roots = app_shell
        .state()
        .projects()
        .iter()
        .map(|project| project.canonical_root_path().clone())
        .collect();
    open_audit_store(path_provider.state_dir(), project_roots)
}

/// Factored out from [`open_real_audit_store`] so tests can open a
/// real, file-backed store against a deterministic temp directory
/// instead of the real `XDG_STATE_HOME`/`HOME`-resolved one -- the same
/// reason `RecentProjectStore::new` takes an already-resolved
/// `AppStatePathProvider` rather than resolving it itself.
fn open_audit_store(
    state_dir: &std::path::Path,
    project_roots: Vec<std::path::PathBuf>,
) -> Option<tekstide_core::audit::AuditStore> {
    std::fs::create_dir_all(state_dir).ok()?;
    let request = tekstide_core::audit::AuditPathRequest::new(state_dir, project_roots);
    let storage_path = tekstide_core::audit::AuditPathResolver
        .resolve(request)
        .ok()?;
    tekstide_core::audit::AuditStore::open(storage_path).ok()
}

/// RFC-017 Amendment 1, PR-A1-C: one [`Subscription`] per pane currently
/// tracked in `state.terminal_panes`, replacing the fixed 50ms
/// `terminal_poll_subscription` this amendment removes. Added to the
/// real subscription tree by [`subscription`] under the same "only when
/// panes exist" condition the old tick used, so this changes nothing
/// about `subscription`'s reviewed non-modal/modal routing for any
/// normal run beyond what drives `poll`/`check_exit`.
///
/// A pane whose `wake_notifier()` fails (`eventfd(2)` resource
/// exhaustion, `TerminalPane::wake_notifier`'s own doc) is silently
/// excluded here rather than surfaced as an error -- that pane simply
/// stops receiving event-driven wakes until something else (a future
/// wake, if the failure was transient and a later `subscription()` call
/// succeeds) restores it; there is no rendering/UX surface for this
/// slice to report it on (out of scope, per the amendment's own text),
/// and failing the whole subscription tree over one pane's `eventfd`
/// would take every other tracked pane down with it.
fn terminal_wake_subscriptions(
    panes: &[crate::surface::terminal::TerminalPane],
) -> Vec<Subscription<Message>> {
    panes
        .iter()
        .filter_map(|pane| {
            pane.wake_notifier()
                .ok()
                .map(|notifier| terminal_wake_subscription(pane.terminal_id().clone(), notifier))
        })
        .collect()
}

/// One pane's own wake subscription, keyed by its `TerminalId` so
/// `iced` recognises the same pane across `subscription()` rebuilds and
/// reuses the already-running bridging thread rather than spawning a
/// new one each time -- `terminal_bridge_thread_count_is_stable_across_many_view_rebuilds`
/// (in `shell::tests`) proves this rather than assuming `Subscription::run_with`'s
/// documented dedup behaviour holds here unverified.
fn terminal_wake_subscription(
    terminal_id: tekstide_core::domain::TerminalId,
    notifier: tekstide_core::runtime::terminal::WakeNotifier,
) -> Subscription<Message> {
    Subscription::run_with(
        TerminalWakeSource {
            terminal_id,
            notifier,
        },
        terminal_wake_stream,
    )
}

/// `Subscription::run_with`'s own identity data. **`Hash` is
/// hand-written, not derived, and deliberately ignores `notifier`**:
/// `subscription()` builds a fresh `TerminalWakeSource` (with a freshly
/// duplicated `eventfd`) on every rebuild, but only the *first* one for
/// a given `terminal_id` should ever reach [`terminal_wake_stream`] --
/// `iced` decides that by comparing hashes across rebuilds, so if the
/// notifier's own fd number were part of the hash, every rebuild would
/// look like a brand new subscription and spawn a brand new bridging
/// thread. Every later, redundant `TerminalWakeSource` for an
/// already-running `terminal_id` is simply dropped once built (closing
/// its own duplicated fd harmlessly) without its `notifier` ever being
/// used -- a few wasted `dup`/`close` syscalls per rebuild, not a
/// leaked thread.
struct TerminalWakeSource {
    terminal_id: tekstide_core::domain::TerminalId,
    notifier: tekstide_core::runtime::terminal::WakeNotifier,
}

impl std::hash::Hash for TerminalWakeSource {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.terminal_id.hash(state);
    }
}

/// The bridging stream itself: spawns one dedicated OS thread that
/// blocks on [`WakeNotifier::block_until_woken`] (a real `poll(2)` park,
/// not a timer) and forwards exactly the terminal's own id into `iced`'s
/// async world each time -- **response 205's first constraint**: the
/// message carries a `TerminalId` and nothing else, never bytes, so
/// terminal content can never become `Debug`-formattable or `Clone`-able
/// through `Message`, and P2's "one consumer" property does not depend
/// on `iced`'s own internal queueing.
///
/// The async block itself does no blocking work -- it only keeps the
/// stream alive (`std::future::pending`) while the real work happens on
/// the spawned thread, matching the pattern `iced_futures`'s own
/// `Subscription::run` documentation shows for bridging a synchronous
/// worker into an async `Stream`.
fn terminal_wake_stream(
    source: &TerminalWakeSource,
) -> impl iced::futures::Stream<Item = Message> + use<> {
    let terminal_id = source.terminal_id.clone();
    let notifier = source.notifier.try_clone();
    iced::stream::channel(1, async move |mut output| {
        let Ok(notifier) = notifier else {
            // Duplicating an already-open eventfd should not fail in
            // practice; if it somehow does, this pane just never wakes
            // again through this stream -- no data was lost (nothing
            // was read), only the event-driven trigger for this one
            // pane, same degradation as `terminal_wake_subscriptions`'s
            // own `wake_notifier()` failure case.
            std::future::pending::<()>().await;
            return;
        };
        std::thread::spawn(move || {
            loop {
                let more_coming = notifier.block_until_woken();
                let send_result = iced::futures::executor::block_on(
                    output.send(Message::TerminalWoke(terminal_id.clone())),
                );
                if send_result.is_err() || !more_coming {
                    return;
                }
            }
        });
        std::future::pending::<()>().await;
    })
}

/// `OpenProjectBoard` and, since PR-015-E, `ToggleProjectMode` map to
/// existing `AppCommand`s. `LaunchTerminal` is the terminal-launch-UX
/// handoff's addition -- its `AppCommand` arm only handles the
/// route/mode half; `update`'s `Shell(shell_input)` handler special-
/// cases this one action to also perform the real spawn (I/O
/// `tekstide-core::shell::dispatch` cannot do). `OpenCommandPalette` has
/// a real, reserved binding (`KeybindingPolicy::linux_mvp()`) but no
/// command palette feature exists yet to dispatch to; every other
/// `NavigationAction` has no default binding at all until RFC-023
/// supplies one. Not a placeholder -- an honest reflection of what is
/// real right now.
fn app_command_for(action: NavigationAction) -> Option<AppCommand> {
    match action {
        NavigationAction::OpenProjectBoard => Some(AppCommand::OpenProjectBoard),
        NavigationAction::ToggleProjectMode => Some(AppCommand::ToggleActiveProjectMode),
        NavigationAction::LaunchTerminal => Some(AppCommand::LaunchTerminal),
        // RFC-022 PR-022-D: mirrors `LaunchTerminal` -- the actual launch
        // (profile resolution, validation, PTY spawn, registration) is
        // real I/O and lives in `update`'s `Shell` arm, dispatched
        // alongside this command rather than inside it, the same split
        // `LaunchTerminal` already uses.
        NavigationAction::LaunchAgentRun => Some(AppCommand::LaunchAgentRun),
        // RFC-022 PR-022-D: the route to an already-running run's detail
        // view -- no I/O, so no `update` special-case is needed the way
        // `LaunchAgentRun` above needs one.
        NavigationAction::OpenCurrentAgentRunDetail => Some(AppCommand::OpenActiveProjectSurface(
            ProjectOpenSurface::AgentRunDetail,
        )),
        // RFC-018 PR-018-B: paste needs no core route/mode change --
        // `update`'s `Shell` arm special-cases it directly, the same
        // shape `LaunchTerminal` uses for the half of its own work that
        // isn't a `dispatch`-able command either.
        NavigationAction::PasteIntoTerminal
        // RFC-019 PR-019-D: save needs no core route/mode change either --
        // `update`'s `Shell` arm special-cases it directly, the same shape
        // `PasteIntoTerminal` uses above.
        | NavigationAction::SaveActiveDocument
        | NavigationAction::OpenCommandPalette
        | NavigationAction::SwitchActiveProject
        | NavigationAction::CycleVisibleTerminalSession
        | NavigationAction::OpenPendingApproval
        | NavigationAction::OpenDiffReview
        | NavigationAction::OpenSafeCloseDialog => None,
    }
}

/// The check `pr-015-c-input-routing.md` requires before a `TextStream`
/// may be delivered: "a stale or cross-project id is dropped, not
/// best-effort delivered." **Gets its first real caller this slice**:
/// PR-017-D left this `#[allow(dead_code)]` because its demo pane was
/// deliberately not registered as a `TerminalSession` on the real active
/// project. RFC-017 PR-017-E's demo panes are (`launch_terminal_demo_panes`),
/// so this now-real, core-backed check is what actually gates delivery
/// in `update`. Proven directly against `ApplicationShell` fixtures in
/// `shell::tests`.
pub(crate) fn terminal_stream_targets_a_live_terminal(
    app_shell: &ApplicationShell,
    stream: &TextStream,
) -> bool {
    app_shell
        .state()
        .active_project()
        .and_then(|project| project.terminal_session(stream.target()))
        .is_some()
}

/// RFC-018 PR-018-G: the full-window dimming layer behind whatever modal
/// is open. Applied to the same `center(modal_view)` container `opaque`
/// already wraps -- not a second widget, not a second input-capturing
/// surface. `center` already fills `Length::Fill` (the whole window), so
/// `opaque`'s existing click-capture bounds are already full-window;
/// adding a background colour to that same container changes nothing
/// about what captures input, only what is drawn. This is deliberate:
/// `SubscriptionMode::for_modal` plus the `is_none()` guard at the write
/// site is the one mechanism that actually protects the user (see
/// `subscription`'s own doc comment below) -- the scrim must stay
/// additive cosmetics on top of it, never a second one.
///
/// Full-window, chrome included, is the whole argument: the spatial tell
/// PR-018-E's evidence work found broken was content-dependent because
/// the *pasted content* controlled the dialog's size. A scrim covering
/// only the content region would reproduce that exact weakness --
/// unlike pasted content, the attacker does not control the window size,
/// so this must cover area a terminal pane structurally cannot draw
/// into (the session bar, the window margin) for the content-independence
/// argument to hold at all.
fn modal_scrim_style(theme: crate::theme::Theme) -> impl Fn(&iced::Theme) -> container::Style {
    move |_base_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(theme.scrim())),
        ..container::Style::default()
    }
}

pub fn view(state: &State) -> Element<'_, Message> {
    let base: Element<'_, Message> =
        column![top_bar(state), content_area(state), status_bar(state)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

    if let Some(modal) = &state.modal {
        let modal_view = match modal {
            ModalContent::LayerDemo { focus } => layer_composition_demo_modal(state, *focus),
            ModalContent::PasteConfirmation(paste_modal) => {
                paste_confirmation_modal_view(state, paste_modal)
            }
            ModalContent::ExternalChange(external_change_modal) => {
                external_change_modal_view(state, external_change_modal)
            }
        };
        let scrim = center(modal_view).style(modal_scrim_style(state.theme));
        stack![base, opaque(scrim)].into()
    } else {
        base
    }
}

/// Structural proof this module cannot bypass modal exclusivity for the
/// *call*: `route_non_modal_input` needs an `input::ModalAbsent`,
/// obtainable only by checking `state.modal` itself -- there is no other
/// way to reach [`non_modal_subscription`]. See `input`'s module doc for
/// why deleting this `match` is a compile error, not a behaviour change.
///
/// **What that alone does not prove** (response 130 Required 1): actual
/// exclusivity -- that `SurfaceInput`/`TextStream` are never produced
/// while a modal is shown -- also depends on `iced` tearing down the
/// non-modal subscription (and the `ModalAbsent` it captured, which is
/// `Copy` and therefore can outlive the instant it was checked) the
/// moment this function starts returning [`input::SubscriptionMode::Modal`]
/// instead. That is a real dependency on `iced`'s subscription-rebuild
/// lifecycle, not a second type-level guarantee -- named here rather
/// than left implicit, and `input::SubscriptionMode::for_modal` is
/// tested directly (`shell::tests`) so at least the branch this function
/// picks is asserted, even though the framework half is not.
pub fn subscription(state: &State) -> Subscription<Message> {
    // Checked first, ahead of modal/non-modal routing entirely: a
    // measurement run is a special, bounded, self-terminating mode (see
    // `measurement`'s module doc), never part of ordinary interactive
    // use -- `state.measurement` is `None` unless the env var is set, so
    // this branch changes nothing about PR-015-C's reviewed structure
    // for any normal run.
    //
    // RFC-017 PR-017-G: `TerminalFlood` also needs its one live pane
    // polled during the run -- otherwise its background flood would be
    // written into a pty nothing ever reads, and the contention this
    // criterion exists to measure (poll's PTY-read/VTE-processing cost
    // competing with input handling on the same executor) would never
    // happen. Batched in exactly like the non-measurement path below
    // does for `state.terminal_panes`, rather than a second copy of that
    // condition living only inside `measurement_subscription`.
    if let Some(measurement) = &state.measurement {
        let measurement_routing = measurement_subscription(measurement.criterion());
        return if state.terminal_panes.is_empty() {
            measurement_routing
        } else {
            let mut subscriptions = terminal_wake_subscriptions(&state.terminal_panes);
            subscriptions.push(measurement_routing);
            Subscription::batch(subscriptions)
        };
    }

    let routing = match input::SubscriptionMode::for_modal(&state.modal) {
        input::SubscriptionMode::NonModal(proof) => {
            non_modal_subscription(proof, state.focus, active_terminal_focus(state))
                .map(Message::Input)
        }
        input::SubscriptionMode::Modal => modal_subscription(),
    };

    // RFC-017 PR-017-C: only added when a demo pane exists (the env var
    // was set), so this changes nothing about the routing above for any
    // normal run -- the same "checked but usually absent" shape the
    // measurement branch above already uses.
    if !state.terminal_panes.is_empty() {
        let mut subscriptions = terminal_wake_subscriptions(&state.terminal_panes);
        subscriptions.push(routing);
        Subscription::batch(subscriptions)
    } else {
        routing
    }
}

/// `Startup` reuses the RFC-014 spike's exact mechanism (subscribe
/// `frames()`, record the first one, exit) -- safe here specifically
/// because the process exits immediately after, so there is no
/// *sustained* redraw-forcing during any real interactive session for
/// it to contaminate. `Typing` never subscribes to `frames()` at all --
/// see `measurement`'s module doc for why -- and instead pairs a
/// measurement-only keyboard listener with a periodic tick used solely
/// to detect "target sample count reached, time to self-exit."
fn measurement_subscription(criterion: measurement::Criterion) -> Subscription<Message> {
    match criterion {
        measurement::Criterion::Startup => iced::window::frames().map(Message::MeasurementFrame),
        measurement::Criterion::Typing => {
            measured_key_subscription(Message::MeasuredKey as fn(std::time::Instant) -> Message)
        }
        // RFC-015 PR-015-E: identical shape to `Typing`'s arm -- the same
        // measurement key, the same self-terminating tick -- differing
        // only in which `Message` variant (and therefore which `update`
        // handler) the keystroke produces. See `measured_key_subscription`.
        measurement::Criterion::ModeSwitch => measured_key_subscription(
            Message::MeasuredModeSwitch as fn(std::time::Instant) -> Message,
        ),
        // RFC-017 PR-017-G: same shape again; `subscription` additionally
        // batches in `terminal_poll_subscription()` for this criterion
        // (see `subscription`'s own doc) so the concurrent flood this
        // criterion's one pane is running actually gets polled during
        // the run, not just written into and left unread.
        measurement::Criterion::TerminalFlood => measured_key_subscription(
            Message::MeasuredTerminalInput as fn(std::time::Instant) -> Message,
        ),
    }
}

/// Shared by `Typing` and `ModeSwitch`: a measurement-only keyboard
/// listener for [`measurement::MEASURED_KEY_CHARACTER`], paired with the
/// periodic self-exit tick. Parameterized on which `Message` variant to
/// produce so the two criteria's `update` handlers stay distinct (one
/// appends to a synthetic document, the other dispatches a real
/// `AppCommand`) without duplicating this subscription's shape twice.
fn measured_key_subscription(
    to_message: fn(std::time::Instant) -> Message,
) -> Subscription<Message> {
    Subscription::batch([
        keyboard::listen()
            .with(to_message)
            .filter_map(|(to_message, event)| match event {
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref character),
                    ..
                } if character.as_str() == measurement::MEASURED_KEY_CHARACTER => {
                    Some(to_message(std::time::Instant::now()))
                }
                _ => None,
            }),
        iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::MeasurementTick),
    ])
}

/// RFC-017 PR-017-D/E: a terminal is "focused" for input-routing
/// purposes exactly when `FocusZone::MainArea` is focused *and* the
/// active project is in `TerminalImmersion` mode, matching
/// `main_area_view`'s own substitution condition. Outside that, `None`:
/// a hidden or content-mode pane must not receive keystrokes just
/// because it happens to exist.
///
/// **Which of up to two visible panes, decided this slice**: the one
/// holding `VisibleSlot::Primary`. Multiple visible panes competing for
/// one keyboard-input target is a real question this slice does not
/// have a UI feature (a per-pane click-to-focus, a cycle keybinding) to
/// answer against yet -- picking `Primary` as the sole input target is
/// a deliberate, narrower scope than "solve pane-to-pane input focus,"
/// consistent with `visible_terminal_limit`'s own model where `Primary`
/// is the first slot. Revisit if/when a feature needs the `Secondary`
/// pane to receive keystrokes too.
fn active_terminal_focus(state: &State) -> Option<tekstide_core::domain::TerminalId> {
    if state.focus != FocusZone::MainArea {
        return None;
    }
    let mode = state
        .app_shell
        .state()
        .active_project()
        .map(tekstide_core::project::ProjectSession::mode);
    if mode != Some(ProjectMode::TerminalImmersion) {
        return None;
    }
    active_project_terminal_sessions(state)
        .iter()
        .find(|session| session.visible_slot() == tekstide_core::domain::VisibleSlot::Primary)
        .map(|session| session.id.clone())
}

/// The real active project's registered terminal sessions -- visible
/// and hidden alike, in registration order. Factored out since both
/// [`active_terminal_focus`] and [`terminal_workspace_view`] need it,
/// and neither should cache a second copy of what `tekstide-core`
/// already owns.
fn active_project_terminal_sessions(state: &State) -> &[tekstide_core::domain::TerminalSession] {
    state
        .app_shell
        .state()
        .active_project()
        .map(tekstide_core::project::ProjectSession::terminal_sessions)
        .unwrap_or(&[])
}

fn non_modal_subscription(
    proof: input::ModalAbsent,
    focus: FocusZone,
    terminal_focus: Option<tekstide_core::domain::TerminalId>,
) -> Subscription<RoutedInput> {
    // `.filter_map`'s closure must be non-capturing (`iced` panics
    // otherwise: "cannot capture external variables"). `.with(...)`
    // threads `proof`/`focus`/`terminal_focus` in through the closure's
    // own parameter instead of a capture, which is why `ModalAbsent` and
    // `FocusZone` both derive `Hash` -- `.with` requires it to detect
    // whether the subscription's identity changed across rebuilds.
    // `TerminalId` derives `Hash` too, so a real value here changes
    // nothing about that requirement.
    keyboard::listen()
        .with((proof, focus, terminal_focus))
        .filter_map(|((proof, focus, terminal_focus), event)| {
            let press = key_press_from_event(event)?;
            let policy = KeybindingPolicy::linux_mvp();
            Some(input::route_non_modal_input(
                proof,
                &policy,
                focus,
                terminal_focus.as_ref(),
                press,
            ))
        })
}

fn modal_subscription() -> Subscription<Message> {
    keyboard::listen().filter_map(|event| match event {
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Tab),
            modifiers,
            ..
        } if modifiers.shift() => Some(Message::ModalFocusPrevious),
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Tab),
            ..
        } => Some(Message::ModalFocusNext),
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Enter),
            ..
        } => Some(Message::ModalActivate),
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            ..
        } => Some(Message::ModalDismiss),
        _ => None,
    })
}

fn key_press_from_event(event: keyboard::Event) -> Option<input::KeyPress> {
    match event {
        keyboard::Event::KeyPressed { key, modifiers, .. } => {
            Some(input::KeyPress { key, modifiers })
        }
        _ => None,
    }
}

/// Owned, `Copy` colour values rather than a borrowed `&Theme`, so this
/// helper's return type needs no lifetime capture at all -- simpler than
/// reasoning about RPIT capture rules for a borrow that would otherwise
/// need to outlive the returned closure.
fn chrome_style(
    background: iced::Color,
    foreground: iced::Color,
    border: iced::Color,
) -> impl Fn(&iced::Theme) -> container::Style {
    move |_base_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(background)),
        text_color: Some(foreground),
        border: Border {
            color: border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

fn top_bar(state: &State) -> Element<'_, Message> {
    container(text(state.window_title()).size(state.theme.font_size_heading()))
        .width(Length::Fill)
        .padding(8)
        .style(chrome_style(
            state.theme.surface_elevated(),
            state.theme.foreground(),
            state.theme.border_default(),
        ))
        .into()
}

/// The route symbol `status_bar_summary` selects on -- a compile-time
/// literal per `AppRoute` variant, not runtime-derived text, so it is
/// exactly what `CatalogArgs::trusted_symbol` is for.
fn route_symbol(route: AppRoute) -> &'static str {
    match route {
        AppRoute::ProjectBoard => "project-board",
        AppRoute::ActiveProjectWorkspace => "active-project-workspace",
    }
}

/// The status bar's text, factored out from [`status_bar`] so it is
/// directly testable without going through `iced`'s `Element` tree.
/// Response 132 Required: this count must agree with the number of rows
/// the Project Board actually renders, or the first thing a user sees
/// is chrome disagreeing with the surface directly below it. Counting
/// `state.app_shell.state().projects().len()` (open sessions only) was
/// correct in PR-015-B, when no board existed to disagree with it --
/// PR-015-D's board deliberately also lists recent-but-not-open
/// projects (RFC-005's model), so the two collections are different
/// sizes in general. Using `project_board().rows.len()` here is the
/// same computation `surface::board::view` renders from, not a second,
/// independently-arrived-at count that could drift again.
pub(crate) fn status_bar_summary(state: &State) -> String {
    let project_count = state.app_shell.project_board().rows.len();
    state.catalog.get_with_args(
        "status-bar-summary",
        &CatalogArgs::new()
            .trusted_symbol("route", route_symbol(state.app_shell.route()))
            .number("count", project_count as u32),
    )
}

fn status_bar(state: &State) -> Element<'_, Message> {
    container(text(status_bar_summary(state)).size(state.theme.font_size_status()))
        .width(Length::Fill)
        .padding(6)
        .style(chrome_style(
            state.theme.surface_elevated(),
            state.theme.foreground(),
            state.theme.border_default(),
        ))
        .into()
}

fn content_area(state: &State) -> Element<'_, Message> {
    let content: Element<'_, Message> = if state.is_measuring_typing() {
        typing_measurement_view(state)
    } else {
        match state.app_shell.route() {
            AppRoute::ProjectBoard => crate::surface::board::view(
                &state.app_shell.project_board(),
                &state.catalog,
                &state.theme,
            ),
            AppRoute::ActiveProjectWorkspace => active_project_workspace_view(state),
        }
    };

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_base_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(state.theme.background())),
            text_color: Some(state.theme.foreground()),
            ..container::Style::default()
        })
        .into()
}

/// RFC-015 PR-015-E: the sidebar/main-area scaffolding RFC-017 (terminal),
/// RFC-019 (editor/explorer), and RFC-020 (diff/review) plug real content
/// into. Both zones are catalog-driven placeholders today -- `surface.rs`'s
/// contract stays concrete methods, not a `trait Surface`, because a
/// second implementor still gives nothing to generalise from (unchanged
/// from PR-015-D's reasoning; this slice adds a second *zone*, not a
/// second surface implementation).
fn active_project_workspace_view(state: &State) -> Element<'_, Message> {
    let mode = state
        .app_shell
        .state()
        .active_project()
        .map(tekstide_core::project::ProjectSession::mode);

    row![sidebar_view(state, mode), main_area_view(state, mode)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Style for a focusable content-area zone (sidebar, main area):
/// `NFR-UX-002` forbids a colour-only status, so `focused` changes two
/// channels at once -- border colour (`Theme::border_focused`) and
/// border width -- never colour alone. Callers additionally prefix their
/// own text with the same `"> "`/`"  "` marker `shell.rs`'s modal
/// buttons already use, a third, purely textual channel.
fn zone_style(theme: Theme, focused: bool) -> impl Fn(&iced::Theme) -> container::Style {
    move |_base_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(theme.background())),
        text_color: Some(theme.foreground()),
        border: Border {
            color: if focused {
                theme.border_focused()
            } else {
                theme.border_default()
            },
            width: if focused { 2.0 } else { 1.0 },
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

/// Focus marker matching the modal's own convention
/// (`layer_composition_demo_modal`'s `button_line`) -- a textual channel
/// independent of colour or border width, so the indicator survives even
/// if a future theme makes the two border widths visually similar.
fn focus_marker(focused: bool) -> &'static str {
    if focused { "> " } else { "  " }
}

/// Factored out from [`sidebar_view`] so the focus-marker/catalog-text
/// combination is directly testable, the same shape as
/// `status_bar_summary`/`surface::board::row_lines`.
pub(crate) fn sidebar_label(state: &State) -> String {
    let focused = state.focus == FocusZone::Sidebar;
    format!(
        "{}{}",
        focus_marker(focused),
        state.catalog.get("sidebar-placeholder-title")
    )
}

/// The catalog key [`main_area_label`]/[`main_area_view`] select on for a
/// given `mode` -- `None` (no active project) should not be reachable
/// while routed to `ActiveProjectWorkspace` (core guards every
/// transition into this route on an active project existing), but the
/// fallback is Content Mode's placeholder rather than a panic, matching
/// this crate's "fail visible" convention.
fn main_area_key(mode: Option<ProjectMode>) -> &'static str {
    match mode {
        None | Some(ProjectMode::Content) => "main-area-content-mode-placeholder",
        Some(ProjectMode::TerminalImmersion) => "main-area-terminal-mode-placeholder",
    }
}

/// Factored out from [`main_area_view`] for the same reason as
/// [`sidebar_label`].
pub(crate) fn main_area_label(state: &State, mode: Option<ProjectMode>) -> String {
    let focused = state.focus == FocusZone::MainArea;
    format!(
        "{}{}",
        focus_marker(focused),
        state.catalog.get(main_area_key(mode))
    )
}

/// RFC-019 PR-019-B: `ProjectMode::Content` renders the real explorer
/// tree, the same shape `main_area_view` already uses to substitute real
/// content for `TerminalImmersion`'s placeholder. Every other mode (and
/// no active project) keeps the plain placeholder -- there is no
/// explorer to show without an active project, and `Content` is the only
/// mode this slice's scope covers (RFC-019 does not touch
/// `TerminalImmersion`).
fn sidebar_view(state: &State, mode: Option<ProjectMode>) -> Element<'_, Message> {
    let focused = state.focus == FocusZone::Sidebar;
    let content: Element<'_, Message> = match mode {
        Some(ProjectMode::Content) => {
            let workspace = state
                .app_shell
                .state()
                .active_project()
                .map(tekstide_core::project::ProjectSession::content_workspace);
            match workspace {
                Some(workspace) => crate::surface::explorer::view(
                    workspace.explorer_scan(),
                    workspace.explorer_status(),
                    state.explorer_highlight,
                    &state.catalog,
                    &state.theme,
                ),
                None => text(sidebar_label(state)).into(),
            }
        }
        _ => text(sidebar_label(state)).into(),
    };
    container(content)
        .width(Length::Fixed(220.0))
        .height(Length::Fill)
        .padding(16)
        .style(zone_style(state.theme, focused))
        .into()
}

fn main_area_view(state: &State, mode: Option<ProjectMode>) -> Element<'_, Message> {
    let focused = state.focus == FocusZone::MainArea;
    // RFC-017 PR-017-E: `launch_terminal_demo_panes` returns an empty
    // `Vec` for every normal run (`TEKSTIDE_TERMINAL_DEMO` unset), so
    // this substitutes nothing for the reviewed placeholder outside the
    // env-gated demo path -- the same shape `state.is_measuring_typing()`'s
    // substitution in `content_area` already uses. The pane renders the
    // emulator grid as data (RFC-016's exception); the session bar is
    // real chrome (RFC-016's boundary becomes live here for the first
    // time) and proves nothing about trusted-UI separation (RFC-018's
    // job).
    let content: Element<'_, Message> = match (mode, state.terminal_panes.is_empty()) {
        (Some(ProjectMode::TerminalImmersion), false) => terminal_workspace_view(state),
        // RFC-019 PR-019-C: real content, the same shape the
        // `TerminalImmersion` arm above already established for its own
        // mode -- substitute the placeholder with a real surface rather
        // than adding a third rendering path.
        (Some(ProjectMode::Content), _) => content_mode_editor_view(state),
        _ => column![text(main_area_label(state, mode)).size(state.theme.font_size_body())]
            .spacing(6)
            .into(),
    };
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(16)
        .style(zone_style(state.theme, focused))
        .into()
}

/// RFC-019 PR-019-C: `ProjectMode::Content`'s real content, the
/// analogue of [`terminal_workspace_view`] for the other mode. Always
/// renders *something* real -- `surface::editor::view` itself handles
/// "no document open yet" and "the last open attempt failed" -- falling
/// back to the plain placeholder only when there is no active project at
/// all to read a content workspace from (should not be reachable while
/// routed to `ActiveProjectWorkspace`, the same `None` case
/// `main_area_key` already documents).
fn content_mode_editor_view(state: &State) -> Element<'_, Message> {
    let workspace = state
        .app_shell
        .state()
        .active_project()
        .map(tekstide_core::project::ProjectSession::content_workspace);
    match workspace {
        Some(workspace) => crate::surface::editor::view(
            workspace.active_document(),
            workspace.status(),
            &state.catalog,
            &state.theme,
        ),
        None => text(main_area_label(state, Some(ProjectMode::Content))).into(),
    }
}

/// RFC-017 PR-017-E: the session bar (real chrome, `theme`-sourced
/// colours) above a split view of up to two *visible* panes' grids,
/// ordered `Primary` before `Secondary`. The split itself is decided
/// from the real, measured width `iced::widget::responsive` provides at
/// layout time -- not a fraction of the window -- via
/// `surface::terminal::layout_class_for`; a `Narrow` classification
/// shows only the `Primary` pane rather than rendering a clipped
/// two-column split (see `surface::terminal::layout`'s module doc for
/// why the refusal threshold is a full pane's worth of real columns).
fn terminal_workspace_view(state: &State) -> Element<'_, Message> {
    let font_size = state.theme.font_size_body();
    let theme = state.theme;
    let sessions = active_project_terminal_sessions(state);

    // Terminal launch UX handoff: "the user pressed a key and is owed a
    // visible answer" -- rendered above the session bar so it's the
    // first thing seen, and only ever present right after a refused
    // attempt (`terminal_launch_notice` is cleared at the start of every
    // new one).
    let notice: Option<Element<'_, Message>> =
        state.terminal_launch_notice.as_ref().map(|refusal| {
            text(terminal_launch_refusal_text(&state.catalog, refusal))
                .size(state.theme.font_size_body())
                .into()
        });

    // RFC-018 PR-018-B: same "owed a visible answer" shape, for a
    // refused paste rather than a refused launch. Independent of
    // `notice` above -- a launch notice and a paste notice can never
    // both be relevant to the same keypress, but nothing prevents a
    // paste notice surviving from an earlier attempt while a later,
    // unrelated launch notice also exists, so both render rather than
    // one silently winning.
    let paste_notice: Option<Element<'_, Message>> =
        state.terminal_paste_notice.as_ref().map(|refusal| {
            text(terminal_paste_refusal_text(&state.catalog, refusal))
                .size(state.theme.font_size_body())
                .into()
        });

    // RFC-022 PR-022-D: the same "owed a visible answer" shape as
    // `notice` above, for `LaunchAgentRun`'s own refusals -- rendered
    // here rather than in a separate detail view since a refused agent
    // run still lands the user in Terminal Immersion (`AppCommand::LaunchAgentRun`
    // reuses the same route `LaunchTerminal` does).
    let agent_run_notice: Option<Element<'_, Message>> =
        state.agent_run_launch_notice.as_ref().map(|refusal| {
            text(agent_run_launch_refusal_text(&state.catalog, refusal))
                .size(state.theme.font_size_body())
                .into()
        });

    let entries: Vec<crate::surface::terminal::session_bar::SessionBarEntry> = sessions
        .iter()
        .enumerate()
        .map(
            |(index, session)| crate::surface::terminal::session_bar::SessionBarEntry {
                number: (index + 1) as u32,
                slot: session.visible_slot(),
                status: session.status(),
            },
        )
        .collect();
    let bar = crate::surface::terminal::session_bar::view(theme, &state.catalog, &entries);

    let mut visible_sessions: Vec<&tekstide_core::domain::TerminalSession> = sessions
        .iter()
        .filter(|session| session.visible_slot() != tekstide_core::domain::VisibleSlot::Hidden)
        .collect();
    visible_sessions.sort_by_key(|session| match session.visible_slot() {
        tekstide_core::domain::VisibleSlot::Primary => 0,
        tekstide_core::domain::VisibleSlot::Secondary => 1,
        tekstide_core::domain::VisibleSlot::Hidden => 2,
    });
    let visible_panes: Vec<&crate::surface::terminal::TerminalPane> = visible_sessions
        .into_iter()
        .filter_map(|session| {
            state
                .terminal_panes
                .iter()
                .find(|pane| pane.terminal_id() == &session.id)
        })
        .collect();

    let panes_view: Element<'_, Message> = if visible_panes.is_empty() {
        column![].into()
    } else {
        iced::widget::responsive(move |size| {
            let shown: Vec<&crate::surface::terminal::TerminalPane> =
                match crate::surface::terminal::layout_class_for(size.width, font_size) {
                    tekstide_core::navigation::TerminalLayoutClass::Wide => visible_panes.clone(),
                    tekstide_core::navigation::TerminalLayoutClass::Narrow => {
                        visible_panes.first().copied().into_iter().collect()
                    }
                };
            row(shown
                .into_iter()
                .map(|pane| crate::surface::terminal::view(pane, font_size))
                .collect::<Vec<Element<'_, Message>>>())
            .spacing(8)
            .into()
        })
        .into()
    };

    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    if let Some(notice) = notice {
        rows.push(notice);
    }
    if let Some(paste_notice) = paste_notice {
        rows.push(paste_notice);
    }
    if let Some(agent_run_notice) = agent_run_notice {
        rows.push(agent_run_notice);
    }
    rows.push(bar);
    rows.push(panes_view);
    column(rows).spacing(8).into()
}

/// RFC-015 PR-015-F: renders the tail of `state.typing_doc` in a
/// monospace font, matching the RFC-014 spike's own typing-measurement
/// view -- deliberately not a real editor (RFC-019's job), only enough
/// rendering cost to make view-build-cost measurement meaningful against
/// a realistically-sized document. Only ever reached when
/// `state.is_measuring_typing()` is true, i.e. never during normal use.
fn typing_measurement_view(state: &State) -> Element<'_, Message> {
    let visible = tail_lines(&state.typing_doc, 50);
    container(
        column(
            visible
                .lines()
                .map(|line| {
                    text(line.to_string())
                        .size(state.theme.font_size_body())
                        .font(iced::Font::MONOSPACE)
                        .into()
                })
                .collect::<Vec<Element<'_, Message>>>(),
        )
        .spacing(2),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(16)
    .into()
}

pub(crate) fn tail_lines(doc: &str, count: usize) -> String {
    let lines: Vec<&str> = doc.lines().collect();
    let start = lines.len().saturating_sub(count);
    lines[start..].join("\n")
}

/// The trusted-chrome dialog box both modal kinds render inside --
/// factored out (RFC-018 PR-018-C) so the paste confirmation dialog and
/// the layer-composition placeholder share one styling definition
/// rather than two copies that could drift apart. `NFR-UX-002`-relevant
/// distinctions (focus, accept/reject) are the caller's job via the
/// content passed in, never colour alone here.
fn modal_dialog_box<'a>(state: &'a State, content: Element<'a, Message>) -> Element<'a, Message> {
    container(content)
        .padding(20)
        .style(move |_base_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(state.theme.surface_elevated())),
            text_color: Some(state.theme.foreground()),
            border: Border {
                color: state.theme.accent(),
                width: 2.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn layer_composition_demo_modal(state: &State, focus: ModalButton) -> Element<'_, Message> {
    let button_line = |target: ModalButton, label_key: &str| {
        let marker = if focus == target { "> " } else { "  " };
        text(format!("{marker}{}", state.catalog.get(label_key))).size(state.theme.font_size_body())
    };

    modal_dialog_box(
        state,
        column![
            text(state.catalog.get("layer-demo-modal-title")).size(state.theme.font_size_heading()),
            text(state.catalog.get("layer-demo-modal-body")).size(state.theme.font_size_body()),
            button_line(ModalButton::Acknowledge, "layer-demo-modal-acknowledge"),
            button_line(ModalButton::Dismiss, "layer-demo-modal-dismiss"),
            text(state.catalog.get("layer-demo-modal-dismiss-hint"))
                .size(state.theme.font_size_status()),
        ]
        .spacing(10)
        .into(),
    )
}

/// RFC-018 PR-018-C: the real paste confirmation dialog. The preview is
/// untrusted text in trusted chrome -- RFC-016's grid exception does
/// not reach it -- so it goes through `text_safety::quote_untrusted`
/// exactly like `surface::board::row_lines`'s project-name rendering,
/// truncated to [`PASTE_PREVIEW_CHAR_LIMIT`] *characters before*
/// escaping (never after: slicing an already-escaped, isolate-wrapped
/// string could separate the isolate marks from the content they wrap,
/// which `text_safety`'s own `DisplayText` doc comment warns against).
/// The escaped preview text and whether it was truncated -- factored
/// out from [`paste_confirmation_modal_view`] so both are directly
/// testable without going through `iced`'s `Element` tree, the same
/// shape `surface::board::row_lines` and `shell::status_bar_summary`
/// already use. Truncates the **raw** content to
/// [`PASTE_PREVIEW_CHAR_LIMIT`] characters before escaping, never
/// after: slicing an already-escaped, isolate-wrapped `DisplayText`
/// could separate the isolate marks from the content they wrap, which
/// that type's own doc comment warns against.
pub(crate) fn paste_preview(content: &str) -> (String, bool) {
    let preview_source: String = content.chars().take(PASTE_PREVIEW_CHAR_LIMIT).collect();
    let truncated = preview_source.chars().count() < content.chars().count();
    let preview = tekstide_core::text_safety::quote_untrusted(&preview_source);
    (preview.as_str().to_string(), truncated)
}

fn paste_confirmation_modal_view<'a>(
    state: &'a State,
    modal: &'a PasteConfirmationModal,
) -> Element<'a, Message> {
    let button_line = |target: PasteConfirmButton, label_key: &str| {
        let marker = if modal.focus == target { "> " } else { "  " };
        text(format!("{marker}{}", state.catalog.get(label_key))).size(state.theme.font_size_body())
    };

    let (preview, truncated) = paste_preview(&modal.content);

    let mut lines: Vec<Element<'_, Message>> = vec![
        text(state.catalog.get("paste-confirm-dialog-title"))
            .size(state.theme.font_size_heading())
            .into(),
        text(state.catalog.get_with_args(
            "paste-confirm-dialog-body",
            &CatalogArgs::new().number("line_count", modal.line_count as u32),
        ))
        .size(state.theme.font_size_body())
        .into(),
        text(preview).size(state.theme.font_size_body()).into(),
    ];
    if truncated {
        lines.push(
            text(state.catalog.get("paste-confirm-dialog-preview-truncated"))
                .size(state.theme.font_size_status())
                .into(),
        );
    }
    lines.push(button_line(PasteConfirmButton::Accept, "paste-confirm-dialog-accept").into());
    lines.push(button_line(PasteConfirmButton::Reject, "paste-confirm-dialog-reject").into());
    lines.push(
        text(state.catalog.get("paste-confirm-dialog-hint"))
            .size(state.theme.font_size_status())
            .into(),
    );

    modal_dialog_box(state, column(lines).spacing(10).into())
}

/// RFC-019 PR-019-D: the real external-change conflict dialog. The path
/// is attacker-influenced chrome, escaped the same way `chrome_line`'s
/// header path is -- this dialog names the same file that header
/// already showed escaped, so it must not reintroduce the raw form here.
/// RFC-019 PR-019-D: `relative_path` is the same attacker-influenced
/// class as `chrome_line`'s own path -- escaped the same way, before it
/// reaches the catalog. Factored out so the escaping property is
/// directly testable without going through `iced`'s `Element` tree, the
/// same shape `chrome_line`/`paste_preview` already use.
pub(crate) fn external_change_dialog_body(
    catalog: &Catalog,
    relative_path: &std::path::Path,
    had_local_edits: bool,
) -> String {
    let path = tekstide_core::text_safety::quote_untrusted(&relative_path.display().to_string());
    let reason = if had_local_edits {
        "conflict"
    } else {
        "external-changed"
    };
    catalog.get_with_args(
        "external-change-dialog-body",
        &CatalogArgs::new()
            .untrusted("path", &path)
            .trusted_symbol("reason", reason),
    )
}

/// RFC-022 PR-022-E: a compile-time literal symbol for `RiskLevel`, the
/// same `trusted_symbol` division of labour every other symbol-driven
/// Fluent lookup in this file uses -- the words live in `en.ftl`'s
/// `approval-dialog-risk` select expression, not here. `RiskLevel` is
/// Tekstide's own classification output (`approval::risk::classify`),
/// never adapter-supplied text, so this needs no escaping -- only
/// `display_command`/`cwd` do (response 221).
fn risk_level_symbol(level: tekstide_core::domain::RiskLevel) -> &'static str {
    use tekstide_core::domain::RiskLevel;
    match level {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
        RiskLevel::Destructive => "destructive",
    }
}

/// RFC-022 PR-022-E: the dialog body's escaping, per response 221's
/// corrected split.
///
/// **`display_command` is isolation-wrapped, not re-escaped.**
/// `ApprovalRequest.display_command` is already escaped by the model
/// (`approval::coordinator::display_argv`/`display_entry`, RFC-021
/// response 114/115's own ten-probe suite) -- `quote_untrusted` here
/// only adds the bidi isolation wrap a value needs to embed safely in
/// trusted chrome. Running `escape_untrusted_chars` again on an
/// already-escaped string is a proven no-op: it acts only on control and
/// format characters, and none survive in `display_command` (they were
/// already replaced with `<U+XXXX>` marker text, itself plain ASCII).
/// This is cited, not re-proven -- re-testing it here would test
/// RFC-021, not this slice.
///
/// **`cwd` is escaped here, for the first time.** It arrives on
/// `ApprovalRequest` raw, straight from the adapter's proposal
/// (`domain/approval.rs`) -- response 221 identified it as the actual
/// live attack surface `what-the-dialog-must-not-lie-about.md` §1
/// originally (and wrongly) named `argv` for: a user reads the command
/// carefully but skims the directory to confirm context, which is
/// exactly what a rendering attack targets.
///
/// `environment_summary` is checked and found, not rendered: it is
/// `ApprovalRequest`'s third field response 221 asked about, and it has
/// no writer anywhere in this codebase (`ApprovalRequest::pending` sets
/// it to `None` and nothing since RFC-021 has ever set it to `Some`) --
/// nothing adapter-derived is in it today, so there is nothing to
/// escape or to render.
pub(crate) fn approval_dialog_body(
    catalog: &Catalog,
    request: &tekstide_core::domain::ApprovalRequest,
) -> String {
    let command = tekstide_core::text_safety::quote_untrusted(&request.display_command);
    let cwd = tekstide_core::text_safety::quote_untrusted(&request.cwd.display().to_string());
    catalog.get_with_args(
        "approval-dialog-body",
        &CatalogArgs::new()
            .untrusted("command", &command)
            .untrusted("cwd", &cwd)
            .trusted_symbol("risk", risk_level_symbol(request.risk_level)),
    )
}

/// Not yet called from `view()` -- see [`ApprovalDialog`]'s own doc
/// comment for why the trigger wiring waits on response 220.
/// `#[allow(dead_code)]` rather than a throwaway caller: this function,
/// [`approval_dialog_body`], and [`risk_level_symbol`] are exercised
/// directly by `shell::tests` today, which proves them correct ahead of
/// the wiring that will make them reachable from `main` for real.
#[allow(dead_code)]
fn approval_dialog_view<'a>(state: &'a State, dialog: &'a ApprovalDialog) -> Element<'a, Message> {
    let button_line = |target: ApprovalDialogButton, label_key: &str| {
        let marker = if dialog.focus == target { "> " } else { "  " };
        text(format!("{marker}{}", state.catalog.get(label_key))).size(state.theme.font_size_body())
    };

    let lines: Vec<Element<'_, Message>> = vec![
        text(state.catalog.get("approval-dialog-title"))
            .size(state.theme.font_size_heading())
            .into(),
        text(approval_dialog_body(&state.catalog, &dialog.request))
            .size(state.theme.font_size_body())
            .into(),
        // what-the-dialog-must-not-lie-about.md §2: "the highest-consequence
        // sentence in this RFC" -- rendered as its own line, not folded
        // into the body text above, so it cannot be missed by a reader
        // skimming for the command and cwd alone.
        text(state.catalog.get("approval-dialog-cooperative-notice"))
            .size(state.theme.font_size_body())
            .into(),
        button_line(ApprovalDialogButton::ApproveOnce, "approval-dialog-approve").into(),
        button_line(ApprovalDialogButton::Reject, "approval-dialog-reject").into(),
        text(state.catalog.get("approval-dialog-hint"))
            .size(state.theme.font_size_status())
            .into(),
    ];

    modal_dialog_box(state, column(lines).spacing(10).into())
}

fn external_change_modal_view<'a>(
    state: &'a State,
    modal: &'a ExternalChangeModal,
) -> Element<'a, Message> {
    let button_line = |target: ExternalChangeButton, label_key: &str| {
        let marker = if modal.focus == target { "> " } else { "  " };
        text(format!("{marker}{}", state.catalog.get(label_key))).size(state.theme.font_size_body())
    };

    let lines: Vec<Element<'_, Message>> = vec![
        text(state.catalog.get("external-change-dialog-title"))
            .size(state.theme.font_size_heading())
            .into(),
        text(external_change_dialog_body(
            &state.catalog,
            &modal.relative_path,
            modal.had_local_edits,
        ))
        .size(state.theme.font_size_body())
        .into(),
        button_line(
            ExternalChangeButton::Reload,
            "external-change-dialog-reload",
        )
        .into(),
        button_line(
            ExternalChangeButton::Dismiss,
            "external-change-dialog-dismiss",
        )
        .into(),
        text(state.catalog.get("external-change-dialog-hint"))
            .size(state.theme.font_size_status())
            .into(),
    ];

    modal_dialog_box(state, column(lines).spacing(10).into())
}

#[cfg(test)]
mod tests;
