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
use iced::widget::{button, center, column, container, opaque, row, scrollable, stack, text};
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

/// RFC-032, `what-the-trust-dialog-must-say.md` §2: "the safe thing here
/// is not granting, and the asymmetry is larger [than the paste
/// dialog's]." Same two-item cycle shape as `PasteConfirmButton` and the
/// same reason for a distinct type: "Grant"/"Cancel" do not describe
/// what another dialog's buttons decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrustGrantButton {
    Grant,
    Cancel,
}

impl TrustGrantButton {
    const ORDER: [TrustGrantButton; 2] = [TrustGrantButton::Grant, TrustGrantButton::Cancel];

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

/// RFC-032: the project a trust-grant dialog is deciding about.
/// `root_path`/`canonical_root_path` are captured once, at open time,
/// from the same `ProjectSession` fields the board and every other path
/// display already reads -- not re-read live while the dialog is open,
/// so what the user reads is exactly what `AuditCoordinator::grant_project_trust`
/// is asked to authorise, the same "captured, not re-read" shape
/// `ExternalChangeModal.relative_path` already uses.
#[derive(Debug)]
pub(crate) struct TrustGrantModal {
    project_id: tekstide_core::project::ProjectId,
    root_path: std::path::PathBuf,
    canonical_root_path: std::path::PathBuf,
    focus: TrustGrantButton,
}

/// RFC-033 PR-033-C: purge confirmation -- `what-purge-must-remove.md`'s
/// required reading names the two things the confirmation must state,
/// "what disappears and that it cannot be undone." Same two-item cycle
/// shape as `TrustGrantButton`, and the same reason for a distinct type:
/// "Purge"/"Cancel" do not describe what another dialog's buttons
/// decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranscriptPurgeButton {
    Purge,
    Cancel,
}

impl TranscriptPurgeButton {
    const ORDER: [TranscriptPurgeButton; 2] =
        [TranscriptPurgeButton::Purge, TranscriptPurgeButton::Cancel];

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

/// RFC-033 PR-033-C: the project a purge-confirmation dialog is deciding
/// about. `transcript_count`/`retained_bytes` are captured once, at open
/// time, from `ProjectSession::transcript_local_data_summary` -- the
/// same "captured, not re-read" shape `TrustGrantModal` already uses, so
/// what the confirmation states is exactly what was true when the user
/// chose to open it, not a value that could have drifted (e.g. from
/// another agent run writing a transcript) by the time they decide.
#[derive(Debug)]
pub(crate) struct TranscriptPurgeModal {
    project_id: tekstide_core::project::ProjectId,
    transcript_count: u64,
    retained_bytes: u64,
    focus: TranscriptPurgeButton,
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

/// RFC-022 PR-022-E ("the arrival model"): how often
/// `Message::ApprovalPollTick` fires. Disclosed trade-off in
/// `Message::ApprovalPollTick`'s own doc comment -- a plain interval,
/// not the wake-`eventfd` machinery `terminal_panes` uses, since an
/// approval proposal is far rarer than terminal output.
const APPROVAL_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

/// RFC-022 PR-022-E ("the arrival model"), response 227: how long a
/// freshly promoted approval dialog ignores modal input for -- long
/// enough that a keystroke already in flight (typing mid-word, or
/// dismissing a different modal that just closed) cannot reach this new
/// one, short enough that a user who genuinely wants to act on it right
/// away is not made to wait.
const APPROVAL_DIALOG_INPUT_IGNORE_WINDOW: std::time::Duration =
    std::time::Duration::from_millis(400);

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

/// RFC-022 PR-022-E ("the arrival model"): a live approval channel this
/// GUI is serving, one per `Managed` agent run with a bound endpoint --
/// registered by `register_approval_channel` once
/// `launch_agent_run_with_runtime` returns one (response 227's found
/// defect: this used to be silently dropped one layer down). Holds the
/// `mpsc::Receiver` side of `ApprovalChannelEndpoint::serve_concurrently`
/// (polled by `ApprovalPollTick`) and the `ServeShutdown` handle needed
/// to tear the accept loop down cleanly.
///
/// `verified_cwd`/`project_root`/`state_root` are captured once, at
/// launch time, and reused for every proposal this run's adapter later
/// sends -- `receive_proposal` needs all three on every call, but
/// `VerifiedCwd` has no public constructor accepting an arbitrary path
/// (the only way to obtain one is `AgentRunLaunchValidator::validate`,
/// which runs once at launch, not once per proposal).
struct ApprovalChannelServing {
    project_id: tekstide_core::project::ProjectId,
    agent_run_id: tekstide_core::domain::AgentRunId,
    verified_cwd: tekstide_core::agent::VerifiedCwd,
    project_root: std::path::PathBuf,
    state_root: std::path::PathBuf,
    receiver: std::sync::mpsc::Receiver<
        Result<
            tekstide_core::approval::AcceptedProposal,
            tekstide_core::approval::ApprovalChannelError,
        >,
    >,
    // `ApprovalChannelEndpoint::serve_concurrently` deliberately drops the
    // strong `Arc` it is given (see its own doc comment: the accept loop
    // holds only a `Weak`) and relies on the caller retaining a clone for
    // as long as serving should continue. Without this field, the
    // endpoint's strong count hit zero the instant `register_approval_channel`
    // returned -- running `ApprovalChannelEndpoint`'s `Drop` immediately,
    // which both closed the listener and removed the real socket special
    // file, before the accept-loop thread ever got a chance to call
    // `accept()`. This is what a real adapter's `connect()` was racing and
    // losing every time (`ENOENT`), not a socket-path mismatch. Never
    // explicitly read otherwise -- held purely to keep the endpoint (and
    // therefore the bound socket) alive for this serving's lifetime.
    #[allow(dead_code)]
    endpoint: std::sync::Arc<tekstide_core::approval::ApprovalChannelEndpoint>,
    // RFC-022 PR-022-E: never explicitly read -- held only so its own
    // `Drop` runs when this `ApprovalChannelServing` is dropped
    // (`poll_approval_channels` simply not re-inserting a disconnected
    // serving into `still_open`), the "simply dropping this value" half
    // of `ServeShutdown`'s own documented contract, not an oversight.
    #[allow(dead_code)]
    shutdown: tekstide_core::approval::ServeShutdown,
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
/// **Wired into `ModalContent` as of response 227's "the arrival
/// model."** `proposal_id` bridges back to `ApprovalCoordinator`'s own
/// map (keyed by the wire `ProposalId`, not the domain `ApprovalId`
/// `request.id` carries -- `ApprovalRequest` has no reference back to
/// the wire id, since it is the audit-facing type RFC-021 already
/// defined and this slice does not widen it for GUI-only convenience;
/// `State.approval_proposal_ids` records the mapping instead).
/// `ignore_input_until` is the post-promotion input-ignore window
/// (response 227, and `what-the-dialog-must-not-lie-about.md`'s own
/// "focus defaults to Reject" pairing) -- a stray keystroke already in
/// flight when this dialog is promoted (mid-edit, or dismissing a
/// *different* modal that just closed) must not immediately activate or
/// dismiss it.
#[derive(Debug)]
pub(crate) struct ApprovalDialog {
    request: tekstide_core::domain::ApprovalRequest,
    proposal_id: tekstide_core::approval::ProposalId,
    focus: ApprovalDialogButton,
    ignore_input_until: Option<std::time::Instant>,
}

#[cfg(test)]
impl ApprovalDialog {
    /// Test-only constructor: every real dialog is built by
    /// `evaluate_promotion`, which always arms the input-ignore window
    /// -- this lets a test construct one with `ignore_input_until: None`
    /// to exercise `decide_approval`'s own real wire round trip in
    /// isolation from that window, which is proven separately
    /// (`modal_input_is_ignored_within_the_post_promotion_window`).
    pub(crate) fn for_test(
        request: tekstide_core::domain::ApprovalRequest,
        proposal_id: tekstide_core::approval::ProposalId,
        focus: ApprovalDialogButton,
    ) -> Self {
        Self {
            request,
            proposal_id,
            focus,
            ignore_input_until: None,
        }
    }
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
    /// RFC-022 PR-022-E ("the arrival model", response 220/227): a
    /// promoted `High`/`Destructive` approval request. Unlike every
    /// other modal here, this one is never opened directly in response
    /// to the keystroke that produced it -- `evaluate_promotion` sets
    /// it, from an adapter's own proposal arriving, possibly while the
    /// user is mid-edit elsewhere. Escape leaves the request pending
    /// (no decision recorded), matching `ApprovalCoordinator::decide`'s
    /// own "nobody decided" semantics -- unlike Reload/Accept above,
    /// there is no "closes without consequence" reading here: Reject is
    /// a real decision, reachable only by moving focus and activating,
    /// the same "one stray keystroke can only reject" property
    /// `focus: Reject` by default already gives Escape's usual meaning.
    // Boxed per clippy::large_enum_variant -- `ApprovalDialog` holds a
    // full `ApprovalRequest`, the same reason `ReceiveOutcome::Created`
    // boxes its own copy.
    Approval(Box<ApprovalDialog>),
    /// RFC-032: "the most consequential single click in this
    /// application" (`what-the-trust-dialog-must-say.md`). Always
    /// opened manually, from a control on the `TrustSettings` surface
    /// (`Message::OpenTrustGrantDialog`) -- never automatically, unlike
    /// `Approval` above. Only `Grant` is a real decision; `Cancel`, like
    /// `ModalDismiss`/Escape, closes without granting anything -- the
    /// paste dialog's shape, not the approval dialog's (both of that
    /// one's buttons are real decisions).
    TrustGrant(TrustGrantModal),
    /// RFC-033 PR-033-C: opened manually from the `TrustSettings`
    /// surface's own purge control (`Message::OpenTranscriptPurgeDialog`),
    /// the same manual-only shape `TrustGrant` above uses. Only `Purge`
    /// is a real decision; `Cancel`, like `ModalDismiss`/Escape, closes
    /// without deleting anything -- the paste dialog's shape, not the
    /// approval dialog's.
    TranscriptPurge(TranscriptPurgeModal),
    /// RFC-038 PR-038-C: the keyboard reference, moved off the Project
    /// Board (RFC-039's second principle: reference material does not
    /// live on a working surface) into a modal reachable from anywhere,
    /// `Ctrl+Alt+K`. Unlike every other variant here, there is nothing
    /// to focus or activate -- no fields, a unit variant -- so
    /// `ModalFocusNext`/`ModalFocusPrevious`/`ModalActivate` are no-ops
    /// against it; only `ModalDismiss`/Escape does anything, and that
    /// handler is already generic across every `ModalContent` variant.
    Help,
    /// RFC-038 PR-038-G: the folder browser -- overturns RFC-038's own
    /// D1 (a typed path is not an acceptable *primary* way to choose a
    /// folder). Unlike `Help`, has real state to navigate and a real
    /// decision (`FolderBrowserModal`'s own doc).
    FolderBrowser(FolderBrowserModal),
}

/// RFC-038 PR-038-G: `scan`/`highlight` are always a **valid** scan --
/// a failed navigation attempt (`navigate_error`) or a failed commit
/// (`open_error`) never replaces them, the same "keep the last good
/// state, render the failure alongside it" shape `PathFieldError`
/// already established for the path field (`what-a-path-field-must-
/// not-trust.md` applies here too: whatever directory is ultimately
/// chosen is untrusted and re-validated in full by `add_project_from_path`,
/// exactly as a typed path is).
#[derive(Debug, Clone)]
pub(crate) struct FolderBrowserModal {
    scan: tekstide_core::project::root::DirectoryBrowseScan,
    highlight: usize,
    /// Set when the last `Enter` (navigate into a row) failed --
    /// cleared on the next navigation attempt, successful or not, so it
    /// never describes a stale attempt.
    navigate_failed: bool,
    /// Set when the last `Space` (commit `scan.current_dir` as the new
    /// project) failed. Reuses [`PathFieldError`] -- the same
    /// `add_project_from_path` call, the same failure shapes, the same
    /// "never a raw path in the error type itself" discipline.
    open_error: Option<PathFieldError>,
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
    /// Response 234: the `ApprovalHistory` surface's own keyboard cursor
    /// -- the direct analogue of `explorer_highlight` for a second,
    /// independent list in the same `MainArea` zone. A separate field
    /// rather than reusing `explorer_highlight`: the two lists render in
    /// different zones simultaneously reachable in the same Content
    /// mode session (sidebar explorer vs. main-area history), and their
    /// row counts are unrelated, so sharing one index would let
    /// switching surfaces leave a stale, out-of-range highlight behind.
    /// Not reset when the underlying list's *contents* change between
    /// key presses (a proposal arriving or expiring while the user is
    /// browsing) -- the same limitation `explorer_highlight` already
    /// has relative to a background rescan, clamped defensively on
    /// every read rather than tracked by id.
    approval_history_highlight: usize,
    /// RFC-022 PR-022-E ("the arrival model"): the live, security-critical
    /// coordinator. Lives here, not in `tekstide-core::project::ProjectSession`
    /// -- see `ApprovalDialog`'s own doc comment and response 224:
    /// `AcceptedProposal` holds a real `UnixStream`, so anything holding
    /// one can never be `Clone`/`PartialEq`, unlike `ProjectSession`.
    /// The same reason `TerminalPane` (also holding live OS resources)
    /// lives in `state.terminal_panes` rather than `tekstide-core`. One
    /// coordinator, not one per project: its internal map already keys
    /// by `AgentRunId` (globally unique), the same flat-collection shape
    /// `terminal_panes` already uses across every open project.
    approval_coordinator: tekstide_core::approval::ApprovalCoordinator,
    /// One entry per `AgentRun` with a live approval channel being
    /// served -- populated when `launch_agent_run_with_runtime` returns
    /// a bound endpoint for a `Managed` profile (response 227's found
    /// defect, now fixed one layer down). Polled by `ApprovalPollTick`.
    approval_channels: Vec<ApprovalChannelServing>,
    /// `ApprovalRequest` (the domain/audit-facing type, mirrored into
    /// `ProjectSession.approval_requests`) carries no reference back to
    /// the wire `ProposalId` `ApprovalCoordinator`'s own map is keyed
    /// by -- RFC-021 already defined that type's shape, and this slice
    /// does not widen it for GUI-only convenience. This is the bridge:
    /// populated on every `ReceiveOutcome::Created`, read whenever a
    /// rendered `ApprovalRequest.id` needs to become a real
    /// `decide`/`is_still_answerable` call.
    ///
    /// **Response 228 Required 2**: pruned on both routes an entry can
    /// stop existing elsewhere -- `receive_approval_proposal` removes the
    /// evicted id `ProjectSession::add_approval_request` reports when
    /// `approval_history_limit` eviction fires, and `decide_approval`
    /// removes its own entry the moment a `Decided` outcome is real
    /// (nothing ever looks up a decided request's `ProposalId` again).
    /// Expiry deliberately does not prune: an expired request is neither
    /// evicted nor decided, stays `Pending`, and remains retained until
    /// one of those two things eventually happens to it -- pruning here
    /// early would desync this map from a request that can still be
    /// looked up (`sweep_expired_approvals` itself, before it marks an
    /// entry expired) until eviction actually removes it.
    ///
    /// **Response 229: the bound this map now has is real but
    /// indirect, worth stating rather than leaving for a reader to
    /// derive.** This map is not bounded by anything of its own --
    /// eviction pruning is what bounds it, eviction itself is bounded by
    /// `approval_history_limit` (`ProjectResourceLimits`), and every
    /// entry here corresponds to exactly one `ApprovalRequest`
    /// `ProjectSession` retains. So this map's size is bounded
    /// transitively, through that limit, not by any check of its own.
    approval_proposal_ids: std::collections::HashMap<
        tekstide_core::domain::ApprovalId,
        tekstide_core::approval::ProposalId,
    >,
    /// Terminal resize handoff: the window's real logical size, as of
    /// the most recent `iced::window::resize_events()` firing --
    /// `None` until the first one arrives (every tracked pane keeps
    /// [`crate::surface::terminal::ROWS`]/[`crate::surface::terminal::COLS`]
    /// until then). `Message::WindowResized`'s handler is the only writer;
    /// [`terminal_workspace_content_size`] is the only reader, and it is
    /// the one place this becomes a real grid size -- see that
    /// function's own doc for why a computed size, not a second live
    /// measurement, is what response 242 chose.
    window_size: Option<iced::Size>,
    /// change-detection-wiring handoff, Slice C: the filesystem baseline
    /// captured at agent-run launch (`attempt_agent_run_launch_with_profile`),
    /// held here until that run's terminal exits and detection can run
    /// against it (`record_terminal_exit`). Shell-local, transient
    /// coordination state -- the same per-`AgentRunId` map shape
    /// `approval_channels` uses, for the same reason: a captured
    /// baseline is not part of the durable project model, only a fact
    /// this GUI session needs to carry between two points in time.
    /// Removed the moment detection is attempted for a run (whether or
    /// not a real `ChangeSet` resulted) -- a baseline only makes sense
    /// against the one run it was captured for, and is never reused.
    agent_run_change_baselines: std::collections::HashMap<
        tekstide_core::domain::AgentRunId,
        tekstide_core::project::ReviewBaseline,
    >,
    /// change-detection-wiring handoff, Slice D (D2): the outcome of the
    /// most recent detection attempt for each run, recorded whether or
    /// not it produced a `ChangeSet`. **The reason this map exists**:
    /// `project.change_sets()` alone cannot tell a caller "this run's
    /// changes are unknown, the scan was truncated" apart from "this run
    /// genuinely touched nothing" -- both look identical, zero entries.
    /// `Some(ChangeDetectionStatus::Complete)` with no change set for a
    /// run means the second, honestly; anything else
    /// (`Partial`/`Failed`/`Unavailable`/`Unsupported`) means the first,
    /// and must never be presented as though it were the second. No
    /// renderer reads this yet (RFC-020 does not exist) -- this is the
    /// data that must already be honest before one does, not a rendered
    /// state itself.
    agent_run_change_detection_status: std::collections::HashMap<
        tekstide_core::domain::AgentRunId,
        tekstide_core::domain::ChangeDetectionStatus,
    >,
    /// RFC-038 PR-038-A: the Project Board empty state's path field.
    /// Shell-local, transient UI state -- the same shape `typing_doc`
    /// already is -- not part of `tekstide-core`'s model, since it holds
    /// nothing until `Enter` turns it into a real
    /// `add_project_from_path` call. Raw, unescaped text: per
    /// `text_safety`'s own rule, escaping happens at render
    /// (`board::empty_state_view`), never before storing. Bounded to
    /// [`MAX_PATH_FIELD_CHARS`] as it grows, from either source
    /// (`push_to_path_field`), so neither typing nor a pasted clipboard
    /// value can grow this without bound.
    path_field: String,
    /// The most recent `add_project_from_path` failure reached through
    /// the field, if any -- same shell-local, transient shape as
    /// `terminal_launch_notice`. Cleared at the start of every new
    /// submit attempt. Holds no path of its own: `path_field` above is
    /// the one source `path_field_error_text` reads the (still-live,
    /// still-editable) typed value from when rendering this.
    path_field_notice: Option<PathFieldError>,
    /// RFC-038 PR-038-B: set by `Ctrl+Alt+O`
    /// (`NavigationAction::OpenProjectEntryField`), cleared on a
    /// successful open or `Escape`. The empty board's field is always
    /// showing on its own (`empty_state.is_some()`); this is what makes
    /// the field available on the *populated* board too, for the
    /// second-project case PR-038-A's field does not serve. `board::view`
    /// and `handle_project_board_path_field_key` both read this, the
    /// same single-signal shape `empty_state.is_some()` already is for
    /// the empty-board case, so the two cannot independently drift about
    /// when the field is showing.
    path_field_requested: bool,
    /// RFC-038 PR-038-D: the Project Board's own keyboard cursor over
    /// `project_board().rows` -- the direct analogue of
    /// `approval_history_highlight` for a third, independent list.
    /// Moves over every row (active sessions included); only `Enter` on
    /// a `Recent*`-kind row acts on it -- an `ActiveSession` row is
    /// already open, so `Enter` on one is still inert here
    /// (`handle_project_board_row_key`'s own doc); switching to it is
    /// now RFC-039 PR-039-B's own tab strip, a different control
    /// entirely, not this one. Clamped defensively on every read, the
    /// same "not tracked by id" limitation `explorer_highlight`/
    /// `approval_history_highlight` already carry relative to the list
    /// changing between key presses.
    project_board_row_highlight: usize,
    /// RFC-039 PR-039-B: the tab strip's own keyboard cursor -- the
    /// fourth of this shape (`explorer_highlight`, `approval_history_
    /// highlight`, `project_board_row_highlight` are the other three).
    /// Index 0 is the permanent "Projects" home tab; indices `1..=N`
    /// are the `N` open projects, in `AppState::projects()`'s own
    /// order. Meaningful only while `focus == FocusZone::TabStrip`
    /// (`handle_tab_strip_key`'s own guard); left as-is otherwise, the
    /// same "stale but re-clamped on next use" shape every sibling
    /// highlight field already has.
    tab_strip_highlight: usize,
}

impl State {
    pub fn new(mut app_shell: ApplicationShell, catalog: Catalog) -> Self {
        // RFC-032 PR-032-C, response 245: the audit store, not the
        // user-writable recent-projects cache, is authoritative for
        // whether a boot-time (CLI-argument) project is really trusted.
        // Must run before anything reads `trust_state()` for a security
        // decision -- this is the one place every boot-time project is
        // in place before the event loop starts.
        //
        // **Corrected in PR-038-E (response 302's own finding)**: the
        // original comment here said "currently nothing does yet"
        // about RFC-032's dialog and restricted-mode gates, true when
        // written and false since those shipped. Worse, this call is
        // no longer the *only* place trust gets verified: RFC-038
        // PR-038-D added three more (`reopen_recent_project`,
        // `attempt_open_project_from_path_field`,
        // `choose_current_browsed_directory`) once it became possible
        // to restore a project's trust from the recent-projects cache
        // *after* boot, mid-session -- each calls `verify_restored_trust`
        // itself, right after its own `add_project_from_path` succeeds.
        // This call covers only the CLI-argument projects already live
        // when `State::new` runs.
        verify_restored_trust(&mut app_shell);
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
            approval_history_highlight: 0,
            approval_coordinator: tekstide_core::approval::ApprovalCoordinator::new(),
            approval_channels: Vec::new(),
            approval_proposal_ids: std::collections::HashMap::new(),
            window_size: None,
            agent_run_change_baselines: std::collections::HashMap::new(),
            agent_run_change_detection_status: std::collections::HashMap::new(),
            path_field: String::new(),
            path_field_notice: None,
            path_field_requested: false,
            project_board_row_highlight: 0,
            tab_strip_highlight: 0,
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
    /// RFC-022 PR-022-E ("the arrival model"): periodic check of every
    /// `state.approval_channels` entry for a newly-arrived proposal, and
    /// every retained-but-`Pending` `ApprovalRequest` for whether its
    /// connection has since closed. A plain interval tick
    /// (`iced::time::every`), not the wake-`eventfd` machinery
    /// `terminal_panes` uses -- disclosed trade-off, not an oversight:
    /// an approval proposal is far rarer than terminal output (the
    /// adapter is already blocked waiting up to 30 seconds once one
    /// arrives), so a few hundred milliseconds of polling latency to
    /// notice it is a small fraction of that budget, not a
    /// user-perceptible delay for something the user was not expecting
    /// at that exact instant anyway.
    ApprovalPollTick,
    /// Response 233: fired by a live entry's control on the
    /// `ApprovalHistory` surface. Reuses the exact same `ApprovalDialog`
    /// construction `evaluate_promotion` uses (see
    /// `open_approval_history_entry`) rather than a second, inline
    /// decision UI -- "one decision, one command, read individually"
    /// must hold regardless of how the dialog was reached.
    OpenApprovalHistoryEntry(tekstide_core::domain::ApprovalId),
    /// Terminal resize handoff (response 242): the window's logical size
    /// changed. `iced::window::resize_events()` is genuinely event-driven
    /// (filters `Event::Window(Event::Resized(_))`), not a per-frame
    /// subscription -- fires on discrete geometry changes only, which is
    /// also why applying every one of these directly, with no further
    /// coalescing, does not produce a syscall storm: many of them during
    /// a drag collapse to the same computed grid size until a real
    /// glyph/line boundary is crossed (`apply_terminal_geometry`'s own
    /// no-op-when-unchanged check, backed by `TerminalPane::resize`'s).
    ///
    /// Response 243's required fix: also the message a real, queried
    /// window size arrives as (see `Message::WindowOpened`'s handler) --
    /// `apply_terminal_geometry` does not care whether the size it is
    /// given came from a drag or from the one-time query that primes
    /// `state.window_size` after boot; either way, it is a real size to
    /// apply to every tracked pane.
    WindowResized(iced::Size),
    /// Response 243's required fix: `iced::window::open_events()` fired
    /// -- a window (in practice, this application's one and only window)
    /// now exists. `boot()` cannot know the real window size (no window
    /// is open yet when it runs -- see its own doc comment), so
    /// `state.window_size` starts `None` and every pane launched before
    /// the first real `WindowResized` event used to stay at the
    /// launch-time `ROWS`/`COLS` default until the user happened to drag
    /// the window edge. This message's handler asks `iced` for the real
    /// size directly (`iced::window::size`) and feeds it through the
    /// same `Message::WindowResized` path a live resize uses -- one
    /// formula, one application point, whether the size came from
    /// opening or from resizing.
    WindowOpened(iced::window::Id),
    /// RFC-032: fired by the `TrustSettings` surface's own "Grant"
    /// control -- opens the real confirmation dialog. No I/O here; the
    /// real grant only happens on `ModalActivate` with focus on `Grant`
    /// (see `TrustGrantModal`'s own doc for why this dialog is not
    /// folded into `AppCommand::OpenActiveProjectSurface` the way
    /// opening the surface itself is).
    OpenTrustGrantDialog,
    /// RFC-032: fired by the `TrustSettings` surface's own "Revoke"
    /// control. Unlike granting, revocation has no confirmation dialog
    /// of its own -- `what-the-trust-dialog-must-say.md` §5 requires
    /// revoking to be *reachable*, not gated the way the RFC's own
    /// larger, harder-to-undo grant is; this is the direct, one-action
    /// path that makes it so.
    RevokeWorkspaceTrust,
    /// RFC-033 PR-033-B: fired by the `TrustSettings` surface's own
    /// capture-opt-out control. Unlike the trust action above, this
    /// control is always present regardless of trust state (declining
    /// capture is independent of trust), so it cannot reuse Enter --
    /// see `handle_trust_settings_key`'s own doc comment for why Space
    /// is the key. No confirmation dialog: declining is the safe
    /// direction, the same reasoning `RevokeWorkspaceTrust` already
    /// applies to revocation.
    ToggleTranscriptCaptureDeclined,
    /// RFC-033 PR-033-C: fired by the `TrustSettings` surface's own
    /// purge control -- opens the real confirmation dialog. No I/O here;
    /// the real purge only happens on `ModalActivate` with focus on
    /// `Purge` (see `TranscriptPurgeModal`'s own doc for why this dialog
    /// is not folded into a direct mutation the way capture-decline is:
    /// deletion is irreversible, and `what-purge-must-remove.md` requires
    /// the confirmation to name the scope and say it cannot be undone).
    OpenTranscriptPurgeDialog,
    /// RFC-038 PR-038-A: `Ctrl+V` while the Project Board's empty-state
    /// path field would receive the key (`handle_project_board_path_
    /// field_key`) -- the same async-round-trip shape `TerminalPasteResolved`
    /// already established for `Ctrl+Shift+V`, since `iced` has no
    /// synchronous clipboard access. Simpler than that one: this field is
    /// not a PTY, so there is no `TerminalInputPolicy` decision to defer
    /// -- `None` (empty or unreadable clipboard) is a silent no-op, the
    /// same as an empty terminal paste.
    PathFieldPasteResolved(Option<String>),
    /// RFC-038 PR-038-G: the Project Board's real, clickable "Browse..."
    /// button -- `iced`'s own click dispatch, not the reviewed keyboard
    /// router (mouse input was never part of that router's threat
    /// model; see `board::empty_state_view`'s own doc on why keyboard
    /// input specifically needed one). `NavigationAction::
    /// OpenFolderBrowser`'s `Shell` handler and this arm both call
    /// [`open_folder_browser`], so a mouse click and `Ctrl+Alt+B` open
    /// the exact same modal through the exact same setup, not two
    /// independently-maintained copies of it.
    OpenFolderBrowserButtonPressed,
    /// RFC-038 PR-038-G: `Space` while the folder browser is open --
    /// commits `scan.current_dir` as the new project, through the same
    /// `add_project_from_path` entry point every other route uses.
    /// `modal_subscription`'s own doc explains why this is a new
    /// recognised key rather than reusing `ModalActivate` (`Enter`):
    /// `Enter` already means "navigate into the highlighted row," and a
    /// folder browser genuinely needs both actions distinguishable.
    FolderBrowserChooseCurrentDirectory,
    /// RFC-038 PR-038-D: the Project Board's real, clickable "Open"
    /// button on a `Recent*`-kind row -- `iced`'s own click dispatch,
    /// the same mouse-is-outside-the-keyboard-router reasoning
    /// `OpenFolderBrowserButtonPressed` already established. Both this
    /// arm and `handle_project_board_row_key`'s own `Enter` case call
    /// [`reopen_recent_project`], so a mouse click and the keyboard
    /// one-key reopen (RFC-038's own OQ1: "offering it as one-key
    /// reopen") converge on the exact same setup.
    ReopenRecentProjectRowPressed(tekstide_core::project::ProjectId),
    /// RFC-039 PR-039-B: a real, clickable tab on the strip -- both this
    /// arm and `handle_tab_strip_key`'s own `Enter` case (when the
    /// highlighted item is index `1..=N`, not the home tab) call
    /// [`switch_to_project_tab`], so a mouse click and the strip's own
    /// keyboard navigation converge on the exact same setup. Distinct
    /// from `NavigationAction::SwitchActiveProject`'s global `Ctrl+Alt+N`
    /// accelerator, which *cycles*; this switches to the one specific
    /// project the user clicked or highlighted.
    SwitchActiveProjectTabPressed(tekstide_core::project::ProjectId),
    /// RFC-039 D1: the strip's permanent leftmost "Projects" tab -- both
    /// this arm and `handle_tab_strip_key`'s own `Enter` case (when the
    /// highlighted item is index 0) call [`go_to_project_board`], the
    /// visible control workflow 5 names, alongside the pre-existing
    /// `Ctrl+Alt+P` accelerator (unchanged, not replaced).
    GoToProjectBoardTabPressed,
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
    // RFC-022 PR-022-E ("the arrival model"), response 227: a promoted
    // approval dialog briefly ignores modal input after appearing, so a
    // keystroke already in flight (typing mid-word, or dismissing a
    // *different* modal that just closed and re-triggered promotion)
    // cannot immediately activate or dismiss it. Checked once, ahead of
    // the real `match` below, for exactly the four modal-input messages
    // every other modal already responds to -- this dialog's own
    // rendering/promotion/decision logic is otherwise unaffected.
    if matches!(
        message,
        Message::ModalFocusNext
            | Message::ModalFocusPrevious
            | Message::ModalActivate
            | Message::ModalDismiss
    ) && let Some(ModalContent::Approval(dialog)) = state.modal.as_ref()
        && dialog
            .ignore_input_until
            .is_some_and(|until| std::time::Instant::now() < until)
    {
        return Task::none();
    }

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
                        record_restricted_mode_blocked_if_applicable(state, &refusal);
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
            // RFC-038 PR-038-B: the route change itself now goes through
            // the normal `Some(command)` branch above
            // (`app_command_for`'s own doc on this action explains why
            // that is safe again as of PR-038-F) -- this only sets the
            // one piece of shell-local UI state `dispatch` has no way to
            // express.
            if action == NavigationAction::OpenProjectEntryField {
                state.path_field_requested = true;
            }
            // RFC-038 PR-038-C: reachable from anywhere, any route or
            // mode -- global keybindings are matched by
            // `route_non_modal_input` before terminal focus or shell
            // zone are even consulted, and `non_modal_subscription`
            // (the only source of `RoutedInput::Shell`) only runs while
            // no modal is already open, so this can never overwrite one.
            if action == NavigationAction::OpenHelp {
                state.modal = Some(ModalContent::Help);
            }
            // RFC-038 PR-038-G: the keyboard accelerator alongside the
            // real button (`Message::OpenFolderBrowserButtonPressed`) --
            // both converge on `open_folder_browser`.
            if action == NavigationAction::OpenFolderBrowser {
                open_folder_browser(state);
            }
            // RFC-039 PR-039-B: no single `AppCommand` can express
            // "switch to the next project" -- *which* project that is
            // depends on `AppState::projects()`'s own current order and
            // the currently active id, both shell-layer computations
            // core has no route/mode command for. The global
            // accelerator alongside the strip's own real, visible
            // controls (a tab click, or the strip's own keyboard
            // navigation) -- `app_command_for`'s own doc on this action
            // explains why it is not in that function's `Some` group.
            if action == NavigationAction::SwitchActiveProject {
                cycle_to_next_active_project(state);
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
            // RFC-039 PR-039-B: the tab strip is the first, and so far
            // only, `TabStrip` consumer -- naturally mutually exclusive
            // with every `Sidebar`/`MainArea` consumer below, since
            // `surface_input.target()` is always exactly one zone.
            if surface_input.target() == FocusZone::TabStrip {
                handle_tab_strip_key(state, surface_input.key());
            }
            // RFC-019 PR-019-D: the editor is the second real consumer --
            // a key routed to `MainArea` while in Content mode with an
            // active document edits it. `apply_edit_key` decides *what*
            // the next text is (append-only, see its own doc comment for
            // why); this arm only decides *whether* a key reaches it at
            // all.
            //
            // Response 234: `ApprovalHistory` is a third `MainArea`
            // consumer, and mutually exclusive with the editor -- both
            // `handle_editor_key` and `handle_approval_history_key`
            // check `open_surface` themselves rather than this call site
            // deciding once and dispatching, so neither can silently
            // read a key meant for the other (an active document left
            // open underneath a surface switch must not keep editing
            // from stray keystrokes the user is aiming at the history
            // list instead).
            if surface_input.target() == FocusZone::MainArea {
                handle_editor_key(state, surface_input.key());
                handle_approval_history_key(state, surface_input.key());
                // RFC-032, response 248's required fix: a fourth
                // `MainArea` consumer, the same "each checks
                // `open_surface` itself" mutual-exclusion shape the
                // comment above already establishes for the other three.
                handle_trust_settings_key(state, surface_input.key());
                // RFC-038 PR-038-D: a sixth `MainArea` consumer, checking
                // `route() == ProjectBoard` in place of the first four's
                // `active_project()`/`open_surface()` guards, plus
                // `!path_field_is_showing(state)` for mutual exclusion
                // with the fifth (`Enter` means two different things to
                // the two of them -- submit the typed path, or reopen
                // the highlighted row -- so exactly one may claim it).
                // Called before the field, not after: the field's own
                // `return` below would otherwise skip this one entirely.
                handle_project_board_row_key(state, surface_input.key());
                // RFC-038 PR-038-A: a fifth `MainArea` consumer, checking
                // `empty_state.is_some()` in place of the other four's
                // `active_project()`/`open_surface()` guards -- naturally
                // mutually exclusive with all of them, since none of the
                // other four can be reachable while there is no active
                // project. Returned rather than called as a statement:
                // this is the one of the five that sometimes needs a
                // real `Task` (`Ctrl+V`'s async clipboard read).
                return handle_project_board_path_field_key(state, surface_input.key());
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
        Message::PathFieldPasteResolved(content) => {
            // Same "state changed enough during the round trip" silent
            // no-op precedent `TerminalPasteResolved` sets, checked with
            // the same [`path_field_is_showing`]
            // `handle_project_board_path_field_key` itself reads: the
            // field may have been dismissed (`Escape`) or already
            // submitted by the time this async read resolves.
            if !path_field_is_showing(state) {
                return Task::none();
            }
            if let Some(content) = content {
                push_to_path_field(state, &content);
            }
        }
        Message::OpenFolderBrowserButtonPressed => {
            open_folder_browser(state);
        }
        Message::FolderBrowserChooseCurrentDirectory => {
            choose_current_browsed_directory(state);
        }
        Message::ReopenRecentProjectRowPressed(project_id) => {
            reopen_recent_project(state, &project_id);
        }
        Message::SwitchActiveProjectTabPressed(project_id) => {
            switch_to_project_tab(state, &project_id);
        }
        Message::GoToProjectBoardTabPressed => {
            go_to_project_board(state);
        }
        Message::Input(RoutedInput::FocusNext) => state.focus = state.focus.next(),
        Message::Input(RoutedInput::FocusPrevious) => state.focus = state.focus.previous(),
        Message::ModalFocusNext => match state.modal.as_mut() {
            Some(ModalContent::LayerDemo { focus }) => *focus = focus.next(),
            Some(ModalContent::PasteConfirmation(modal)) => modal.focus = modal.focus.next(),
            Some(ModalContent::ExternalChange(modal)) => modal.focus = modal.focus.next(),
            Some(ModalContent::Approval(dialog)) => dialog.focus = dialog.focus.next(),
            Some(ModalContent::TrustGrant(modal)) => modal.focus = modal.focus.next(),
            Some(ModalContent::TranscriptPurge(modal)) => modal.focus = modal.focus.next(),
            // RFC-038 PR-038-C: nothing to focus -- a read-only surface,
            // not a dialog with buttons.
            Some(ModalContent::Help) => {}
            // RFC-038 PR-038-G: moves the highlighted row, clamped (not
            // wrapping) -- the same shape `handle_explorer_key` already
            // uses for the project explorer's own Up/Down, since this is
            // a list to move through, not a small Tab-cycled button set
            // like every other modal above.
            Some(ModalContent::FolderBrowser(modal)) => {
                let row_count = crate::surface::explorer::visible_browse_rows(&modal.scan).len();
                if row_count > 0 {
                    modal.highlight = (modal.highlight + 1).min(row_count - 1);
                }
            }
            None => {}
        },
        Message::ModalFocusPrevious => match state.modal.as_mut() {
            Some(ModalContent::LayerDemo { focus }) => *focus = focus.previous(),
            Some(ModalContent::PasteConfirmation(modal)) => modal.focus = modal.focus.previous(),
            Some(ModalContent::ExternalChange(modal)) => modal.focus = modal.focus.previous(),
            Some(ModalContent::Approval(dialog)) => dialog.focus = dialog.focus.previous(),
            Some(ModalContent::TrustGrant(modal)) => modal.focus = modal.focus.previous(),
            Some(ModalContent::TranscriptPurge(modal)) => modal.focus = modal.focus.previous(),
            Some(ModalContent::Help) => {}
            Some(ModalContent::FolderBrowser(modal)) => {
                modal.highlight = modal.highlight.saturating_sub(1);
            }
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
        Message::ModalActivate => {
            match state.modal.take() {
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
                // RFC-022 PR-022-E: unlike Paste/ExternalChange above, *both*
                // of this dialog's own focus positions are real decisions --
                // there is no "closes without consequence" reading for
                // Approve/Reject the way Dismiss/anything-but-Reload is for
                // the other two. `decide_approval` records whichever one
                // focus landed on.
                Some(ModalContent::Approval(dialog)) => {
                    let decision = match dialog.focus {
                        ApprovalDialogButton::ApproveOnce => {
                            tekstide_core::approval::SimpleDecision::ApprovedOnce
                        }
                        ApprovalDialogButton::Reject => {
                            tekstide_core::approval::SimpleDecision::Rejected
                        }
                    };
                    decide_approval(state, *dialog, decision);
                }
                // RFC-032: the paste dialog's shape, not the approval
                // dialog's -- only `Grant` is a real decision. Any other
                // focus (`Cancel`), or `ModalDismiss` (Escape) below,
                // closes without granting anything.
                Some(ModalContent::TrustGrant(modal)) if modal.focus == TrustGrantButton::Grant => {
                    apply_workspace_trust_grant(state, &modal);
                }
                // RFC-033 PR-033-C: the paste dialog's shape again --
                // only `Purge` is a real decision. Any other focus
                // (`Cancel`), or `ModalDismiss` (Escape) below, closes
                // without deleting anything.
                Some(ModalContent::TranscriptPurge(modal))
                    if modal.focus == TranscriptPurgeButton::Purge =>
                {
                    apply_transcript_purge(state, &modal);
                }
                // RFC-038 PR-038-G: unlike every arm above, this one
                // does not represent a final decision -- `Enter`
                // navigates the browser, it does not close it. `modal`
                // is `state.modal.take()`'s own owned value (this whole
                // match runs against it, not a `state.modal.as_mut()`
                // borrow), so it must be explicitly put back; every
                // other arm's implicit "stays closed" is what this one
                // deliberately does not do.
                Some(ModalContent::FolderBrowser(mut modal)) => {
                    navigate_folder_browser(&mut modal);
                    state.modal = Some(ModalContent::FolderBrowser(modal));
                }
                Some(ModalContent::LayerDemo { .. })
                | Some(ModalContent::PasteConfirmation(_))
                | Some(ModalContent::ExternalChange(_))
                | Some(ModalContent::TrustGrant(_))
                | Some(ModalContent::TranscriptPurge(_))
                // RFC-038 PR-038-C: nothing to activate -- closes the
                // same way `ModalDismiss` does, the same "no real
                // decision for this arm" shape every other no-op case
                // above already has.
                | Some(ModalContent::Help)
                | None => {}
            }
            // RFC-022 PR-022-E, response 227: re-evaluate after every
            // activation, not only a real approval decision -- any
            // `ModalActivate` closes whatever modal was open (its own
            // action, or a no-op focus), freeing the slot the same way
            // `ModalDismiss` does below.
            evaluate_promotion(state);
        }
        Message::ModalDismiss => {
            state.modal = None;
            // RFC-022 PR-022-E, response 227: re-evaluate on every modal
            // close, not only this dialog's own -- dismissing the paste
            // dialog can free the slot a queued Destructive proposal for
            // the active project has been waiting on. `evaluate_promotion`
            // itself is a no-op if nothing qualifies.
            evaluate_promotion(state);
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
        Message::ApprovalPollTick => {
            poll_approval_channels(state);
        }
        Message::OpenApprovalHistoryEntry(approval_id) => {
            open_approval_history_entry(state, &approval_id);
        }
        Message::WindowResized(size) => {
            state.window_size = Some(size);
            apply_terminal_geometry(state);
        }
        Message::WindowOpened(id) => {
            return iced::window::size(id).map(Message::WindowResized);
        }
        Message::OpenTrustGrantDialog => {
            open_trust_grant_dialog(state);
        }
        Message::RevokeWorkspaceTrust => {
            revoke_workspace_trust(state);
        }
        Message::ToggleTranscriptCaptureDeclined => {
            toggle_transcript_capture_declined(state);
        }
        Message::OpenTranscriptPurgeDialog => {
            open_transcript_purge_dialog(state);
        }
    }
    Task::none()
}

/// Terminal resize handoff: the layout constants
/// [`terminal_workspace_content_size`]'s chrome subtraction is built
/// from, named rather than duplicated as bare literals -- each mirrors
/// the real value the corresponding view function already uses
/// ([`top_bar`]'s `padding(8)`, [`status_bar`]'s `padding(6)`,
/// [`sidebar_view`]'s fixed `220.0`, [`main_area_view`]'s `padding(16)`,
/// [`terminal_workspace_view`]'s own `column(...).spacing(8)`, and
/// `session_bar::view`'s `padding(4)` around `iced`'s own default text
/// size, `Pixels(16.0)`, since that view specifies no explicit
/// `.size(...)`). Kept next to this comment specifically so a chrome
/// change and this function are easy to notice out of sync -- response
/// 242 disclosed that kind of drift as cosmetic (a gap or a clip), not a
/// correctness risk, because every consumer of the *result* still agrees
/// with the others; see [`crate::surface::terminal::TerminalPane::resize`].
const TOP_BAR_PADDING_PX: f32 = 8.0;
const STATUS_BAR_PADDING_PX: f32 = 6.0;
const SIDEBAR_WIDTH_PX: f32 = 220.0;
const MAIN_AREA_PADDING_PX: f32 = 16.0;
const WORKSPACE_ROW_SPACING_PX: f32 = 8.0;
const SESSION_BAR_PADDING_PX: f32 = 4.0;
const SESSION_BAR_TEXT_SIZE_PX: f32 = 16.0;

/// Terminal resize handoff: the width/height available to
/// `terminal_workspace_view`'s `panes_view` row, computed from
/// `state.window_size` and the named chrome constants above rather than
/// read from `iced`'s own live layout measurement -- `None` until the
/// first `Message::WindowResized` arrives (see `state.window_size`'s own
/// doc). This is the one function both [`apply_terminal_geometry`]
/// (`update()`, real I/O) and [`terminal_workspace_view`] (`view()`,
/// the split decision) call -- one formula, not two that could drift
/// apart, matching response 242's requirement.
fn terminal_workspace_content_size(state: &State) -> Option<(f32, f32)> {
    let window_size = state.window_size?;

    let top_bar_height = 2.0 * TOP_BAR_PADDING_PX
        + crate::surface::terminal::line_height_px(state.theme.font_size_heading());
    let status_bar_height = 2.0 * STATUS_BAR_PADDING_PX
        + crate::surface::terminal::line_height_px(state.theme.font_size_status());
    let content_area_height = (window_size.height - top_bar_height - status_bar_height).max(0.0);

    let main_area_width = (window_size.width - SIDEBAR_WIDTH_PX).max(0.0);
    let main_area_inner_width = (main_area_width - 2.0 * MAIN_AREA_PADDING_PX).max(0.0);
    let main_area_inner_height = (content_area_height - 2.0 * MAIN_AREA_PADDING_PX).max(0.0);

    let notice_count = [
        state.terminal_launch_notice.is_some(),
        state.terminal_paste_notice.is_some(),
        state.agent_run_launch_notice.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    let notice_height = crate::surface::terminal::line_height_px(state.theme.font_size_body());
    let notices_height = notice_count as f32 * (notice_height + WORKSPACE_ROW_SPACING_PX);

    let session_bar_height = 2.0 * SESSION_BAR_PADDING_PX
        + crate::surface::terminal::line_height_px(SESSION_BAR_TEXT_SIZE_PX);

    let panes_width = main_area_inner_width;
    let panes_height =
        (main_area_inner_height - notices_height - session_bar_height - WORKSPACE_ROW_SPACING_PX)
            .max(0.0);

    Some((panes_width, panes_height))
}

/// Terminal resize handoff: the single point where a computed window
/// geometry becomes a real PTY resize. Applies the same computed
/// `(rows, cols)` to **every** tracked pane, visible or hidden
/// (`state.terminal_panes`, not just the currently-shown slots) --
/// response 242: "a computed size needs no measurement, so a hidden pane
/// can be sized on the same basis as a visible one." `TerminalPane::resize`
/// is itself a no-op when the clamped size is unchanged, which is the
/// resize-storm bound this handoff's review gate asks for: many
/// `WindowResized` events during a drag collapse to the same character
/// grid until a real glyph/line boundary is crossed, so most calls here
/// touch neither the PTY nor `Term`.
///
/// **Response 243's required fix**: called from three places, not only
/// `Message::WindowResized`'s handler -- also from `Message::WindowOpened`'s
/// handler (indirectly, once the real size it queried arrives as a
/// `WindowResized`) and from both production pane-launch call sites
/// (`attempt_terminal_launch`, `attempt_agent_run_launch_with_profile`),
/// right after the new pane is pushed. Without the launch-site calls, a
/// pane launched between boot and the first live resize -- the common
/// case, since most sessions never drag the window edge -- stayed at the
/// `ROWS`/`COLS` launch default forever; a `None` `state.window_size`
/// (before the very first size arrives) makes this a no-op, so a pane
/// launched in that narrow window still self-corrects the moment it
/// does.
fn apply_terminal_geometry(state: &mut State) {
    let Some((panes_width, panes_height)) = terminal_workspace_content_size(state) else {
        return;
    };
    let font_size = state.theme.font_size_body();

    let per_pane_width = match crate::surface::terminal::layout_class_for(panes_width, font_size) {
        tekstide_core::navigation::TerminalLayoutClass::Wide => {
            (panes_width - crate::surface::terminal::PANE_GAP_PX) / 2.0
        }
        tekstide_core::navigation::TerminalLayoutClass::Narrow => panes_width,
    };

    let (cols, rows) =
        crate::surface::terminal::pane_dimensions_for_area(per_pane_width, panes_height, font_size);

    for pane in &mut state.terminal_panes {
        let _ = pane.resize(rows, cols);
    }
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

    // change-detection-wiring handoff, Slice C (D3): exit is the
    // completion trigger, but only for a terminal an `AgentRun` actually
    // owns -- a plain terminal (`Ctrl+Alt+T`) keeps the exact behaviour
    // in the `else` branch below, unchanged. Read fresh rather than
    // cached, the same discipline every other post-launch fact in this
    // module already follows.
    let owning_agent_run_id = state
        .app_shell
        .state()
        .active_project()
        .and_then(|project| {
            project
                .agent_runs()
                .iter()
                .find(|run| run.terminal_id.as_ref() == Some(&terminal_id))
        })
        .map(|run| run.id.clone());

    if let Some(agent_run_id) = owning_agent_run_id {
        // `apply_agent_terminal_outcome` marks the terminal exited *and*
        // transitions the run's own status together, so the two facts
        // cannot land out of step with each other the way calling
        // `mark_terminal_exited` and a separate status transition
        // sequentially could.
        let _ = state.app_shell.state_mut().apply_agent_terminal_outcome(
            &agent_run_id,
            &terminal_id,
            &outcome,
        );
        attempt_generated_change_detection(state, &agent_run_id);
    } else {
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
    }

    let _ = state
        .app_shell
        .state_mut()
        .assign_terminal_visible_slot(&terminal_id, tekstide_core::domain::VisibleSlot::Hidden);
    if let Some(store) = audit_store.as_mut() {
        let _ = tekstide_core::audit::AuditCoordinator::new(store, &mut audit_health)
            .record_plain_terminal_terminated(project_id, terminal_id, &outcome);
    }
}

/// change-detection-wiring handoff, Slice C: the completion half of the
/// wiring -- runs generated-change detection against the baseline
/// captured at launch (`state.agent_run_change_baselines`, populated by
/// `attempt_agent_run_launch_with_profile`), and calls
/// `add_detected_generated_change_set` for real when there is one to
/// create. Best-effort past this point, the same discipline
/// `record_terminal_exit` already follows for everything after a
/// terminal's exit is recorded: a missing baseline (this run's terminal
/// exited before launch finished capturing one, a `None` active
/// project, or detection already ran once for this run) is not an
/// error, just nothing further to do.
///
/// **Makes a real `ChangeSet` buildable in production, not diff review
/// reachable.** RFC-020's own surface still renders nothing -- see the
/// handoff's own gate for why that distinction is stated explicitly
/// rather than left implicit.
///
/// **D2**: always records `detected.status` in
/// `state.agent_run_change_detection_status`, whether or not a
/// `ChangeSet` results -- a truncated or failed scan must stay
/// distinguishable from a genuinely clean one (`Complete`, zero
/// changes), never collapse into the same "no `ChangeSet` for this run"
/// shape both currently produce in `project.change_sets()` alone.
fn attempt_generated_change_detection(
    state: &mut State,
    agent_run_id: &tekstide_core::domain::AgentRunId,
) {
    let Some(baseline) = state.agent_run_change_baselines.remove(agent_run_id) else {
        return;
    };
    let Some(project) = state.app_shell.state().active_project() else {
        return;
    };
    let detector =
        tekstide_core::project::GeneratedChangeDetector::new(generated_change_detection_policy());
    let detected = detector.detect_filesystem_changes(project, &baseline);
    state
        .agent_run_change_detection_status
        .insert(agent_run_id.clone(), detected.status);
    // Not localized: nothing renders a `ChangeSet.summary` in production
    // yet (RFC-020 does not exist) -- see the handoff's own reachability
    // note. Revisit once a real surface reads this field.
    let _ = state
        .app_shell
        .state_mut()
        .add_detected_generated_change_set(
            &baseline,
            &detected,
            Some(agent_run_id),
            "Filesystem changes detected after this run exited",
        );
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

/// RFC-038 PR-038-A: `add_project_from_path`'s error, minus the raw path
/// it embeds in its own `Display` -- that path lives in
/// `state.path_field` already (the field the user is still looking at
/// and can correct), and rendering `error.to_string()` directly would
/// hand an un-escaped, un-bounded filesystem path straight to `text(...)`,
/// exactly what `what-a-path-field-must-not-trust.md` §1 says never to
/// do. This enum carries only *which kind* of refusal happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathFieldError {
    DoesNotExist,
    NotDirectory,
    PermissionDenied,
    CannotReadFolder,
    SymlinkAmbiguous,
}

impl PathFieldError {
    fn from_validation_error(
        error: &tekstide_core::project::root::ProjectRootValidationError,
    ) -> Self {
        use tekstide_core::project::root::ProjectRootValidationError as E;
        match error {
            E::DoesNotExist { .. } => Self::DoesNotExist,
            E::NotDirectory { .. } => Self::NotDirectory,
            E::PermissionDenied { .. } => Self::PermissionDenied,
            E::CannotReadFolder { .. } => Self::CannotReadFolder,
            E::SymlinkAmbiguous { .. } => Self::SymlinkAmbiguous,
        }
    }
}

/// A compile-time literal symbol, not the displayed word -- same
/// division of labour `terminal_launch_refusal_symbol` already uses.
fn path_field_error_symbol(error: PathFieldError) -> &'static str {
    match error {
        PathFieldError::DoesNotExist => "does-not-exist",
        PathFieldError::NotDirectory => "not-directory",
        PathFieldError::PermissionDenied => "permission-denied",
        PathFieldError::CannotReadFolder => "cannot-read-folder",
        PathFieldError::SymlinkAmbiguous => "symlink-ambiguous",
    }
}

/// The one catalog lookup a path-field failure's full text takes -- same
/// shape `terminal_launch_refusal_text` uses, so a test can assert over
/// what actually renders. `raw_path` is bounded and escaped here, at
/// render (`what-a-path-field-must-not-trust.md` §3), following RFC-023's
/// `bound_key_segment` in spirit -- truncate the **raw** chars first,
/// escape second -- but through `text_safety::quote_untrusted`'s single
/// canonical whole-string API rather than a second, hand-rolled escaping
/// call: `quote_untrusted` already does the escaping *and* the bidi
/// isolation this embedded value needs, and `escape_untrusted_chars`
/// alone would either skip isolation or (called in addition to
/// `quote_untrusted`) escape the same text twice for no benefit --
/// `what-a-path-field-must-not-trust.md` §3's "do not write a second
/// escaping routine" reading applies to *calling* the primitive twice,
/// not only to reimplementing it. Truncating before escaping (not
/// after) is still required: escaping expands (a marker is several
/// characters), so truncating post-escape could cut one in half.
const MAX_PATH_FIELD_ERROR_DISPLAY_CHARS: usize = 128;

fn path_field_error_text(catalog: &Catalog, raw_path: &str, error: PathFieldError) -> String {
    let mut truncated: String = raw_path
        .chars()
        .take(MAX_PATH_FIELD_ERROR_DISPLAY_CHARS)
        .collect();
    if raw_path.chars().count() > MAX_PATH_FIELD_ERROR_DISPLAY_CHARS {
        truncated.push('\u{2026}');
    }
    let quoted = tekstide_core::text_safety::quote_untrusted(&truncated);
    catalog.get_with_args(
        "project-board-path-field-error",
        &CatalogArgs::new()
            .trusted_symbol("reason", path_field_error_symbol(error))
            .untrusted("path", &quoted),
    )
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

/// RFC-031 PR-031-A: `restricted_mode_blocked`'s only call site.
/// **Only the workspace-discovery refusal, not every refusal** --
/// `RunLimitExceeded` and `ExecutableUnavailable` are not restricted-mode
/// blocks and must not produce this family. Reuses
/// `agent_run_launch_refusal_symbol`'s own `"workspace-blocked"`
/// discrimination by matching the identical pattern that symbol checks,
/// rather than re-deriving the distinction a second way that could drift
/// from the first. Best-effort, matching every other producer call site
/// in this crate (`record_paste_blocked`'s own): a failed audit write
/// must never turn a real refusal into a second, different failure.
fn record_restricted_mode_blocked_if_applicable(
    state: &mut State,
    refusal: &AgentRunLaunchRefusal,
) {
    if !matches!(
        refusal,
        AgentRunLaunchRefusal::Validation(
            tekstide_core::agent::AgentRunLaunchValidationError::WorkspaceDiscoveryBlocked { .. }
        )
    ) {
        return;
    }
    let Some(project_id) = state.app_shell.state().active_project_id().cloned() else {
        return;
    };
    let mut audit_store = open_real_audit_store(&state.app_shell);
    let mut audit_health = tekstide_core::audit::AuditHealth::default();
    if let Some(store) = audit_store.as_mut() {
        let _ = tekstide_core::audit::AuditCoordinator::new(store, &mut audit_health)
            .record_restricted_mode_blocked(project_id);
    }
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

/// change-detection-wiring handoff, Slice C, review response 252's D4
/// decision: the production entry cap, explicit rather than inherited
/// from `GeneratedChangeDetectionPolicy::default()`'s `4,096`. This
/// repository alone is 1,506 entries after the D1 exclusions -- 37% of
/// the default cap -- so a project a few times this size already gets
/// nothing back under the default. `16,384` measures to a worst-case
/// synchronous cost of ~60ms at this project's own measured per-entry
/// rate (3.65 µs/entry, response 252), once at agent-run launch and
/// once at exit, on the calling thread -- accepted as comfortably
/// inside what a process spawn already costs at those two moments.
/// `65,536` was considered and rejected: a ~239ms stall, twice per run,
/// is not a cost response 252 was willing to accept on the calling
/// thread. `max_changed_paths` is untouched -- a separate limit with
/// its own semantics, left for Slice D.
fn generated_change_detection_policy() -> tekstide_core::project::GeneratedChangeDetectionPolicy {
    tekstide_core::project::GeneratedChangeDetectionPolicy {
        max_entries: 16_384,
        ..tekstide_core::project::GeneratedChangeDetectionPolicy::default()
    }
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
    attempt_agent_run_launch_with_profile_and_state_root(
        state,
        profile,
        open_real_agent_run_state_root(),
    )
}

/// transcript-capture-evidence handoff: the same testability split
/// [`attempt_agent_run_launch_with_profile`] already gives `profile` --
/// applied to `state_root` too, so a test can point transcript capture
/// at a temporary directory instead of the developer's real
/// `$XDG_STATE_HOME`/`open_real_agent_run_state_root`. The real launch
/// path (`attempt_agent_run_launch_with_profile`, `Ctrl+Alt+A`'s own
/// route) is the only production caller of the real resolution; every
/// existing test that does not care about transcript paths keeps
/// calling that wrapper unchanged.
///
/// RFC-033 PR-033-B: this is also the one production caller that reads
/// the real per-project opt-out (`ProjectSession::transcript_capture_declined`)
/// and threads it into `capture_enabled` -- `attempt_agent_run_launch_with_profile_state_root_and_capture`'s
/// own seam, built in PR-033-A specifically so this wiring would be a
/// value change here, not a restructure. Defaults to capture-on
/// (`true`) when there is no active project, matching the inner
/// function's own early-return-on-no-project shape: the value is
/// discarded before it matters.
fn attempt_agent_run_launch_with_profile_and_state_root(
    state: &mut State,
    profile: tekstide_core::agent::AiCliProfile,
    state_root: Option<std::path::PathBuf>,
) -> Result<(), AgentRunLaunchRefusal> {
    let capture_enabled = !state
        .app_shell
        .state()
        .active_project()
        .is_some_and(|project| project.transcript_capture_declined());
    attempt_agent_run_launch_with_profile_state_root_and_capture(
        state,
        profile,
        state_root,
        capture_enabled,
    )
}

/// RFC-033 PR-033-A handoff: the third testability split this same
/// function has now had -- `capture_enabled` is the seam PR-033-B's
/// real per-project opt-out will drive (a persisted setting instead of
/// a test literal); the wrappers above keep capture on, matching every
/// existing caller's current, unchanged behaviour.
///
/// **The fix this slice exists for**: `approval_state_root` is now set
/// explicitly, from the same `state_root`, whenever one is available --
/// unconditionally, not only when `with_local_bounded_transcript` is
/// also called. Before this, a `Managed` launch with capture disabled
/// (not reachable yet -- `claude_code_linux_default` is `Supervised` --
/// but about to become reachable the moment PR-033-B lands) would have
/// had no state root to bind its approval channel to at all, and failed
/// closed with `AgentAdapterApprovalError::StateRootMissing`.
/// `prepare_adapter_approval`'s own fallback to `transcript_state_root`
/// (RFC-022 PR-022-C response 216) was never wrong -- this call site
/// simply never took up the escape hatch response 216 built.
fn attempt_agent_run_launch_with_profile_state_root_and_capture(
    state: &mut State,
    profile: tekstide_core::agent::AiCliProfile,
    state_root: Option<std::path::PathBuf>,
    capture_enabled: bool,
) -> Result<(), AgentRunLaunchRefusal> {
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
            request = if capture_enabled {
                request.with_local_bounded_transcript(state_root.clone())
            } else {
                request.without_transcript_capture()
            };
            request = request.with_approval_channel(state_root);
        }

        let validation = tekstide_core::agent::AgentRunLaunchValidator
            .validate(project, &profile, &request)
            .map_err(AgentRunLaunchRefusal::Validation)?;
        tekstide_core::agent::AgentRunLaunchPlan::from_validation(validation, "Claude Code")
            .map_err(AgentRunLaunchRefusal::PlanTransition)?
    };

    let project_id = plan.spec().project_id().clone();
    // Captured before `plan` is consumed below -- `receive_proposal`
    // needs all three on every future proposal this run's adapter
    // sends, not only at launch (`ApprovalChannelServing`'s own doc
    // comment explains why `VerifiedCwd` specifically has to be captured
    // now rather than re-derived later).
    let verified_cwd = plan.spec().verified_cwd().clone();
    let project_root = plan.spec().project_root().to_path_buf();

    // change-detection-wiring handoff, Slice D (review response 258,
    // Slice D item 1): captured *before* the process is spawned below,
    // not merely "as early as possible after" it (Slice C's original
    // shape) -- a filesystem scan is not atomic with respect to a
    // concurrent writer, so a baseline taken while the agent process is
    // already live could observe a file mid-write and read it as
    // unchanged forever after. This closes that race by construction
    // rather than shrinking the window: nothing has run yet. Carries no
    // `agent_run_id` until one exists a few lines below --
    // `detected_change_association` requires an exact match for a
    // `Strong` association, so that assignment is load-bearing, not
    // cosmetic. Best-effort: no active project here would mean the
    // launch below could not succeed either, so this is not expected to
    // be skipped in practice, but a missing baseline is not fatal to a
    // launch that goes on to succeed anyway -- see
    // `attempt_generated_change_detection`'s own doc for what a missing
    // baseline means at the other end.
    let pre_launch_baseline = state.app_shell.state().active_project().map(|project| {
        tekstide_core::project::GeneratedChangeDetector::new(generated_change_detection_policy())
            .capture_filesystem_baseline(project)
    });

    let mut runtime = LinuxTerminalRuntime::new();
    let (agent_run_id, _events, approval_endpoint) = state
        .app_shell
        .state_mut()
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .map_err(AgentRunLaunchRefusal::Runtime)?;
    if let Some(mut baseline) = pre_launch_baseline {
        baseline.agent_run_id = Some(agent_run_id.clone());
        state
            .agent_run_change_baselines
            .insert(agent_run_id.clone(), baseline);
    }
    // `claude_code_linux_default` is `Supervised`, which never binds an
    // approval endpoint -- `None` here today. Registered for real once a
    // `Managed` profile can reach this path (response 227's found
    // defect: this endpoint used to be silently dropped one layer down;
    // it no longer is, and this is where a real one would be handed to
    // `state.approval_coordinator`'s own serving machinery). Re-resolves
    // the state root rather than reusing the one captured above (already
    // moved into the request at that point) -- cheap, and guaranteed
    // consistent: `prepare_adapter_approval` could only have bound a
    // real endpoint at all if this same resolution had already produced
    // `Some` once, earlier in this same call.
    if let Some(endpoint) = approval_endpoint {
        let state_root = open_real_agent_run_state_root()
            .expect("a resolvable state root, or prepare_adapter_approval would have failed closed instead of binding an endpoint");
        register_approval_channel(
            state,
            project_id.clone(),
            agent_run_id.clone(),
            verified_cwd,
            project_root,
            state_root,
            endpoint,
        );
    }

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
    // Response 243's required fix: size the freshly launched pane
    // immediately from whatever geometry is already known, rather than
    // leaving it at the launch-time default until the next live resize.
    apply_terminal_geometry(state);
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
///
/// RFC-038 PR-038-F: calls the scan-only entry point
/// (`scan_active_project_explorer_directory_without_navigating`), not
/// the navigating one. Response 233's own finding was that the
/// navigating method's `open_surface` side effect silently overwrote
/// `OpenActiveProjectSurface(surface)` for any surface but `TextEditor`,
/// worked around here by saving and restoring `open_surface` around the
/// call; PR-038-B found a second instance of the identical root cause
/// (the navigating method's `route` side effect undoing
/// `OpenProjectEntryField`'s own route change), worked around by
/// routing that action out of `app_command_for` entirely
/// (`app_command_for`'s own doc on `NavigationAction::OpenProjectEntryField`).
/// Two different pieces of state, two different workarounds, one
/// conflation -- closed at the root here instead of waiting for a third
/// workaround. The `open_surface` save/restore dance is gone: the
/// scan-only method never touches `open_surface` (or `mode`, or
/// `route`) in the first place, so there is nothing to save or restore.
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
        .scan_active_project_explorer_directory_without_navigating(std::path::PathBuf::new());
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
///
/// **Response 234**: also a no-op when `open_surface` does not render
/// the editor -- without this, a document left open from an earlier
/// `TextEditor` visit would keep silently absorbing keystrokes the user
/// is aiming at a different surface after switching, since this
/// function previously had no way to know any other surface existed
/// (`open_surface` had no real reader anywhere before response 233's
/// own `content_mode_view`).
///
/// **Response 235**: this guard and `content_mode_view`'s own render
/// decision used to be two separate, hand-written lists with nothing
/// keeping them in agreement -- both now defer to
/// [`surface_renders_editor`], the one place that answers "does this
/// surface show the editor," so the two questions cannot silently
/// diverge again the way they did between this function's first
/// version and response 235's fix.
fn handle_editor_key(state: &mut State, key: &input::KeyPress) {
    let Some(project) = state.app_shell.state().active_project() else {
        return;
    };
    if project.mode() != ProjectMode::Content {
        return;
    }
    if !surface_renders_editor(project.open_surface()) {
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

/// Response 234: keyboard access for the `ApprovalHistory` surface --
/// the same Up/Down-moves-highlight, Enter-activates shape
/// [`handle_explorer_key`] already establishes for the sidebar's own
/// list, so this crate's one interaction model (Tab/focus-marker/Enter)
/// covers the history list too, not only its mouse button.
///
/// **Why this was required, not optional polish**: RFC-022's own
/// design (`what-the-dialog-must-not-lie-about.md`, response 231) keeps
/// `Low`/`Medium` proposals off the promoted-modal path specifically
/// because *some genuinely are answerable* if the user reaches them
/// inside the adapter's window -- "reachable" assumed a working
/// interaction model. A mouse-only list makes every non-promoted
/// proposal unanswerable in principle for a keyboard user, silently
/// re-imposing the relabel-as-history design the owner rejected, for
/// most of this application's own interaction model. Response 234
/// named this a precedent decision, not a detail.
///
/// A no-op outside Content mode, off the `ApprovalHistory` surface, or
/// with nothing retained to navigate -- the same guard shape
/// [`handle_explorer_key`] uses for its own zone and list.
fn handle_approval_history_key(state: &mut State, key: &input::KeyPress) {
    let Some(project) = state.app_shell.state().active_project() else {
        return;
    };
    if project.mode() != ProjectMode::Content
        || project.open_surface() != ProjectOpenSurface::ApprovalHistory
    {
        return;
    }
    let row_count = project.approval_requests().len();
    if row_count == 0 {
        return;
    }
    state.approval_history_highlight = state.approval_history_highlight.min(row_count - 1);

    match &key.key {
        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
            state.approval_history_highlight =
                (state.approval_history_highlight + 1).min(row_count - 1);
        }
        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
            state.approval_history_highlight = state.approval_history_highlight.saturating_sub(1);
        }
        keyboard::Key::Named(keyboard::key::Named::Enter) => {
            let Some(project) = state.app_shell.state().active_project() else {
                return;
            };
            let Some(highlighted) = project
                .approval_requests()
                .get(state.approval_history_highlight)
            else {
                return;
            };
            let is_expired = project.expired_approval_ids().contains(&highlighted.id);
            if approval_request_is_live(highlighted, is_expired) {
                let approval_id = highlighted.id.clone();
                open_approval_history_entry(state, &approval_id);
            }
        }
        _ => {}
    }
}

/// RFC-032, response 248's required fix: keyboard access for the
/// `TrustSettings` surface -- without this, "Grant Trust…"/"Revoke
/// Trust" were `button(...)` with no key handler at all, mouse-only
/// exactly as `ApprovalHistory` was before response 234's fix. Worse
/// here: this surface is the *only* route to granting trust
/// (`app_command_for`'s mapping is `TrustSettings`'s one path in), so
/// mouse-only would have meant a keyboard user could not grant trust at
/// all, leaving the entire chain RFC-032 exists to unblock unreachable
/// for them.
///
/// **Still no highlight index**, even with three controls here now
/// (RFC-033 PR-033-B added the second, PR-033-C this third) --
/// [`handle_approval_history_key`]'s list needs one because its rows are
/// interchangeable (any row might be the one a user wants); this
/// surface's controls are not interchangeable, they are *independent*
/// settings, so a shared cursor would force navigating past ones a user
/// does not want to reach the one they do, for no reason. Each keeps its
/// own fixed key instead: Enter still activates whichever trust action
/// is currently shown (unchanged from before PR-033-B, mirroring
/// `trust_settings_view`'s own `is_trusted` branch exactly); Space
/// toggles capture, independent of trust state and always available;
/// Delete opens the purge confirmation, also always available and also
/// independent of trust state -- named-key choice deliberate, the same
/// reasoning Space's own doc comment already gives, and unclaimed by any
/// other handler in this file at the time of writing (confirmed before
/// use). What each key does still always matches what is rendered --
/// there are just three keys doing that now instead of one.
fn handle_trust_settings_key(state: &mut State, key: &input::KeyPress) {
    let Some(project) = state.app_shell.state().active_project() else {
        return;
    };
    if project.mode() != ProjectMode::Content
        || project.open_surface() != ProjectOpenSurface::TrustSettings
    {
        return;
    }
    match &key.key {
        keyboard::Key::Named(keyboard::key::Named::Enter) => {
            if project.trust_state() == tekstide_core::project::WorkspaceTrust::Trusted {
                revoke_workspace_trust(state);
            } else {
                open_trust_grant_dialog(state);
            }
        }
        keyboard::Key::Named(keyboard::key::Named::Space) => {
            toggle_transcript_capture_declined(state);
        }
        keyboard::Key::Named(keyboard::key::Named::Delete) => {
            open_transcript_purge_dialog(state);
        }
        _ => {}
    }
}

/// A real filesystem path can legitimately be long -- reasoned against
/// Linux's own `PATH_MAX` (4096 bytes) rather than an arbitrary
/// UI-driven number, so this bound never rejects a real path while still
/// bounding worst-case cost from a hostile or oversized paste. This
/// caps `state.path_field` itself (both typing and paste share
/// [`push_to_path_field`]); the separate, much tighter
/// [`MAX_PATH_FIELD_ERROR_DISPLAY_CHARS`] bounds only the *failure
/// notice*'s embedded copy of it, per `what-a-path-field-must-not-trust.md`
/// §3.
const MAX_PATH_FIELD_CHARS: usize = 4096;

/// The one signal deciding whether the path field is showing at all --
/// read by [`handle_project_board_path_field_key`] and `board::view`'s
/// call site alike, so the two cannot independently drift about it.
/// `true` in two disjoint cases: the board is genuinely empty
/// (`ProjectBoardViewModel::from_app_state`'s own `empty_state`), or
/// `Ctrl+Alt+O` asked for it on a populated board
/// (`state.path_field_requested`, RFC-038 PR-038-B's own addition for
/// the second-project case).
fn path_field_is_showing(state: &State) -> bool {
    state.app_shell.project_board().empty_state.is_some() || state.path_field_requested
}

/// RFC-038 PR-038-D, RFC-038's own OQ1 ("one-key reopen"): `Up`/`Down`
/// move `project_board_row_highlight` over every board row (mirrors
/// [`handle_approval_history_key`]'s own shape exactly -- clamp on
/// entry, then move, not wrapping); `Enter` reopens the highlighted row
/// through [`reopen_recent_project`], but only when it names a
/// `Recent*`-kind row. An `ActiveSession` row is already open -- there
/// is nothing for `Enter` to do to it, and switching to it is
/// `NavigationAction::SwitchActiveProject`, still `Configurable`/`None`
/// and out of this RFC's scope (see the known-limitations note PR-038-B's
/// own qa-evidence.md section already carries).
///
/// Guarded on `route() == ProjectBoard` (this zone means something
/// different on every other route) and `!path_field_is_showing(state)`
/// (mutual exclusion with [`handle_project_board_path_field_key`] --
/// `Enter` means two different things to the two of them).
fn handle_project_board_row_key(state: &mut State, key: &input::KeyPress) {
    if state.app_shell.route() != AppRoute::ProjectBoard || path_field_is_showing(state) {
        return;
    }
    let row_count = state.app_shell.project_board().rows.len();
    if row_count == 0 {
        return;
    }
    state.project_board_row_highlight = state.project_board_row_highlight.min(row_count - 1);

    match &key.key {
        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
            state.project_board_row_highlight =
                (state.project_board_row_highlight + 1).min(row_count - 1);
        }
        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
            state.project_board_row_highlight = state.project_board_row_highlight.saturating_sub(1);
        }
        keyboard::Key::Named(keyboard::key::Named::Enter) => {
            let board = state.app_shell.project_board();
            let Some(row) = board.rows.get(state.project_board_row_highlight) else {
                return;
            };
            if row.row_kind != tekstide_core::project_board::BoardRowKind::ActiveSession {
                let project_id = row.project_id.clone();
                reopen_recent_project(state, &project_id);
            }
        }
        _ => {}
    }
}

/// RFC-038 PR-038-D: reopens a remembered-but-not-currently-open
/// project by its id, through the exact same `add_project_from_path`
/// entry point PR-038-A's field and PR-038-G's browser both already
/// use -- the same audit record, the same live re-validation
/// (`what-a-path-field-must-not-trust.md` applies unchanged: a
/// remembered path is untrusted exactly as a typed or browsed one is,
/// however long it has sat in the cache).
///
/// **The property this function exists to guarantee**: `add_project_
/// session`'s own cached-trust restoration (`AppState::add_project_
/// session`, keyed by canonical root) is *not* proof by itself -- it is
/// the same user-writable display hint `project_board.rs`'s own
/// `recent_project_row` doc already calls one. [`verify_restored_trust`]
/// is called immediately after a successful add, the same demotion
/// pass [`State::new`] already runs once at boot for CLI-opened
/// projects, so a cached `Trusted` label the durable audit store does
/// not confirm is demoted before this function returns, never rendered
/// or acted on as real trust. See this slice's own qa-evidence.md for
/// the finding that led here: this same gap was live, unfixed, in both
/// pre-existing non-CLI call sites (the path field, the browser) --
/// fixed there too, in this same slice, not left for a third call site
/// to repeat.
///
/// A row whose remembered path no longer resolves (missing, unreadable,
/// permission-denied) fails through `add_project_from_path`'s own
/// real, live validation -- rendered by reusing the path field's own
/// notice machinery (`path_field`/`path_field_requested`/
/// `path_field_notice`) rather than a second, parallel notice type, so
/// the user lands on an editable field showing exactly the path that
/// failed and why, the same "never a silent no-op" shape
/// `a_bad_path_renders_a_notice_and_the_application_keeps_running`
/// already proves for a typed path.
fn reopen_recent_project(state: &mut State, project_id: &tekstide_core::project::ProjectId) {
    state.project_board_row_highlight = 0;
    let Some(restored) = state
        .app_shell
        .state()
        .recent_projects()
        .iter()
        .find(|restored| &restored.recent_project.project_id == project_id)
    else {
        return;
    };
    let root_path = restored.recent_project.root_path.clone();

    match state.app_shell.add_project_from_path(&root_path) {
        Ok(tekstide_core::app::AddProjectOutcome::Added(project_id)) => {
            record_new_project_added(state, project_id);
            verify_restored_trust(&mut state.app_shell);
        }
        // Should not normally happen -- a `Recent*`-kind row is, by
        // construction, not currently open -- but if the board's rows
        // and the live project set have somehow diverged, "nothing new
        // happened" is still the correct, existing precedent
        // (`attempt_open_project_from_path_field`'s own `FocusedExisting`
        // arm), not an error.
        Ok(tekstide_core::app::AddProjectOutcome::FocusedExisting(_)) => {}
        Err(error) => {
            state.path_field = root_path.display().to_string();
            state.path_field_requested = true;
            state.path_field_notice = Some(PathFieldError::from_validation_error(&error));
        }
    }
}

/// RFC-039 PR-039-B: workflow 4 ("Enter a project and work in it"),
/// reached by clicking a tab or by the strip's own keyboard navigation
/// landing `Enter` on one -- both converge here. `ApplicationShell::
/// switch_active_project` is the real state change *and* the route
/// change that makes it visible (own doc comment); `ensure_explorer_
/// scanned` afterward matches every other route-changing path in this
/// function, so entering a project this way primes its explorer cache
/// exactly as entering it any other way already does. A `project_id`
/// that no longer names an open project (the strip and the live project
/// set having somehow diverged between render and this call) is a
/// silent no-op -- the same precedent `reopen_recent_project`'s own
/// `FocusedExisting` arm already sets for an equivalent impossible case.
fn switch_to_project_tab(state: &mut State, project_id: &tekstide_core::project::ProjectId) {
    let _ = state.app_shell.switch_active_project(project_id);
    ensure_explorer_scanned(state);
}

/// RFC-039 D1: workflow 5 ("Return to the entrance"), reached by
/// clicking the strip's own permanent leftmost tab, by `Enter` with it
/// highlighted, or by the pre-existing `Ctrl+Alt+P` accelerator
/// (`app_command_for`'s own `OpenProjectBoard` arm) -- all three
/// converge on the same `AppCommand::OpenProjectBoard` dispatch, not
/// three independent copies of it.
fn go_to_project_board(state: &mut State) {
    state.app_shell.dispatch(AppCommand::OpenProjectBoard);
    ensure_explorer_scanned(state);
}

/// RFC-039 PR-039-B: `Ctrl+Alt+N`'s own handler -- cycles to the next
/// project in `AppState::projects()`'s own order, wrapping; a no-op
/// with fewer than two projects open (nothing to cycle to), and starts
/// from index 0 if, somehow, no project is currently active despite one
/// being open (defensive; `active_project_id()` should always be `Some`
/// whenever `projects()` is non-empty, but this function does not rely
/// on that holding to stay correct).
fn cycle_to_next_active_project(state: &mut State) {
    let project_count = state.app_shell.state().projects().len();
    if project_count < 2 {
        return;
    }
    let current_index = state
        .app_shell
        .state()
        .active_project_id()
        .and_then(|active_id| {
            state
                .app_shell
                .state()
                .projects()
                .iter()
                .position(|project| project.id() == active_id)
        });
    let next_index = match current_index {
        Some(index) => (index + 1) % project_count,
        None => 0,
    };
    let next_id = state.app_shell.state().projects()[next_index].id().clone();
    switch_to_project_tab(state, &next_id);
}

/// RFC-039 PR-039-B: the tab strip's own keyboard navigation --
/// `ArrowLeft`/`ArrowRight` move `tab_strip_highlight` among the
/// strip's own items (index 0 is the permanent "Projects" home tab;
/// indices `1..=N` are the `N` open projects), clamped, not wrapping,
/// the same shape every other highlight in this crate already uses,
/// just along the strip's own horizontal axis rather than a vertical
/// list's. `Enter` activates whichever is highlighted. A no-op outside
/// `FocusZone::TabStrip` -- this is the zone's own guard, the same
/// "each MainArea consumer checks its own precondition" shape this
/// function's siblings use, just for a zone rather than a mode/surface.
fn handle_tab_strip_key(state: &mut State, key: &input::KeyPress) {
    if state.focus != FocusZone::TabStrip {
        return;
    }
    let item_count = 1 + state.app_shell.state().projects().len();
    state.tab_strip_highlight = state.tab_strip_highlight.min(item_count - 1);

    match &key.key {
        keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
            state.tab_strip_highlight = (state.tab_strip_highlight + 1).min(item_count - 1);
        }
        keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
            state.tab_strip_highlight = state.tab_strip_highlight.saturating_sub(1);
        }
        keyboard::Key::Named(keyboard::key::Named::Enter) => {
            if state.tab_strip_highlight == 0 {
                go_to_project_board(state);
            } else {
                let index = state.tab_strip_highlight - 1;
                if let Some(project) = state.app_shell.state().projects().get(index) {
                    let project_id = project.id().clone();
                    switch_to_project_tab(state, &project_id);
                }
            }
        }
        _ => {}
    }
}

/// RFC-038 PR-038-A/B: the Project Board's path field, in both the
/// places it can appear. Mirrors the shape
/// [`handle_editor_key`]/[`handle_trust_settings_key`] already establish
/// for their own `MainArea` zones -- a no-op guard, then a direct match
/// on the key -- but has no `active_project()` to check: the empty-board
/// case is exactly the state that exists *because* there is none, and
/// the populated-board case (PR-038-B) is deliberately independent of
/// which project, if any, is active.
fn handle_project_board_path_field_key(state: &mut State, key: &input::KeyPress) -> Task<Message> {
    if !path_field_is_showing(state) {
        return Task::none();
    }
    match &key.key {
        keyboard::Key::Character(typed) => {
            if key.modifiers.control() && typed.as_ref() == "v" {
                return iced::clipboard::read().map(Message::PathFieldPasteResolved);
            }
            if !key.modifiers.control() && !key.modifiers.alt() {
                push_to_path_field(state, typed);
            }
        }
        keyboard::Key::Named(keyboard::key::Named::Space) => {
            push_to_path_field(state, " ");
        }
        keyboard::Key::Named(keyboard::key::Named::Backspace) => {
            let mut chars: Vec<char> = state.path_field.chars().collect();
            chars.pop();
            state.path_field = chars.into_iter().collect();
        }
        keyboard::Key::Named(keyboard::key::Named::Enter) => {
            attempt_open_project_from_path_field(state);
        }
        // RFC-038 PR-038-B: only meaningful when the field was *asked*
        // for (`Ctrl+Alt+O`) rather than being the empty board's own
        // permanent fixture -- there is nothing else on an empty board
        // for `Escape` to reveal by dismissing the field, so it is a
        // no-op there, matching every other MainArea handler's own
        // "guard, then act" shape rather than a special case here.
        keyboard::Key::Named(keyboard::key::Named::Escape) if state.path_field_requested => {
            state.path_field_requested = false;
            state.path_field.clear();
            state.path_field_notice = None;
        }
        _ => {}
    }
    Task::none()
}

/// The one place `state.path_field` grows, from either source (typing a
/// character, or [`Message::PathFieldPasteResolved`]'s pasted content) --
/// so [`MAX_PATH_FIELD_CHARS`] is enforced exactly once, the same
/// "one bounding point, not two" shape `attempt_paste_into_terminal`'s
/// own `MAX_PASTE_BYTES` cap uses for terminal paste. Silently stops
/// accepting more once the cap is hit, rather than truncating and
/// continuing -- a path this long has already left "someone typed a
/// real path" and entered "something is wrong," and continuing to
/// accumulate would trade one bound for a different one.
fn push_to_path_field(state: &mut State, text: &str) {
    let remaining = MAX_PATH_FIELD_CHARS.saturating_sub(state.path_field.chars().count());
    if remaining == 0 {
        return;
    }
    state.path_field.extend(text.chars().take(remaining));
}

/// RFC-038 PR-038-A: `Enter`'s real handler, and the field's second
/// (only other) call to `add_project_from_path` alongside `main.rs`'s
/// own -- see `what-a-path-field-must-not-trust.md` §5 and
/// `add_project_from_path_is_called_exactly_once_from_main_rs_and_nowhere_else`'s
/// updated allow-list. Mirrors `apply_workspace_trust_grant`'s own
/// direct-audit-write shape (`open_real_audit_store` +
/// `AuditCoordinator::new` + the one producer call) rather than reusing
/// `main.rs`'s `open_cli_project_path_and_record`: that helper's own
/// caller (`boot()`) `eprintln!`s and exits on `Err`, which
/// `what-a-path-field-must-not-trust.md` §2 says is catastrophic here --
/// a typo must never close the application. This function's failure
/// path renders instead, and the application keeps running either way.
/// **RFC-038 PR-038-D finding, fixed here retroactively**: a typed path
/// matching a recent project's canonical root inherits that project's
/// *cached* trust (`AppState::add_project_session`) with nothing to
/// confirm it against the durable audit store -- the same gap
/// [`reopen_recent_project`]'s own doc explains fixing at its own,
/// newer call site. A user who once granted trust to a project, closed
/// it, then retyped its exact path here got `Trusted` back with no
/// re-verification. [`verify_restored_trust`] closes it: the same
/// demotion pass `State::new` already runs once at boot.
fn attempt_open_project_from_path_field(state: &mut State) {
    state.path_field_notice = None;
    let path = state.path_field.clone();
    match state.app_shell.add_project_from_path(&path) {
        Ok(tekstide_core::app::AddProjectOutcome::Added(project_id)) => {
            record_new_project_added(state, project_id);
            verify_restored_trust(&mut state.app_shell);
            state.path_field.clear();
            state.path_field_requested = false;
        }
        Ok(tekstide_core::app::AddProjectOutcome::FocusedExisting(_)) => {
            // Same as `open_cli_project_path_and_record`'s own
            // `FocusedExisting` arm: nothing new happened, so no record.
            state.path_field.clear();
            state.path_field_requested = false;
        }
        Err(error) => {
            // Deliberately does not clear `path_field`: a rejected path
            // is exactly what the user needs to see and correct, not
            // have silently wiped out from under them.
            state.path_field_notice = Some(PathFieldError::from_validation_error(&error));
        }
    }
}

/// [`attempt_open_project_from_path_field`]'s own audit write -- same
/// shape `main.rs`'s `record_project_added_if_possible` uses
/// (best-effort: a failed audit write must never turn a real,
/// already-successful project add into a visible failure), reached
/// directly rather than through that private `main.rs` function, since
/// `shell.rs` already owns this exact `open_real_audit_store` +
/// `AuditCoordinator::new` shape for every other GUI-triggered producer
/// (`apply_workspace_trust_grant`, `revoke_workspace_trust`).
/// The shared audit write behind both of this crate's non-CLI
/// `add_project_from_path` call sites (the path field,
/// `attempt_open_project_from_path_field`, and the folder browser,
/// `choose_current_browsed_directory`) -- same shape `main.rs`'s
/// `record_project_added_if_possible` uses (best-effort: a failed audit
/// write must never turn a real, already-successful project add into a
/// visible failure), reached directly rather than through that private
/// `main.rs` function, since `shell.rs` already owns this exact
/// `open_real_audit_store` + `AuditCoordinator::new` shape for every
/// other GUI-triggered producer (`apply_workspace_trust_grant`,
/// `revoke_workspace_trust`).
fn record_new_project_added(state: &State, project_id: tekstide_core::project::ProjectId) {
    let Some(mut audit_store) = open_real_audit_store(&state.app_shell) else {
        return;
    };
    let mut audit_health = tekstide_core::audit::AuditHealth::default();
    let _ = tekstide_core::audit::AuditCoordinator::new(&mut audit_store, &mut audit_health)
        .record_project_added(project_id);
}

/// RFC-038 PR-038-G: opens the folder browser, at `$HOME` (falling back
/// to the filesystem root -- see [`starting_browse_directory`]). Both
/// `Ctrl+Alt+B` and the real "Browse..." button call this, so a
/// keyboard user and a mouse user reach the exact same setup, not two
/// independently-maintained copies of it.
///
/// If even the fallback somehow fails to scan (a pathological
/// environment with no readable filesystem root at all), this is a
/// silent no-op -- the same "nothing sensible to do" precedent
/// `attempt_terminal_launch`'s own "no active project" branch already
/// sets, rather than opening a modal with nothing real to show.
fn open_folder_browser(state: &mut State) {
    let start = starting_browse_directory();
    if let Ok(scan) = tekstide_core::project::root::browse_directory(
        &start,
        &tekstide_core::project::root::FileExplorerScanPolicy::linux_mvp(),
    ) {
        state.modal = Some(ModalContent::FolderBrowser(FolderBrowserModal {
            scan,
            highlight: 0,
            navigate_failed: false,
            open_error: None,
        }));
    }
}

/// `$HOME`, falling back to the filesystem root if unset or not a real,
/// readable directory -- the same `std::env::var_os("HOME")` convention
/// `tekstide-core`'s own config/profile/recent-project-store code
/// already uses (`config/path.rs`, `agent/profile.rs`,
/// `project/recent/store.rs`), read directly here since this crate has
/// no shared home-directory helper of its own yet to reuse.
fn starting_browse_directory() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .filter(|home| home.is_dir())
        .unwrap_or_else(|| std::path::PathBuf::from("/"))
}

/// RFC-038 PR-038-G: `Space`'s real handler -- commits
/// `scan.current_dir` (the directory currently being *shown*, not
/// whichever row is highlighted) as the new project, through the exact
/// same `add_project_from_path` entry point every other route uses.
/// Mirrors [`attempt_open_project_from_path_field`]'s own shape: never
/// exits, never touches core's canonicalisation/symlink logic directly
/// (`what-a-path-field-must-not-trust.md` §6 applies unchanged -- a
/// directory found by browsing is untrusted exactly as a typed one is),
/// and a failure is recorded on the modal rather than closing it, so
/// the user can back out or try another folder.
/// **RFC-038 PR-038-D finding, fixed here retroactively**: see
/// `attempt_open_project_from_path_field`'s own doc comment for the
/// same gap and the same fix -- a browsed path matching a recent
/// project's canonical root inherited cached trust with no audit-store
/// confirmation, which this call site shared before this fix.
fn choose_current_browsed_directory(state: &mut State) {
    let Some(ModalContent::FolderBrowser(modal)) = state.modal.as_mut() else {
        return;
    };
    modal.open_error = None;
    let path = modal.scan.current_dir.clone();

    match state.app_shell.add_project_from_path(&path) {
        Ok(tekstide_core::app::AddProjectOutcome::Added(project_id)) => {
            record_new_project_added(state, project_id);
            verify_restored_trust(&mut state.app_shell);
            state.modal = None;
        }
        Ok(tekstide_core::app::AddProjectOutcome::FocusedExisting(_)) => {
            state.modal = None;
        }
        Err(error) => {
            if let Some(ModalContent::FolderBrowser(modal)) = state.modal.as_mut() {
                modal.open_error = Some(PathFieldError::from_validation_error(&error));
            }
        }
    }
}

/// RFC-038 PR-038-G: `Enter`'s real handler for the folder browser --
/// navigates into the highlighted row (`Parent` or a subdirectory),
/// re-scanning at the new location. Distinct from
/// [`choose_current_browsed_directory`] (`Space`): this never calls
/// `add_project_from_path` and never closes the modal -- it only moves
/// where the browser is looking.
///
/// A failed navigation (permission changed, directory removed, racing
/// the earlier scan) leaves `scan`/`highlight` at the last good state
/// and sets `navigate_failed` instead -- the same "keep the last good
/// state, render the failure alongside it" shape `PathFieldError`
/// already established.
fn navigate_folder_browser(modal: &mut FolderBrowserModal) {
    let rows = crate::surface::explorer::visible_browse_rows(&modal.scan);
    let Some(row) = rows.get(modal.highlight) else {
        return;
    };
    let target = match row {
        crate::surface::explorer::BrowseRow::Parent => modal.scan.parent_dir.clone(),
        crate::surface::explorer::BrowseRow::Node(node) => Some(node.path.clone()),
    };
    let Some(target) = target else {
        return;
    };

    match tekstide_core::project::root::browse_directory(
        &target,
        &tekstide_core::project::root::FileExplorerScanPolicy::linux_mvp(),
    ) {
        Ok(new_scan) => {
            modal.scan = new_scan;
            modal.highlight = 0;
            modal.navigate_failed = false;
        }
        Err(_) => {
            modal.navigate_failed = true;
        }
    }
}

/// Shared between [`handle_approval_history_key`]'s Enter handling and
/// [`approval_history_entry_view`]'s own control-vs-plain-text decision
/// -- one definition of "live," not two that could drift apart.
fn approval_request_is_live(
    request: &tekstide_core::domain::ApprovalRequest,
    is_expired: bool,
) -> bool {
    request.decision == tekstide_core::domain::ApprovalDecision::Pending && !is_expired
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
    // Response 243's required fix: size the freshly launched pane
    // immediately from whatever geometry is already known, rather than
    // leaving it at the launch-time default until the next live resize.
    apply_terminal_geometry(state);
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
        // RFC-022 PR-022-E: the third contributor this doc comment
        // anticipated -- `ApprovalActive` was already a dedicated
        // variant, defined by RFC-021 ahead of this dialog actually
        // existing.
        Some(ModalContent::Approval(_)) => TerminalTrustedUiState::ApprovalActive,
        // RFC-032: the trust-grant dialog falls into the same generic
        // bucket `LayerDemo`/`ExternalChange` already share -- it is not
        // a terminal-paste concern any more than those are, but modal
        // exclusivity still needs it to read as active, not `Inactive`.
        // RFC-033 PR-033-C: the purge-confirmation dialog falls into the
        // same generic bucket, for the same reason `TrustGrant` does.
        // RFC-038 PR-038-C: the Help modal falls into the same generic
        // bucket -- not a terminal-paste concern, but modal exclusivity
        // still needs it to read as active while it is open.
        Some(ModalContent::LayerDemo { .. })
        | Some(ModalContent::ExternalChange(_))
        | Some(ModalContent::TrustGrant(_))
        | Some(ModalContent::TranscriptPurge(_))
        | Some(ModalContent::Help)
        // RFC-038 PR-038-G: same generic bucket, same reason.
        | Some(ModalContent::FolderBrowser(_)) => TerminalTrustedUiState::SecurityDialogActive,
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
pub(crate) fn open_real_audit_store(
    app_shell: &ApplicationShell,
) -> Option<tekstide_core::audit::AuditStore> {
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

/// RFC-032 PR-032-C, response 245: the audit store, not the
/// user-writable recent-projects cache, is authoritative for trust.
/// `AppState::add_project_session` (`tekstide-core`) optimistically
/// restores `Trusted` from that cache on reopen (PR-032-B) -- anything
/// that can write the cache file could otherwise mark a project trusted
/// with no corresponding `TrustGrant` in the durable record. This
/// confirms every currently-`Trusted` project against a real, applied
/// grant and demotes the ones the store does not confirm
/// (`ProjectSession::deny_unverified_trust`, no `AuditEvent` -- see its
/// own doc for why).
///
/// **Opens the audit store only when there is something to verify**
/// (`trusted_project_ids` non-empty) -- the same "ordinary use does not
/// create this file" discipline [`open_real_audit_store`]'s own doc
/// documents for `launch_terminal_demo_panes`. This is never a *new*
/// reason to create the file: a project can only be cached `Trusted` if
/// `grant_project_trust` ran for it at some point, and that call
/// already created the store itself (`append_required`'s own write) --
/// so by the time this function would open it, it already exists.
///
/// **Fails closed on a store that will not even open** -- an
/// unreadable, corrupt, or (implausibly, given the point above) missing
/// store must not be treated as silent confirmation; every currently-
/// `Trusted` project is demoted in that case, the same as one the store
/// opens but genuinely has no record for.
fn verify_restored_trust(app_shell: &mut ApplicationShell) {
    verify_restored_trust_against(app_shell, open_real_audit_store);
}

/// Factored out from [`verify_restored_trust`], the same reason
/// [`open_audit_store`] is factored out from [`open_real_audit_store`]:
/// so a test can supply a real, temp-dir-backed store instead of the
/// real `XDG_STATE_HOME`/`HOME`-resolved one, without duplicating this
/// function's own demotion logic against a mock.
fn verify_restored_trust_against(
    app_shell: &mut ApplicationShell,
    open_store: impl FnOnce(&ApplicationShell) -> Option<tekstide_core::audit::AuditStore>,
) {
    let trusted_project_ids: Vec<_> = app_shell
        .state()
        .projects()
        .iter()
        .filter(|project| project.trust_state() == tekstide_core::project::WorkspaceTrust::Trusted)
        .map(tekstide_core::project::ProjectSession::id)
        .cloned()
        .collect();
    if trusted_project_ids.is_empty() {
        return;
    }

    let Some(store) = open_store(app_shell) else {
        for project_id in &trusted_project_ids {
            if let Some(project) = app_shell.state_mut().project_mut(project_id) {
                project.deny_unverified_trust();
            }
        }
        return;
    };

    for project_id in &trusted_project_ids {
        let confirmed = store.has_applied_trust_grant(project_id).unwrap_or(false);
        if !confirmed && let Some(project) = app_shell.state_mut().project_mut(project_id) {
            project.deny_unverified_trust();
        }
    }
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
        // RFC-038 PR-038-B: the second-project case -- `Ctrl+Alt+O` needs
        // exactly the same route change `OpenProjectBoard` above does,
        // just on a populated board. **Was** special-cased entirely
        // outside this function (`update`'s `Shell` arm dispatching
        // `AppCommand::OpenProjectBoard` directly) because routing it
        // through this `Some` branch also triggers `ensure_explorer_
        // scanned` right after `dispatch`, and that function's own
        // navigating scan method used to unconditionally flip `route`
        // back to `ActiveProjectWorkspace`, undoing the very route
        // change this action exists to make. **Restored to the normal
        // path by PR-038-F**: `ensure_explorer_scanned` now calls a
        // scan-only entry point that touches no route at all, so the
        // failure mode this special case existed to dodge no longer
        // exists -- `ctrl_alt_o_opens_a_second_project_through_real_keys_on_a_populated_board`'s
        // own `route()` assertion is what proves it, and what an
        // ablation back to the navigating method fails against
        // (`ensure_explorer_scanned`'s own doc has the full account).
        // `update`'s `Shell` arm still sets `state.path_field_requested`
        // separately -- that part is shell-local UI state with no core
        // equivalent, not a route/mode change `dispatch` can express.
        NavigationAction::OpenProjectEntryField => Some(AppCommand::OpenProjectBoard),
        NavigationAction::ToggleProjectMode => Some(AppCommand::ToggleActiveProjectMode),
        NavigationAction::LaunchTerminal => Some(AppCommand::LaunchTerminal),
        // RFC-022 PR-022-D: mirrors `LaunchTerminal` -- the actual launch
        // (profile resolution, validation, PTY spawn, registration) is
        // real I/O and lives in `update`'s `Shell` arm, dispatched
        // alongside this command rather than inside it, the same split
        // `LaunchTerminal` already uses.
        NavigationAction::LaunchAgentRun => Some(AppCommand::LaunchAgentRun),
        // RFC-039 PR-039-B: not `Some(AppCommand::...)` -- no single core
        // command can express "switch to the next project", since
        // *which* project that is depends on `AppState::projects()`'s
        // own current order and the currently active id, both
        // shell-layer computations with no route/mode `AppCommand` to
        // carry them. `update`'s `Shell` arm special-cases it directly
        // (`cycle_to_next_active_project`), the same "no core route/mode
        // change through this path" shape `PasteIntoTerminal`/
        // `SaveActiveDocument` already use, just for a different reason
        // (theirs need real I/O this function has no room to attempt;
        // this one needs a target id core cannot compute for itself).
        NavigationAction::SwitchActiveProject => None,
        // RFC-022 PR-022-D: the route to an already-running run's detail
        // view -- no I/O, so no `update` special-case is needed the way
        // `LaunchAgentRun` above needs one.
        NavigationAction::OpenCurrentAgentRunDetail => Some(AppCommand::OpenActiveProjectSurface(
            ProjectOpenSurface::AgentRunDetail,
        )),
        // RFC-022 PR-022-E: the same shape as `OpenCurrentAgentRunDetail`
        // immediately above -- no I/O, so no `update` special-case is
        // needed. `ProjectOpenSurface::ApprovalHistory`'s own doc comment
        // explains why this is the *first* `open_surface`-conditional
        // branch `view()` has ever had (response 233 Finding: all seven
        // variants were previously set and never read anywhere).
        NavigationAction::OpenApprovalHistory => Some(AppCommand::OpenActiveProjectSurface(
            ProjectOpenSurface::ApprovalHistory,
        )),
        // RFC-032: the second real `open_surface`-conditional dispatch,
        // the same "no I/O, no `update` special-case" shape as
        // `OpenApprovalHistory` immediately above -- granting/revoking
        // themselves are the I/O, dispatched from controls *within* the
        // `TrustSettings` surface (`Message::OpenTrustGrantDialog`/
        // `Message::RevokeWorkspaceTrust`), not from opening the surface.
        NavigationAction::OpenTrustSettings => Some(AppCommand::OpenActiveProjectSurface(
            ProjectOpenSurface::TrustSettings,
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
        // RFC-038 PR-038-C: opening a modal is shell-local UI state
        // (`state.modal`), never part of `tekstide-core`'s `AppState`/
        // `AppRoute` model -- there is no `AppCommand` for it, the same
        // reason `OpenTrustGrantDialog`/`OpenTranscriptPurgeDialog` are
        // dedicated `Message` variants rather than `NavigationAction`s
        // routed through this function. `update`'s `Shell` arm sets
        // `state.modal` directly.
        | NavigationAction::OpenHelp
        // RFC-038 PR-038-G: same reason as `OpenHelp` immediately
        // above -- opening the folder-browser modal is shell-local UI
        // state, not a core route/mode change.
        | NavigationAction::OpenFolderBrowser
        | NavigationAction::OpenCommandPalette
        | NavigationAction::CycleVisibleTerminalSession
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
            ModalContent::Approval(dialog) => approval_dialog_view(state, dialog),
            ModalContent::TrustGrant(modal) => trust_grant_dialog_view(state, modal),
            ModalContent::TranscriptPurge(modal) => transcript_purge_dialog_view(state, modal),
            ModalContent::Help => help_modal_view(state),
            ModalContent::FolderBrowser(modal) => folder_browser_modal_view(state, modal),
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

    // Terminal resize handoff, response 243's required fix: unconditional,
    // unlike the wake/resize-event subscriptions below -- `state.window_size`
    // must end up populated even if no pane exists yet at boot (the demo
    // panes `State::new` may already have launched, and whatever the user
    // launches next, both need a real size the first time
    // `apply_terminal_geometry` runs, not only after a live drag).
    // `open_events()` fires once for this application's one window;
    // `Message::WindowOpened`'s handler is what turns that into a real,
    // queried size.
    let mut subscriptions = vec![iced::window::open_events().map(Message::WindowOpened)];

    // RFC-017 PR-017-C: only added when a demo pane exists (the env var
    // was set), so this changes nothing about the routing above for any
    // normal run -- the same "checked but usually absent" shape the
    // measurement branch above already uses.
    //
    // Terminal resize handoff: `resize_events()` is batched in alongside
    // the wake subscriptions, gated the same way -- nothing to resize
    // without a tracked pane. Genuinely event-driven (filters
    // `Event::Window(Event::Resized(_))`), not a per-frame subscription
    // like `window::frames()` -- see `Message::WindowResized`'s own doc.
    if !state.terminal_panes.is_empty() {
        subscriptions.extend(terminal_wake_subscriptions(&state.terminal_panes));
        subscriptions
            .push(iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size)));
    }
    // RFC-022 PR-022-E ("the arrival model"): polled regardless of modal
    // state -- a new proposal must still enter the queue while a
    // *different* modal is open (it just cannot promote until that modal
    // closes, per `evaluate_promotion`'s own guard), the same "keeps
    // running underneath modal exclusivity" shape terminal output
    // already has. Only added when there is something to poll, the same
    // "checked but usually absent" precedent the two branches above use.
    if !state.approval_channels.is_empty() {
        subscriptions
            .push(iced::time::every(APPROVAL_POLL_INTERVAL).map(|_| Message::ApprovalPollTick));
    }
    if subscriptions.is_empty() {
        routing
    } else {
        subscriptions.push(routing);
        Subscription::batch(subscriptions)
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
        // RFC-038 PR-038-G: `Up`/`Down` alias `Tab`/`Shift+Tab` --
        // harmless for every existing modal (a small, Tab-cycled button
        // set has no reason to also respond to arrow keys, but nothing
        // stops it from doing so identically), and the folder browser's
        // own list navigation is what actually needs them; no modal-
        // specific dispatch lives here, matching every other message
        // this function already produces unconditionally.
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::ArrowUp),
            ..
        } => Some(Message::ModalFocusPrevious),
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::ArrowDown),
            ..
        } => Some(Message::ModalFocusNext),
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Enter),
            ..
        } => Some(Message::ModalActivate),
        // RFC-038 PR-038-G: `Space` -- distinct from `Enter`
        // (`ModalActivate`, which navigates *into* the highlighted
        // row): this commits the folder currently being *shown*. Every
        // modal but the folder browser ignores it, the same "produced
        // unconditionally, each modal's own handler decides" shape
        // every message above already has.
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Space),
            ..
        } => Some(Message::FolderBrowserChooseCurrentDirectory),
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

/// RFC-039 D1: the project tab strip lives in this existing chrome,
/// composed here alongside the title rather than as a fourth top-level
/// row -- `view()`'s own `column![top_bar, content_area, status_bar]`
/// already runs in every mode, Terminal Immersion included, so nothing
/// new needs to be threaded through that composition for the strip to
/// survive every route/mode the title already does.
fn top_bar(state: &State) -> Element<'_, Message> {
    let content = column![
        text(state.window_title()).size(state.theme.font_size_heading()),
        project_tab_strip(state),
    ]
    .spacing(6);

    container(content)
        .width(Length::Fill)
        .padding(8)
        .style(chrome_style(
            state.theme.surface_elevated(),
            state.theme.foreground(),
            state.theme.border_default(),
        ))
        .into()
}

/// RFC-039 D1/PR-039-B: the project tab strip -- one real, clickable
/// `iced::widget::button` per open project (in `AppState::projects()`'s
/// own order), plus a permanent leftmost "Projects" button (workflow 5,
/// always present, even with no project open -- unlike PR-039-A's own
/// per-project tabs, which simply have nothing to render for). Both
/// mouse- and keyboard-operable: a click dispatches directly
/// (`Message::SwitchActiveProjectTabPressed`/`GoToProjectBoardTabPressed`);
/// `FocusZone::TabStrip`'s own `Left`/`Right` + `Enter` navigation
/// (`handle_tab_strip_key`) reaches the exact same [`switch_to_project_tab`]/
/// [`go_to_project_board`] either way, so a mouse user and a keyboard
/// user converge on the same setup, the same shape every other
/// button-plus-accelerator control in this crate already establishes.
///
/// **Two independent visual channels, deliberately not one** (response
/// 306's own required correction to PR-039-A, which had used the same
/// pair for both): *focus* -- `FocusZone::TabStrip`'s own keyboard
/// cursor, `tab_strip_highlight` -- keeps `zone_style`'s border
/// treatment and `focus_marker`'s `"> "` prefix, **unchanged**, since
/// focus indication must stay one consistent language across the whole
/// shell. *Active* -- which project is actually current -- moves to a
/// background-fill channel (`tab_active_style`) plus its own distinct
/// textual marker (`"\u{25CF} "` active / `"\u{25CB} "` inactive,
/// [`tab_marker`]), so a tab that is both focused and highlighted *and*
/// active (the common case) shows both signals at once, legibly,
/// instead of one meaning silently overwriting the other.
fn project_tab_strip(state: &State) -> Element<'_, Message> {
    let projects = state.app_shell.state().projects();
    let active_id = state.app_shell.state().active_project_id();
    let strip_focused = state.focus == FocusZone::TabStrip;
    let highlight = state.tab_strip_highlight.min(projects.len());

    let home_active = state.app_shell.route() == tekstide_core::route::AppRoute::ProjectBoard;
    let home_focused = strip_focused && highlight == 0;
    let home_tab = button(
        text(home_tab_label(&state.catalog, home_focused)).size(state.theme.font_size_body()),
    )
    .padding(6)
    .style(tab_active_style(state.theme, home_focused, home_active))
    .on_press(Message::GoToProjectBoardTabPressed);

    let mut tabs: Vec<Element<'_, Message>> = vec![home_tab.into()];
    tabs.extend(projects.iter().enumerate().map(|(index, project)| {
        let active = Some(project.id()) == active_id;
        let focused = strip_focused && highlight == index + 1;
        button(text(project_tab_label(project, active, focused)).size(state.theme.font_size_body()))
            .padding(6)
            .style(tab_active_style(state.theme, focused, active))
            .on_press(Message::SwitchActiveProjectTabPressed(project.id().clone()))
            .into()
    }));

    row(tabs).spacing(4).into()
}

/// **Two independent visual channels** (see [`project_tab_strip`]'s own
/// doc for the full reasoning): border colour/width for `focused` --
/// the same `border_focused`/`border_default` and `2.0`/`1.0` values
/// `zone_style` itself uses, not reused directly only because `iced`
/// gives `button`/`container` distinct `Style` types with no shared
/// trait between them -- and background fill for `active`. A project
/// that is both, the common case, shows both without either
/// overwriting the other.
fn tab_active_style(
    theme: Theme,
    focused: bool,
    active: bool,
) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style {
    move |_base_theme: &iced::Theme, _status: iced::widget::button::Status| {
        iced::widget::button::Style {
            background: Some(Background::Color(if active {
                theme.surface_elevated()
            } else {
                theme.background()
            })),
            text_color: theme.foreground(),
            border: Border {
                color: if focused {
                    theme.border_focused()
                } else {
                    theme.border_default()
                },
                width: if focused { 2.0 } else { 1.0 },
                radius: 0.0.into(),
            },
            ..iced::widget::button::Style::default()
        }
    }
}

/// The active-project marker, textually independent of [`focus_marker`]
/// -- `"\u{25CF} "` (a filled circle) for the active project, `"\u{25CB} "`
/// (a hollow one) otherwise, so a screen or terminal that cannot render
/// colour still shows which channel means what: `focus_marker`'s `"> "`/
/// `"  "` prefix for keyboard focus, this one for which project is
/// actually active. Composed with `focus_marker` by its one caller
/// ([`project_tab_label`]) -- never used alone, there is always a focus
/// state to render alongside an active one, and never by
/// [`home_tab_label`], which is not a project and does not get this
/// symbol (response 307).
fn tab_marker(focused: bool, active: bool) -> String {
    let active_symbol = if active { '\u{25CF}' } else { '\u{25CB}' };
    format!("{}{active_symbol} ", focus_marker(focused))
}

/// The strip's own bound -- shorter than
/// [`MAX_PATH_FIELD_ERROR_DISPLAY_CHARS`] (a single notice's own
/// embedded path) since several of these render side by side in one
/// fixed-width row; one long name must not push the rest of the strip
/// off-screen. Truncate-then-escape, the same order
/// `path_field_error_text` already establishes and for the same reason:
/// escaping expands text (a marker is several characters), so
/// truncating after escaping could cut one in half.
const MAX_TAB_NAME_DISPLAY_CHARS: usize = 24;

/// A project tab's own rendered label -- the marker plus the escaped,
/// bounded display name -- factored out from [`project_tab_strip`] for
/// the same testability reason every other rendered-string function in
/// this crate already is: the rendered string, not the `Element` tree.
/// `project.display_name()` is filesystem-derived and untrusted
/// (RFC-016), the same discipline `board::row_lines` already applies to
/// the identical field on the Project Board -- this is trusted chrome
/// (the top bar), not the RFC-016 terminal-grid exception, so escaping
/// is required here too
/// (`what-closing-a-project-must-not-lose.md` §5, D3).
pub(crate) fn project_tab_label(
    project: &tekstide_core::project::ProjectSession,
    active: bool,
    focused: bool,
) -> String {
    let raw_name = project.display_name();
    let mut truncated: String = raw_name.chars().take(MAX_TAB_NAME_DISPLAY_CHARS).collect();
    if raw_name.chars().count() > MAX_TAB_NAME_DISPLAY_CHARS {
        truncated.push('\u{2026}');
    }
    let quoted = tekstide_core::text_safety::quote_untrusted(&truncated);
    format!("{}{quoted}", tab_marker(focused, active))
}

/// The permanent leftmost "Projects" tab's own label -- trusted,
/// catalog-driven text (D1's own workflow 5 name), not filesystem-
/// derived, so unlike [`project_tab_label`] it is never escaped: there
/// is nothing untrusted in it to escape.
///
/// Deliberately carries only [`focus_marker`], not [`tab_marker`]'s
/// `"\u{25CF}"`/`"\u{25CB}"` pair (response 307's own finding): that
/// symbol's one meaning everywhere else in the strip is "this is
/// `AppState::active_project_id()`", a fact about a project session.
/// The home tab is not a project session, so giving it the same symbol
/// for a different fact ("`route() == ProjectBoard`") reads as a second
/// active project when the board is showing -- two filled circles
/// answering two different questions the same way. "You are here" is
/// still shown, honestly, through [`tab_active_style`]'s own background
/// fill and border (`project_tab_strip` passes `home_active` there
/// unchanged) -- just not through the symbol reserved for project
/// identity.
pub(crate) fn home_tab_label(catalog: &Catalog, focused: bool) -> String {
    format!(
        "{}{}",
        focus_marker(focused),
        catalog.get("project-tab-strip-home")
    )
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

/// The summary and the keyboard hint share one line on purpose:
/// [`content_area_height`] subtracts this bar's height to size real
/// terminal panes, so a second line would silently shrink every PTY.
fn status_bar(state: &State) -> Element<'_, Message> {
    container(
        row![
            text(status_bar_summary(state)).size(state.theme.font_size_status()),
            text(state.catalog.get("status-bar-key-hint")).size(state.theme.font_size_status()),
        ]
        .spacing(16),
    )
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
            AppRoute::ProjectBoard => {
                // Owned, not borrowed: this `String` is freshly computed
                // per render (the same "notice computed by `shell.rs`,
                // rendered by the surface" division
                // `terminal_launch_refusal_text` already uses), and
                // `board::view`'s returned `Element` must not outlive a
                // function-local borrow.
                let path_field_notice_text: Option<String> = state
                    .path_field_notice
                    .map(|error| path_field_error_text(&state.catalog, &state.path_field, error));
                crate::surface::board::view(
                    &state.app_shell.project_board(),
                    &state.catalog,
                    &state.theme,
                    &state.path_field,
                    path_field_notice_text,
                    state.path_field_requested,
                    Message::OpenFolderBrowserButtonPressed,
                    state.project_board_row_highlight,
                    Message::ReopenRecentProjectRowPressed,
                )
            }
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
        (Some(ProjectMode::Content), _) => content_mode_view(state),
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

/// Response 233: `ProjectMode::Content`'s dispatch on
/// `ProjectSession::open_surface()` -- the **first** `open_surface`-
/// conditional branch this crate has ever had. Every one of
/// `ProjectOpenSurface`'s seven other variants (including
/// `AgentRunDetail`, despite `OpenCurrentAgentRunDetail` having a real
/// `AppCommand` dispatch since PR-022-D) was, until this response, set
/// and never read anywhere in this crate outside tests -- `view()` only
/// ever branched on `ProjectMode`. Confirmed directly
/// (`grep -rn "open_surface()" crates/tekstide/src`, no non-test
/// matches) before writing this function, not assumed.
///
/// The six still-dormant variants all fall to today's behaviour (the
/// plain editor view, unconditional on `open_surface` exactly as it
/// always has been) -- building only `ApprovalHistory`'s real arm is
/// the intended scope of this response, not building six more surfaces
/// to prove the mechanism works for one.
///
/// **Response 235**: originally written as its own exhaustive match
/// naming all seven dormant variants individually, so a new variant
/// failed to compile here until someone decided what it renders. Moved
/// that exhaustiveness into [`surface_renders_editor`] instead, shared
/// with [`handle_editor_key`] -- the same review found that this
/// function's own exhaustive match had no mechanism keeping
/// `handle_editor_key`'s separate, hand-written exclusion in agreement
/// with it, and a fix that updated only one of the two (which is
/// exactly what happened earlier in that response) would have silently
/// reintroduced a hidden document absorbing keystrokes the next time a
/// dormant surface gets a real render arm. One exhaustive predicate,
/// not two lists that can drift apart.
fn content_mode_view(state: &State) -> Element<'_, Message> {
    let open_surface = state
        .app_shell
        .state()
        .active_project()
        .map(tekstide_core::project::ProjectSession::open_surface);
    match open_surface {
        // RFC-032: the second real `open_surface`-conditional arm, after
        // `ApprovalHistory` -- `surface_renders_editor` still decides
        // *whether* a surface is one of these (response 235's one
        // predicate, unchanged); this inner match decides *which* one.
        Some(ProjectOpenSurface::TrustSettings) => trust_settings_view(state),
        // PR-020-B report surface: its own explicit arm, the same shape
        // `TrustSettings` above already uses, rather than falling into
        // the generic `!surface_renders_editor` arm below -- that arm
        // is `approval_history_view`'s own, not a dispatch over which
        // non-editor surface applies, so a second surface joining that
        // classification would have silently rendered as the wrong one.
        Some(ProjectOpenSurface::AgentRunDetail) => agent_run_detail_view(state),
        Some(surface) if !surface_renders_editor(surface) => approval_history_view(state),
        Some(_) | None => content_mode_editor_view(state),
    }
}

/// Response 235: the one predicate deciding whether a given
/// `ProjectOpenSurface` renders as the plain editor -- used here by
/// [`content_mode_view`] to pick its render arm, and by
/// [`handle_editor_key`] to decide whether a keystroke should be
/// absorbed. Before this response the two questions were answered by
/// two separate, hand-written lists that had no mechanism keeping them
/// in agreement: `content_mode_view`'s own exhaustive match decided
/// what renders, `handle_editor_key`'s exclusion decided what absorbs
/// keys, and the fix earlier in this response updated only the second
/// -- exactly the shape that would silently reintroduce a hidden
/// document absorbing keystrokes the moment a future surface (RFC-020's
/// `AgentRunDetail`, say) gets a real render arm here without someone
/// remembering to also touch the other function. One exhaustive match,
/// not two: a ninth `ProjectOpenSurface` variant fails to compile right
/// here until someone decides which side of this predicate it falls on,
/// and both call sites inherit that decision automatically rather than
/// needing their own separate update.
fn surface_renders_editor(surface: ProjectOpenSurface) -> bool {
    match surface {
        // PR-020-B report surface: classified `false`, the same side as
        // `ApprovalHistory` -- a pure read-only report with no
        // interactive elements of its own, so a document left open in
        // the background must not keep absorbing keystrokes underneath
        // it. Not `TrustSettings`'s side: that surface has real Enter-
        // driven actions of its own reachable while a background
        // document is still technically "open," this one has none.
        ProjectOpenSurface::ApprovalHistory | ProjectOpenSurface::AgentRunDetail => false,
        ProjectOpenSurface::ProjectDashboard
        | ProjectOpenSurface::TextEditor
        | ProjectOpenSurface::GitStatus
        | ProjectOpenSurface::DiffReview
        | ProjectOpenSurface::HandoffReport
        | ProjectOpenSurface::TrustSettings => true,
    }
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

    // Terminal resize handoff (response 242): the split decision now
    // reads the same computed width `apply_terminal_geometry` uses to
    // drive the real resize, rather than a second, `responsive`-measured
    // width of its own -- one formula, not two that could disagree about
    // whether a two-pane split fits. Falls back to `Wide` (both panes
    // shown) when `state.window_size` is still `None` (before the first
    // `WindowResized` event) -- the launch-time default already renders
    // this way (see `main`'s initial window request), and this only
    // matters for the handful of frames before that first event.
    let panes_view: Element<'_, Message> = if visible_panes.is_empty() {
        column![].into()
    } else {
        let class = terminal_workspace_content_size(state)
            .map(|(panes_width, _panes_height)| {
                crate::surface::terminal::layout_class_for(panes_width, font_size)
            })
            .unwrap_or(tekstide_core::navigation::TerminalLayoutClass::Wide);
        let shown: Vec<&crate::surface::terminal::TerminalPane> = match class {
            tekstide_core::navigation::TerminalLayoutClass::Wide => visible_panes,
            tekstide_core::navigation::TerminalLayoutClass::Narrow => {
                visible_panes.into_iter().take(1).collect()
            }
        };
        row(shown
            .into_iter()
            .map(|pane| crate::surface::terminal::view(pane, font_size))
            .collect::<Vec<Element<'_, Message>>>())
        .spacing(8)
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

/// Width of the binding column, so descriptions line up -- the same
/// number and reasoning `board.rs`'s own `KEYBOARD_HELP_BINDING_COLUMN_PX`
/// used before this list moved here (RFC-038 PR-038-C, RFC-039's second
/// principle: reference material does not live on a working surface).
const HELP_MODAL_BINDING_COLUMN_PX: f32 = 110.0;

/// RFC-038 PR-038-C: the keyboard reference, reachable from anywhere via
/// `Ctrl+Alt+K` -- replaces the Project Board's own former keyboard list
/// (`board.rs`'s `keyboard_help_view`, deleted this slice). Derives from
/// `keyboard_help::keyboard_help_lines`, the one place `KeybindingPolicy`
/// becomes user-facing text -- not a second, hand-written list. No
/// buttons: `ModalFocusNext`/`ModalFocusPrevious`/`ModalActivate` are all
/// no-ops against `ModalContent::Help` (see that variant's own doc);
/// only `Escape` does anything, and `ModalDismiss`'s handler is already
/// generic.
fn help_modal_view(state: &State) -> Element<'_, Message> {
    let mut lines = column![
        text(state.catalog.get("help-dialog-title")).size(state.theme.font_size_heading()),
    ]
    .spacing(6);

    // `binding` is a `&'static str` from the policy (trusted, fixed-set,
    // not filesystem-derived); `description` comes from the catalog --
    // the same "neither is untrusted" reasoning `board.rs`'s own
    // (now-deleted) `keyboard_help_view` stated for this exact loop.
    for line in crate::keyboard_help::keyboard_help_lines(&state.catalog) {
        lines = lines.push(
            row![
                text(line.binding)
                    .size(state.theme.font_size_body())
                    .width(Length::Fixed(HELP_MODAL_BINDING_COLUMN_PX)),
                text(line.description).size(state.theme.font_size_body()),
            ]
            .spacing(8),
        );
    }

    lines = lines
        .push(text(state.catalog.get("help-dialog-hint")).size(state.theme.font_size_status()));

    modal_dialog_box(state, lines.into())
}

/// RFC-038 PR-038-G: the folder browser's own modal chrome (title,
/// notices, hint) around [`crate::surface::explorer::browse_view`]'s
/// rendering of the scan itself -- the same "surface renders the data,
/// `shell.rs` composes the modal around it" split [`help_modal_view`]
/// already uses for `keyboard_help_lines`.
fn folder_browser_modal_view<'a>(
    state: &'a State,
    modal: &'a FolderBrowserModal,
) -> Element<'a, Message> {
    let mut lines = column![
        text(state.catalog.get("browse-dialog-title")).size(state.theme.font_size_heading()),
    ]
    .spacing(6);

    lines = lines.push(crate::surface::explorer::browse_view(
        &modal.scan,
        modal.highlight,
        &state.catalog,
        &state.theme,
    ));

    if modal.navigate_failed {
        lines = lines.push(
            text(state.catalog.get("browse-navigate-error")).size(state.theme.font_size_body()),
        );
    }
    // `path_field_error_text` reused as-is (see its own doc comment):
    // the same `add_project_from_path` call, the same failure shapes,
    // the same bound-then-escape discipline -- `modal.scan.current_dir`
    // is the exact path that was just handed to it.
    if let Some(error) = modal.open_error {
        let notice = path_field_error_text(
            &state.catalog,
            &modal.scan.current_dir.display().to_string(),
            error,
        );
        lines = lines.push(text(notice).size(state.theme.font_size_body()));
    }

    lines = lines
        .push(text(state.catalog.get("browse-dialog-hint")).size(state.theme.font_size_status()));

    modal_dialog_box(state, lines.into())
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

/// RFC-022 PR-022-E ("the arrival model"): registers a freshly bound
/// approval channel for polling -- the one production call site for
/// `ApprovalChannelEndpoint::serve_concurrently` (response 227's found
/// defect: the endpoint used to be silently dropped before anything
/// could call this). `serve_concurrently`'s accept loop holds only a
/// `Weak` reference internally (see that method's own doc comment in
/// `tekstide-core`) and explicitly drops the strong `Arc` it is handed --
/// a caller-retained clone is required for the endpoint, and therefore
/// the bound socket, to stay alive at all. An earlier version of this
/// function passed a bare temporary `Arc::new(endpoint)` straight into
/// `serve_concurrently` and kept no clone, so the strong count hit zero
/// the instant this function returned: `ApprovalChannelEndpoint::drop`
/// ran immediately, removing the real socket special file and closing
/// the listener before the accept-loop thread's first `accept()` call, a
/// real adapter's `connect()` losing the resulting race every time
/// (`ENOENT`). `endpoint` below is that missing caller-retained clone,
/// stored on `ApprovalChannelServing` for the whole serving lifetime;
/// dropping the returned `ServeShutdown` is what tears the accept loop --
/// and eventually this `Arc`'s strong count -- down.
fn register_approval_channel(
    state: &mut State,
    project_id: tekstide_core::project::ProjectId,
    agent_run_id: tekstide_core::domain::AgentRunId,
    verified_cwd: tekstide_core::agent::VerifiedCwd,
    project_root: std::path::PathBuf,
    state_root: std::path::PathBuf,
    endpoint: tekstide_core::approval::ApprovalChannelEndpoint,
) {
    let endpoint = std::sync::Arc::new(endpoint);
    // The clone handed to `serve_concurrently` is consumed and downgraded
    // to a `Weak` internally; `endpoint` itself is the caller-retained
    // strong reference `serve_concurrently`'s contract requires, stored
    // below on `ApprovalChannelServing` for the serving's whole lifetime.
    let (receiver, shutdown) = std::sync::Arc::clone(&endpoint).serve_concurrently();
    state.approval_channels.push(ApprovalChannelServing {
        project_id,
        agent_run_id,
        verified_cwd,
        project_root,
        state_root,
        receiver,
        endpoint,
        shutdown,
    });
}

/// RFC-022 PR-022-E ("the arrival model"): `Message::ApprovalPollTick`'s
/// handler -- drains every open channel's receiver (non-blocking;
/// `try_recv` never waits for a proposal that has not arrived), feeds
/// each accepted proposal through the real coordinator, mirrors the
/// result into the owning `ProjectSession`, sweeps every still-`Pending`
/// request for expiry, and finally re-evaluates promotion once (an
/// arrival is exactly the "point-in-time predicate" case
/// `should_promote_to_modal` was always meant to answer, not only the
/// modal-close/project-switch re-evaluation cases response 227 added).
fn poll_approval_channels(state: &mut State) {
    // RFC-022 PR-022-E: `.retain` would drop a serving the moment its
    // receiver reports `Disconnected` -- but a disconnected receiver
    // (the adapter's own listen loop exited, or `ServeShutdown` fired)
    // says nothing about whether *already-accepted* proposals on that
    // run are still individually live; each one answers that for itself
    // via `AcceptedProposal::is_connection_still_open`, checked in the
    // expiry sweep below. So a disconnected serving is only dropped from
    // this list (stops being polled for *new* proposals); it does not
    // touch anything already in the coordinator's own map.
    let mut still_open = Vec::with_capacity(state.approval_channels.len());
    for serving in std::mem::take(&mut state.approval_channels) {
        let mut accepted_this_tick = Vec::new();
        loop {
            match serving.receiver.try_recv() {
                Ok(Ok(accepted)) => accepted_this_tick.push(accepted),
                // A connection that failed authentication -- already
                // logged/rejected server-side (`accept_proposal`'s own
                // fail-closed-without-a-dialog design); nothing further
                // for this GUI to do with it.
                Ok(Err(_channel_error)) => {}
                Err(
                    std::sync::mpsc::TryRecvError::Empty
                    | std::sync::mpsc::TryRecvError::Disconnected,
                ) => {
                    break;
                }
            }
        }
        let disconnected = matches!(
            serving.receiver.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected)
        );
        for accepted in accepted_this_tick {
            receive_approval_proposal(state, &serving, accepted);
        }
        if !disconnected {
            still_open.push(serving);
        }
    }
    state.approval_channels = still_open;

    sweep_expired_approvals(state);
    evaluate_promotion(state);
}

/// RFC-022 PR-022-E: feeds one accepted proposal through the real
/// `ApprovalCoordinator`, mirroring a `Created` result into the owning
/// `ProjectSession` (`add_approval_request`) and recording the
/// `ApprovalId` -> `ProposalId` bridge (`ApprovalDialog`'s own doc
/// comment explains why that bridge has to live here). `DuplicateRejected`/
/// `QueueLimitExceeded` need no mirroring: the coordinator already
/// dropped the connection and created nothing to mirror. `verified_cwd`/
/// `project_root`/`state_root` come from `serving` -- captured once, at
/// launch time (`register_approval_channel`), since `VerifiedCwd` has no
/// public constructor accepting an arbitrary path (the only way to
/// obtain one is `AgentRunLaunchValidator::validate`, which runs once,
/// not on every proposal a long-lived agent run's adapter later sends).
fn receive_approval_proposal(
    state: &mut State,
    serving: &ApprovalChannelServing,
    accepted: tekstide_core::approval::AcceptedProposal,
) {
    let Some(project) = state
        .app_shell
        .state()
        .project(&serving.project_id)
        .cloned()
    else {
        return;
    };
    let limits = tekstide_core::approval::ApprovalQueueLimits {
        per_agent_run: project.resource_limits().agent_run_approval_limit,
        per_project: project.resource_limits().approval_request_limit,
    };
    let Some(mut audit_store) = open_real_audit_store(&state.app_shell) else {
        // RFC-022 PR-022-E: `receive_proposal` requires a real
        // `AuditCoordinator` to call at all -- the same "no store, no
        // action" degraded mode `decide_approval` also accepts, for the
        // same reason. This specific proposal is lost, not retried
        // (`try_recv` already removed it from the channel before this
        // function was called) -- rare (state-dir-unavailable) and
        // already an accepted degraded mode elsewhere in this crate; not
        // solved further in this slice.
        return;
    };
    let mut audit_health = tekstide_core::audit::AuditHealth::default();
    let mut audit =
        tekstide_core::audit::AuditCoordinator::new(&mut audit_store, &mut audit_health);
    let proposal_id = accepted.proposal.proposal_id().clone();
    let outcome = state.approval_coordinator.receive_proposal(
        serving.project_id.clone(),
        serving.agent_run_id.clone(),
        &serving.verified_cwd,
        &serving.project_root,
        &serving.state_root,
        accepted,
        limits,
        &mut audit,
    );
    let tekstide_core::approval::ReceiveOutcome::Created { request, .. } = outcome else {
        return;
    };
    let approval_id = request.id.clone();
    state.approval_proposal_ids.insert(approval_id, proposal_id);
    if let Some(project) = state.app_shell.state_mut().project_mut(&serving.project_id)
        && let Ok(Some(evicted_id)) = project.add_approval_request(*request)
    {
        // Response 228 Required 2: `approval_history_limit` eviction just
        // removed `evicted_id` from `ProjectSession` to make room for the
        // one just inserted above -- the bridge entry for it is now
        // unreachable from anywhere real (nothing decides or sweeps an
        // entry that no longer exists), so it is pruned here rather than
        // left to grow unbounded for the life of the session.
        state.approval_proposal_ids.remove(&evicted_id);
    }
}

/// RFC-022 PR-022-E ("the arrival model"): every still-`Pending`,
/// not-yet-marked-expired request across every open project is checked
/// against the real coordinator (`is_still_answerable` -- the
/// authoritative liveness check `AcceptedProposal::is_connection_still_open`
/// backs) and marked expired the moment its connection is found closed.
/// This is what keeps "visibly unanswerable" honest: without this sweep,
/// a request could sit `Pending`, its adapter long gone, indistinguishable
/// from one still genuinely awaiting a decision.
fn sweep_expired_approvals(state: &mut State) {
    let project_ids: Vec<_> = state
        .app_shell
        .state()
        .projects()
        .iter()
        .map(|project| project.id().clone())
        .collect();
    for project_id in project_ids {
        let Some(project) = state.app_shell.state().project(&project_id) else {
            continue;
        };
        let newly_expired: Vec<_> = project
            .approval_requests()
            .iter()
            .filter(|request| {
                request.decision == tekstide_core::domain::ApprovalDecision::Pending
                    && !project.expired_approval_ids().contains(&request.id)
            })
            .filter_map(|request| {
                let agent_run_id = request.agent_run_id.clone()?;
                let proposal_id = state.approval_proposal_ids.get(&request.id)?.clone();
                Some((request.id.clone(), agent_run_id, proposal_id))
            })
            .filter(|(_, agent_run_id, proposal_id)| {
                !state
                    .approval_coordinator
                    .is_still_answerable(agent_run_id, proposal_id)
            })
            .map(|(approval_id, ..)| approval_id)
            .collect();
        if newly_expired.is_empty() {
            continue;
        }
        if let Some(project) = state.app_shell.state_mut().project_mut(&project_id) {
            for approval_id in newly_expired {
                let _ = project.mark_approval_expired(&approval_id);
            }
        }
    }
}

/// RFC-022 PR-022-E ("the arrival model"), response 227: **not only
/// called on arrival.** The promotion decision
/// (`approval::should_promote_to_modal`) is a point-in-time predicate,
/// and response 227's own correction was that a point-in-time check
/// alone is incomplete -- a `Destructive` proposal arriving while a
/// *different* modal is open must not be silently downgraded to a
/// `Low`-equivalent "never promotes" outcome just because of arrival
/// timing. So this is called from every place that can flip the
/// predicate from `false` to `true`: a new arrival (`poll_approval_channels`),
/// a modal closing (`ModalActivate`/`ModalDismiss`), and -- **not yet
/// wired, disclosed rather than silently skipped**: an active-project
/// change, since nothing in the shipped GUI currently switches which
/// project is active during a session at all (`AppState::switch_active_project`
/// has no production caller anywhere in this crate; `NavigationAction::SwitchActiveProject`
/// itself maps to no `AppCommand`). The re-evaluation logic below is
/// unconditionally correct regardless of *why* it was called, so wiring
/// a real call site once project-switching exists elsewhere is a one-line
/// addition, not a design question.
///
/// **Oldest qualifying proposal first** (response 227): `approval_requests()`
/// is already insertion-ordered (push-only), so the first entry this
/// scan finds satisfying every guard *is* the oldest.
fn evaluate_promotion(state: &mut State) {
    if state.modal.is_some() {
        return;
    }
    let Some(project) = state.app_shell.state().active_project() else {
        return;
    };
    let Some(candidate) = project
        .approval_requests()
        .iter()
        .find(|request| {
            request.decision == tekstide_core::domain::ApprovalDecision::Pending
                && !project.expired_approval_ids().contains(&request.id)
                && tekstide_core::approval::should_promote_to_modal(request.risk_level, false, true)
        })
        .cloned()
    else {
        return;
    };
    let Some(agent_run_id) = candidate.agent_run_id.clone() else {
        return;
    };
    let Some(proposal_id) = state.approval_proposal_ids.get(&candidate.id).cloned() else {
        return;
    };
    // Re-confirmed against the real, authoritative coordinator rather
    // than trusting `expired_approval_ids` alone -- that set is only as
    // fresh as the last `sweep_expired_approvals` tick, and promoting a
    // request whose adapter gave up moments ago (before the next sweep)
    // would put a dead request in the one place this whole design exists
    // to make trustworthy.
    if !state
        .approval_coordinator
        .is_still_answerable(&agent_run_id, &proposal_id)
    {
        return;
    }
    state.modal = Some(ModalContent::Approval(Box::new(ApprovalDialog {
        request: candidate,
        proposal_id,
        focus: ApprovalDialogButton::Reject,
        ignore_input_until: Some(std::time::Instant::now() + APPROVAL_DIALOG_INPUT_IGNORE_WINDOW),
    })));
}

/// Response 233: manual open from the `ApprovalHistory` surface --
/// `Message::OpenApprovalHistoryEntry`'s handler. Reuses the exact same
/// `ApprovalDialog` construction [`evaluate_promotion`] uses, but
/// consults none of promotion's own guards
/// (`should_promote_to_modal`'s active-project-only / severity checks):
/// those exist to constrain *automatic interruption* the user did not
/// ask for, and neither reason applies to something the user explicitly
/// opened while already looking at that project's own history -- in
/// fact both are structurally satisfied by that context already (the
/// history surface only ever shows the active project, and a modal
/// covers the surface the user just clicked on).
///
/// **What does still apply, because it is a correctness rule rather
/// than a promotion guard**: never replace an open modal. The user's
/// place in another decision is not this surface's to discard, for the
/// same reason an arriving proposal must not either -- checked first,
/// same as `evaluate_promotion`'s own first line.
fn open_approval_history_entry(state: &mut State, approval_id: &tekstide_core::domain::ApprovalId) {
    if state.modal.is_some() {
        return;
    }
    let Some(project) = state.app_shell.state().active_project() else {
        return;
    };
    let Some(candidate) = project
        .approval_requests()
        .iter()
        .find(|request| {
            &request.id == approval_id
                && request.decision == tekstide_core::domain::ApprovalDecision::Pending
                && !project.expired_approval_ids().contains(&request.id)
        })
        .cloned()
    else {
        return;
    };
    let Some(agent_run_id) = candidate.agent_run_id.clone() else {
        return;
    };
    let Some(proposal_id) = state.approval_proposal_ids.get(&candidate.id).cloned() else {
        return;
    };
    // Re-confirmed against the real, authoritative coordinator rather
    // than trusting `expired_approval_ids` alone, the same reasoning
    // `evaluate_promotion` uses -- that set is only as fresh as the last
    // `sweep_expired_approvals` tick.
    if !state
        .approval_coordinator
        .is_still_answerable(&agent_run_id, &proposal_id)
    {
        return;
    }
    state.modal = Some(ModalContent::Approval(Box::new(ApprovalDialog {
        request: candidate,
        proposal_id,
        focus: ApprovalDialogButton::Reject,
        ignore_input_until: Some(std::time::Instant::now() + APPROVAL_DIALOG_INPUT_IGNORE_WINDOW),
    })));
}

/// RFC-022 PR-022-E: `ModalActivate`'s handler for a promoted approval
/// dialog -- sends the real decision through the real coordinator (the
/// same `decide`/`decide_with_edited_argv` "no decision recorded for an
/// undeliverable proposal" guard applies here exactly as it does
/// anywhere else `decide` is called) and mirrors the result into the
/// owning `ProjectSession` so `pending_approvals` reflects it
/// immediately.
fn decide_approval(
    state: &mut State,
    dialog: ApprovalDialog,
    decision: tekstide_core::approval::SimpleDecision,
) {
    let Some(agent_run_id) = dialog.request.agent_run_id.clone() else {
        return;
    };
    let Some(mut audit_store) = open_real_audit_store(&state.app_shell) else {
        // RFC-022 PR-022-E: `decide` requires a real `AuditCoordinator`
        // to call at all -- `ApprovedOnce`'s own fail-closed authorization
        // needs one to fail closed *against*. Without a real store, this
        // decision cannot be recorded either way, so it is left exactly
        // as it was: `Pending`. Rare (state-dir-unavailable) and already
        // an accepted degraded mode elsewhere in this crate.
        return;
    };
    let mut audit_health = tekstide_core::audit::AuditHealth::default();
    let mut audit =
        tekstide_core::audit::AuditCoordinator::new(&mut audit_store, &mut audit_health);
    let outcome =
        state
            .approval_coordinator
            .decide(&agent_run_id, &dialog.proposal_id, decision, &mut audit);
    let tekstide_core::approval::DecideOutcome::Decided { request, .. } = outcome else {
        // `Undeliverable`/`AlreadyDecided`/`NotFound`/`AuditBlocked`:
        // the stored request's own decision (still `Pending`) is
        // already the honest state for all four -- nothing to mirror.
        return;
    };
    let project_id = dialog.request.project_id.clone();
    // Response 228 Required 2: a `Decided` outcome is always a real,
    // final decision (`decide` only reaches this arm by actually
    // recording one) -- nothing will ever look up this `ApprovalId`'s
    // `ProposalId` again (`sweep_expired_approvals` only checks still-
    // `Pending` requests), so the bridge entry is pruned here rather
    // than left to outlive its own usefulness for the rest of the
    // session.
    state.approval_proposal_ids.remove(&dialog.request.id);
    if let Some(project) = state.app_shell.state_mut().project_mut(&project_id) {
        let _ = project.replace_approval_request(request);
    }
}

/// RFC-032: manual open from the `TrustSettings` surface --
/// `Message::OpenTrustGrantDialog`'s handler. The same "never replace an
/// open modal" correctness rule `open_approval_history_entry` checks
/// first, for the same reason: the user's place in another decision is
/// not this surface's to discard. Captures the active project's paths
/// at open time (`TrustGrantModal`'s own doc explains why), and defaults
/// focus to `Cancel` -- `what-the-trust-dialog-must-say.md` §2, the
/// larger asymmetry than the paste dialog's own default-to-Reject.
fn open_trust_grant_dialog(state: &mut State) {
    if state.modal.is_some() {
        return;
    }
    let Some(project) = state.app_shell.state().active_project() else {
        return;
    };
    state.modal = Some(ModalContent::TrustGrant(TrustGrantModal {
        project_id: project.id().clone(),
        root_path: project.root_path().clone(),
        canonical_root_path: project.canonical_root_path().clone(),
        focus: TrustGrantButton::Cancel,
    }));
}

/// RFC-032: `ModalActivate`'s handler when a trust-grant dialog's focus
/// is on `Grant` -- the real, audited grant, through
/// `AuditCoordinator::grant_project_trust`'s first production caller.
/// Re-resolves the project by `modal.project_id` rather than assuming
/// the active project is still the same one the dialog was opened
/// against (nothing currently lets the active project change while a
/// modal is open -- modal exclusivity precludes it -- but this does not
/// rely on that holding, the same defensive-lookup shape `decide_approval`
/// already uses for its own project id). A project that has since closed
/// (or the audit store being unavailable) is a silent no-op: there is
/// nothing left to grant trust *to*, matching `decide_approval`'s own
/// "cannot record either way, leave state as it was" precedent.
fn apply_workspace_trust_grant(state: &mut State, modal: &TrustGrantModal) {
    let Some(mut audit_store) = open_real_audit_store(&state.app_shell) else {
        return;
    };
    let mut audit_health = tekstide_core::audit::AuditHealth::default();
    let mut audit =
        tekstide_core::audit::AuditCoordinator::new(&mut audit_store, &mut audit_health);
    if let Some(project) = state.app_shell.state_mut().project_mut(&modal.project_id) {
        let _ = audit.grant_project_trust(project);
    }
}

/// RFC-032: `Message::RevokeWorkspaceTrust`'s handler -- the direct,
/// undialogued path `what-the-trust-dialog-must-say.md` §5 requires for
/// revocation, reached from the same `TrustSettings` surface granting is
/// (comparable navigation depth; the *action* itself is deliberately
/// simpler than granting's two-deliberate-acts dialog, since revoking is
/// the safe direction). `AuditCoordinator::revoke_project_trust`'s first
/// production caller.
fn revoke_workspace_trust(state: &mut State) {
    let Some(project_id) = state
        .app_shell
        .state()
        .active_project()
        .map(|project| project.id().clone())
    else {
        return;
    };
    let Some(mut audit_store) = open_real_audit_store(&state.app_shell) else {
        return;
    };
    let mut audit_health = tekstide_core::audit::AuditHealth::default();
    let mut audit =
        tekstide_core::audit::AuditCoordinator::new(&mut audit_store, &mut audit_health);
    if let Some(project) = state.app_shell.state_mut().project_mut(&project_id) {
        let _ = audit.revoke_project_trust(project);
    }
}

/// RFC-033 PR-033-B: `Message::ToggleTranscriptCaptureDeclined`'s
/// handler. No audit store, unlike `revoke_workspace_trust` right
/// above -- `ProjectSession::set_transcript_capture_declined`'s own doc
/// comment states why: the task breakdown names no audit producer for
/// this toggle, only for `transcript_purge` (PR-033-D), and this is the
/// safe direction the same way revocation is.
fn toggle_transcript_capture_declined(state: &mut State) {
    let Some(project_id) = state
        .app_shell
        .state()
        .active_project()
        .map(|project| project.id().clone())
    else {
        return;
    };
    let Some(project) = state.app_shell.state_mut().project_mut(&project_id) else {
        return;
    };
    let declined = project.transcript_capture_declined();
    project.set_transcript_capture_declined(!declined);
}

/// RFC-033 PR-033-C: `TranscriptLocalDataSummary`'s first real caller on
/// this surface (the task breakdown's own description of the gap:
/// "exists and has no caller"). Built from
/// [`tekstide_core::project::ProjectSession::real_retained_transcript_bytes`]
/// / [`tekstide_core::app::AppState::app_wide_retained_transcript_bytes`],
/// **not** `ProjectSession::transcript_local_data_summary` -- that
/// method's own doc comment explains why its `byte_count`-based sum is
/// stale for every real run today, and this surface is read by a user
/// deciding whether to purge, the one place that staleness cannot be
/// silently inherited. `app_retained_bytes`'s own doc states its
/// "currently open projects only" limitation; that limitation
/// propagates into `budget_pressure` here (an app-wide figure computed
/// from an incomplete sum), but `project_retained_bytes` (now real,
/// on-disk bytes) and `project_transcript_count` are exact for the
/// project passed in -- its own `transcripts` list is the complete,
/// authoritative record of which transcripts exist, not a scan of
/// anything that could be missing entries.
fn transcript_local_data_summary_for(
    state: &State,
    project: &tekstide_core::project::ProjectSession,
) -> tekstide_core::transcript::TranscriptLocalDataSummary {
    let app_retained_bytes = state.app_shell.state().app_wide_retained_transcript_bytes();
    tekstide_core::transcript::TranscriptLocalDataSummary::new(
        project.real_retained_transcript_bytes(),
        app_retained_bytes,
        project.transcripts().len() as u64,
        tekstide_core::transcript::TranscriptRetentionLimits::agent_run_default(),
    )
}

/// RFC-033 PR-033-C: `Message::OpenTranscriptPurgeDialog`'s handler --
/// the same "never replace an open modal" rule `open_trust_grant_dialog`
/// checks first, for the same reason. Captures the active project's
/// current retained-transcript count/bytes at open time
/// (`TranscriptPurgeModal`'s own doc explains why), and defaults focus
/// to `Cancel` -- `what-purge-must-remove.md`'s own required framing:
/// deleting is irreversible, so the safe default is not deleting.
fn open_transcript_purge_dialog(state: &mut State) {
    if state.modal.is_some() {
        return;
    }
    let Some(project) = state.app_shell.state().active_project() else {
        return;
    };
    let summary = transcript_local_data_summary_for(state, project);
    state.modal = Some(ModalContent::TranscriptPurge(TranscriptPurgeModal {
        project_id: project.id().clone(),
        transcript_count: summary.project_transcript_count,
        retained_bytes: summary.project_retained_bytes,
        focus: TranscriptPurgeButton::Cancel,
    }));
}

/// RFC-033 PR-033-D, response 279's required fix: `ModalActivate`'s
/// handler when a purge-confirmation dialog's focus is on `Purge` --
/// the real deletion, through `AuditCoordinator::purge_project_transcripts`
/// when the audit store is available, and through
/// `ProjectSession::purge_project_transcripts` directly when it is not
/// (PR-033-C's own original wiring, for that one path). Re-resolves the
/// project by `modal.project_id` rather than assuming the active
/// project is unchanged, the same defensive-lookup shape
/// `apply_workspace_trust_grant`/`revoke_workspace_trust` already use.
///
/// **The purge itself is never gated on the audit store opening.**
/// Response 279 corrected an earlier version of this function that
/// treated `open_real_audit_store` failing as a silent no-op for the
/// whole action, mirroring `revoke_workspace_trust`'s own precedent.
/// That precedent does not transfer: "this cannot be undone" (the
/// confirmation's own wording) describes the *deletion*, not the
/// record, so refusing to delete when the record can't be written does
/// not weaken that promise -- it leaves it unfulfilled, silently, after
/// the user deliberately moved focus off `Cancel` and activated twice.
/// There is also no accountability property being protected here the
/// way there might be for a third-party-facing record: these are the
/// user's own local transcripts and the audit store is local too --
/// anyone able to prevent the store opening could delete the
/// transcripts directly anyway, so refusing the deletion buys nothing
/// and costs the user the thing they asked for. `revoke_workspace_trust`'s
/// own refusal is milder for a reason that does not apply here: trust
/// state is rendered on the same surface, so a silently-failed revoke
/// at least leaves a visible, contradicting "Trusted" label -- deleted
/// bytes have no equivalent tell either way. Not this function's place
/// to fix that one; recorded here only so the asymmetry is not mistaken
/// for a rule.
///
/// **Recording the purge, once the store is open, stays best-effort**
/// (`AuditCoordinator::purge_project_transcripts`'s own
/// `append_observation`) for the same reason as before: the deletion
/// has already happened on the real filesystem by the time the record
/// is built, so a transient write failure for the record alone cannot
/// and does not roll the deletion back. This function now treats
/// "store did not open" and "store opened but the one write failed" as
/// the same case from the deletion's point of view -- delete regardless,
/// record only if and however well the store currently allows.
fn apply_transcript_purge(state: &mut State, modal: &TranscriptPurgeModal) {
    match open_real_audit_store(&state.app_shell) {
        Some(mut audit_store) => {
            let mut audit_health = tekstide_core::audit::AuditHealth::default();
            let mut audit =
                tekstide_core::audit::AuditCoordinator::new(&mut audit_store, &mut audit_health);
            if let Some(project) = state.app_shell.state_mut().project_mut(&modal.project_id) {
                let _ = audit.purge_project_transcripts(project);
            }
        }
        None => {
            if let Some(project) = state.app_shell.state_mut().project_mut(&modal.project_id) {
                let _ = project.purge_project_transcripts();
            }
        }
    }
}

/// RFC-022 PR-022-E: a compile-time literal symbol for `RiskLevel`, the
/// same `trusted_symbol` division of labour every other symbol-driven
/// Fluent lookup in this file uses -- the words live in `en.ftl`'s
/// `approval-dialog-risk` select expression, not here. `RiskLevel` is
/// Tekstide's own classification output (`approval::risk::classify`).
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

/// Called from `view()` as of response 220/227's "the arrival model" --
/// `ModalContent::Approval`'s own render arm.
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

/// Response 233: `ProjectOpenSurface::ApprovalHistory`'s real content --
/// every retained `ApprovalRequest` for the active project, decided and
/// expired included (response 231: the constraint is *visibly*
/// unanswerable, and a live-only view would have nothing to be visibly
/// anything). Both non-optional disclosures (retention limit,
/// classifier limitation) render above the list, always, not only when
/// the list is non-empty -- a user who has never seen an expired entry
/// yet still needs to know risk level is inferred, not guaranteed.
///
/// **No bulk approval, no multi-select** (RFC-022's own explicit
/// constraint): each still-live entry gets its own, individual open
/// control; nothing here selects more than one at a time or exposes a
/// "decide all" action of any kind.
/// RFC-032: `ProjectOpenSurface::TrustSettings`'s real view -- the
/// active project's current trust state, and the one control that
/// actually changes it: "Grant" (opens the confirmation dialog) when not
/// `Trusted`, "Revoke" (direct, no dialog) when it is. Never both at
/// once -- there is nothing to grant while already trusted, and nothing
/// to revoke while not.
fn trust_settings_view(state: &State) -> Element<'_, Message> {
    let Some(project) = state.app_shell.state().active_project() else {
        // Unreachable while routed to `ActiveProjectWorkspace`, the same
        // "fail visible, not panic" fallback every other surface here
        // uses for this case.
        return text(state.catalog.get("trust-settings-empty"))
            .size(state.theme.font_size_body())
            .into();
    };

    let trust_state = project.trust_state();
    let is_trusted = trust_state == tekstide_core::project::WorkspaceTrust::Trusted;

    let mut lines: Vec<Element<'_, Message>> = vec![
        text(state.catalog.get("trust-settings-heading"))
            .size(state.theme.font_size_heading())
            .into(),
        text(state.catalog.get_with_args(
            "trust-settings-current-state",
            &CatalogArgs::new().trusted_symbol("state", trust_state_symbol(trust_state)),
        ))
        .size(state.theme.font_size_body())
        .into(),
    ];

    if is_trusted {
        lines.push(
            button(
                text(state.catalog.get("trust-settings-revoke-button"))
                    .size(state.theme.font_size_body()),
            )
            .on_press(Message::RevokeWorkspaceTrust)
            .into(),
        );
    } else {
        lines.push(
            button(
                text(state.catalog.get("trust-settings-grant-button"))
                    .size(state.theme.font_size_body()),
            )
            .on_press(Message::OpenTrustGrantDialog)
            .into(),
        );
    }

    // RFC-033 PR-033-B: always rendered, regardless of `is_trusted` --
    // unlike the trust action above, capture is a per-project
    // preference independent of trust state, not something that only
    // makes sense in one of two mutually exclusive states.
    // `capture_declined_symbol`'s own doc comment explains why this
    // reuses `trusted_symbol` despite the name.
    let capture_declined = project.transcript_capture_declined();
    lines.push(
        text(state.catalog.get_with_args(
            "trust-settings-capture-current-state",
            &CatalogArgs::new().trusted_symbol("state", capture_declined_symbol(capture_declined)),
        ))
        .size(state.theme.font_size_body())
        .into(),
    );
    lines.push(
        button(
            text(state.catalog.get(if capture_declined {
                "trust-settings-capture-allow-button"
            } else {
                "trust-settings-capture-decline-button"
            }))
            .size(state.theme.font_size_body()),
        )
        .on_press(Message::ToggleTranscriptCaptureDeclined)
        .into(),
    );

    // RFC-033 PR-033-C: `transcript_local_data_summary`'s real caller --
    // a user deciding whether to purge needs to see what is retained
    // first, per the task breakdown. Always rendered, the same
    // "independent of trust state" reasoning the capture row above
    // already applies -- what is retained does not depend on whether
    // the project is currently trusted.
    let summary = transcript_local_data_summary_for(state, project);
    lines.push(
        text(
            state.catalog.get_with_args(
                "trust-settings-retained-transcripts",
                &CatalogArgs::new()
                    .number("count", summary.project_transcript_count)
                    .number("bytes", summary.project_retained_bytes),
            ),
        )
        .size(state.theme.font_size_body())
        .into(),
    );
    lines.push(
        button(
            text(state.catalog.get("trust-settings-purge-button"))
                .size(state.theme.font_size_body()),
        )
        .on_press(Message::OpenTranscriptPurgeDialog)
        .into(),
    );

    column(lines).spacing(12).into()
}

/// RFC-033 PR-033-B: a compile-time literal symbol for the capture
/// opt-out, the same `trusted_symbol` division of labour
/// `trust_state_symbol`/`risk_level_symbol` already use -- the words
/// live in `en.ftl`'s `trust-settings-capture-current-state` select
/// expression, not here.
fn capture_declined_symbol(declined: bool) -> &'static str {
    if declined { "declined" } else { "enabled" }
}

/// A compile-time literal symbol for `WorkspaceTrust`, the same
/// `trusted_symbol` division of labour `risk_level_symbol` already uses
/// -- the words live in `en.ftl`'s `trust-settings-current-state` select
/// expression, not here.
fn trust_state_symbol(trust: tekstide_core::project::WorkspaceTrust) -> &'static str {
    use tekstide_core::project::WorkspaceTrust;
    match trust {
        WorkspaceTrust::Unknown => "unknown",
        WorkspaceTrust::Restricted => "restricted",
        WorkspaceTrust::Trusted => "trusted",
        WorkspaceTrust::Revoked => "revoked",
    }
}

fn approval_history_view(state: &State) -> Element<'_, Message> {
    let Some(project) = state.app_shell.state().active_project() else {
        // Unreachable while routed to `ActiveProjectWorkspace` (core
        // guards every transition into this route on an active project
        // existing) -- the same "fail visible, not panic" fallback
        // `main_area_key`'s own doc comment already establishes for the
        // analogous case in `content_mode_editor_view`.
        return text(state.catalog.get("approval-history-empty"))
            .size(state.theme.font_size_body())
            .into();
    };

    let mut lines: Vec<Element<'_, Message>> = vec![
        text(state.catalog.get("approval-history-heading"))
            .size(state.theme.font_size_heading())
            .into(),
        text(state.catalog.get("approval-history-retention-notice"))
            .size(state.theme.font_size_status())
            .into(),
        text(state.catalog.get("approval-history-classifier-notice"))
            .size(state.theme.font_size_status())
            .into(),
    ];

    let requests = project.approval_requests();
    if requests.is_empty() {
        lines.push(
            text(state.catalog.get("approval-history-empty"))
                .size(state.theme.font_size_body())
                .into(),
        );
    } else {
        let highlight = state.approval_history_highlight.min(requests.len() - 1);
        for (index, request) in requests.iter().enumerate() {
            let is_expired = project.expired_approval_ids().contains(&request.id);
            lines.push(approval_history_entry_view(
                state,
                request,
                is_expired,
                index == highlight,
            ));
        }
    }

    scrollable(column(lines).spacing(12))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// One retained request's row. Live (`Pending`, not expired) entries
/// get a real control (`Message::OpenApprovalHistoryEntry`) opening the
/// same decision dialog `evaluate_promotion` would; every other state
/// is plain, unclickable text -- there is nothing left to decide about
/// an already-decided or expired entry, and offering a control that
/// cannot work is the same defect RFC-022 names for expiry itself
/// ("visibly unanswerable, not merely fail when acted on"). `highlighted`
/// marks the row `handle_approval_history_key`'s Up/Down currently sits
/// on, with the same `focus_marker` convention (`"> "`/`"  "`) every
/// other keyboard-navigable list and modal in this crate already uses --
/// a textual channel, not colour alone (`NFR-UX-002`).
fn approval_history_entry_view<'a>(
    state: &'a State,
    request: &'a tekstide_core::domain::ApprovalRequest,
    is_expired: bool,
    highlighted: bool,
) -> Element<'a, Message> {
    let body = text(format!(
        "{}{}",
        focus_marker(highlighted),
        approval_history_entry_body(&state.catalog, request, is_expired)
    ))
    .size(state.theme.font_size_status());
    let is_live = approval_request_is_live(request, is_expired);

    let content: Element<'_, Message> = if is_live {
        column![
            body,
            button(
                text(state.catalog.get("approval-history-entry-open"))
                    .size(state.theme.font_size_status())
            )
            .on_press(Message::OpenApprovalHistoryEntry(request.id.clone())),
        ]
        .spacing(4)
        .into()
    } else {
        column![body].into()
    };

    container(content)
        .width(Length::Fill)
        .padding(8)
        .style(move |_base_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(state.theme.surface_elevated())),
            text_color: Some(state.theme.foreground()),
            border: Border {
                color: state.theme.border_default(),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// Factored out from [`approval_history_entry_view`] so the actual
/// rendered text (escaping, catalog selection) is directly testable
/// without going through `iced`'s `Element` tree -- the same shape
/// `approval_dialog_body`/`surface::board::row_lines` already use.
pub(crate) fn approval_history_entry_body(
    catalog: &Catalog,
    request: &tekstide_core::domain::ApprovalRequest,
    is_expired: bool,
) -> String {
    let command = tekstide_core::text_safety::quote_untrusted(&request.display_command);
    let cwd = tekstide_core::text_safety::quote_untrusted(&request.cwd.display().to_string());
    catalog.get_with_args(
        "approval-history-entry",
        &CatalogArgs::new()
            .untrusted("command", &command)
            .untrusted("cwd", &cwd)
            .trusted_symbol("risk", risk_level_symbol(request.risk_level))
            .trusted_symbol(
                "state",
                approval_history_entry_state_symbol(request, is_expired),
            ),
    )
}

/// A trusted, compile-time symbol for an entry's decision state.
/// `ApprovalDecision::Pending` alone cannot tell a user whether an
/// entry is still answerable, since RFC-022's own design keeps
/// `decision` at `Pending` even after the connection is gone ("expiry
/// is a connection property, not a decision outcome") -- `is_expired`
/// (from `ProjectSession::expired_approval_ids`) is what actually
/// distinguishes the two, the entire reason this surface exists.
fn approval_history_entry_state_symbol(
    request: &tekstide_core::domain::ApprovalRequest,
    is_expired: bool,
) -> &'static str {
    use tekstide_core::domain::ApprovalDecision;
    match request.decision {
        ApprovalDecision::Pending if is_expired => "expired",
        ApprovalDecision::Pending => "answerable",
        ApprovalDecision::ApprovedOnce => "approved",
        ApprovalDecision::Rejected => "rejected",
        ApprovalDecision::EditedAndApproved => "edited-and-approved",
    }
}

/// PR-020-B: why a real key press reaches transcript content the way
/// it does -- "the most recently launched run in the active project"
/// (`pr-020-b-report-surface.md`'s own answer to the pack's unanswered
/// question: matches `OpenCurrentAgentRunDetail`'s own name,
/// `agent_run_limit` bounds how many could exist, and a selector is a
/// second surface with its own navigation decisions this slice is not
/// for). An empty project renders its own message rather than empty
/// chrome, per the handoff's own "zero-reachable-surface rule, one
/// layer in."
fn agent_run_detail_view(state: &State) -> Element<'_, Message> {
    let Some(project) = state.app_shell.state().active_project() else {
        // Unreachable while routed to `ActiveProjectWorkspace`, the same
        // "fail visible, not panic" fallback every other surface here
        // uses for this case.
        return text(state.catalog.get("agent-run-detail-empty"))
            .size(state.theme.font_size_body())
            .into();
    };
    let Some(run) = project.agent_runs().last() else {
        return text(state.catalog.get("agent-run-detail-no-runs"))
            .size(state.theme.font_size_body())
            .into();
    };

    let mut lines: Vec<Element<'_, Message>> = vec![
        text(state.catalog.get("agent-run-detail-heading"))
            .size(state.theme.font_size_heading())
            .into(),
    ];

    match agent_run_transcript_window(project, run) {
        Ok((transcript, window)) => {
            for notice in agent_run_detail_notices(&state.catalog, transcript, &window) {
                lines.push(text(notice).size(state.theme.font_size_status()).into());
            }
            lines.push(
                text(
                    agent_run_detail_transcript_body(window.content())
                        .as_str()
                        .to_string(),
                )
                .size(state.theme.font_size_body())
                .into(),
            );
        }
        Err(reason) => {
            lines.push(
                text(state.catalog.get(agent_run_detail_unavailable_key(reason)))
                    .size(state.theme.font_size_body())
                    .into(),
            );
        }
    }

    scrollable(column(lines).spacing(12))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// PR-020-B: why the transcript for `run` may not be readable -- kept
/// distinct from a generic `bool`/`Option` so [`agent_run_detail_view`]
/// can render an honest reason rather than one flattened "unavailable"
/// message. None of these are expected to fire on the real launch path
/// (a transcript is written for every AI CLI run, per this handoff's
/// own reachability chain) -- they exist because "unreachable in
/// practice" is not "impossible," the same discipline every other
/// best-effort fallback in this module already follows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AgentRunTranscriptUnavailable {
    NoTranscriptRef,
    TranscriptRecordMissing,
    StateRootUnavailable,
    PathResolutionFailed,
    ReadFailed,
}

fn agent_run_detail_unavailable_key(reason: AgentRunTranscriptUnavailable) -> &'static str {
    match reason {
        AgentRunTranscriptUnavailable::NoTranscriptRef
        | AgentRunTranscriptUnavailable::TranscriptRecordMissing => {
            "agent-run-detail-no-transcript"
        }
        AgentRunTranscriptUnavailable::StateRootUnavailable
        | AgentRunTranscriptUnavailable::PathResolutionFailed
        | AgentRunTranscriptUnavailable::ReadFailed => "agent-run-detail-read-error",
    }
}

/// PR-020-B: the real production entry point -- resolves the real
/// `$XDG_STATE_HOME`-derived state root, the same one a real launch
/// used to capture this transcript in the first place. See
/// [`agent_run_transcript_window_with_state_root`] for the chain
/// itself and why the state root is a parameter there rather than
/// resolved internally.
fn agent_run_transcript_window<'a>(
    project: &'a tekstide_core::project::ProjectSession,
    run: &tekstide_core::domain::AgentRun,
) -> Result<
    (
        &'a tekstide_core::domain::Transcript,
        tekstide_core::transcript::TranscriptWindow,
    ),
    AgentRunTranscriptUnavailable,
> {
    agent_run_transcript_window_with_state_root(project, run, open_real_agent_run_state_root())
}

/// PR-020-B: the full chain the handoff's own reachability table names
/// -- transcript reference, discoverable transcript record, real state
/// root, reconstructed storage path (`TranscriptPathResolver::resolve_agent_run`,
/// the same resolution the writer used, re-derived rather than trusting
/// the stored `Transcript.storage_path` raw -- re-validates containment
/// on every read, the reader's own defense-in-depth), bounded window
/// read. `still_being_written` mirrors `ProjectSession`'s own private
/// `agent_run_status_is_active` set exactly (`Preparing`/`Running`/
/// `AwaitingApproval`) -- not accessible from this crate, so restated
/// here rather than widened to `pub` for one caller.
///
/// `state_root` is a parameter, the same testability split
/// `attempt_agent_run_launch_with_profile_and_state_root` already
/// established -- found necessary, not merely convenient, while writing
/// this slice's own reachability test: an earlier version of this
/// function called `open_real_agent_run_state_root()` internally, which
/// always re-resolves the developer's real `$XDG_STATE_HOME` regardless
/// of what state root a test's own launch used, and silently failed
/// every read (`ReadFailed`, the real path simply never existing)
/// against a transcript captured under an injected test root. The real
/// production path still resolves the real root, through
/// [`agent_run_transcript_window`]'s own one line -- this split changes
/// nothing about production, only what a test can control.
fn agent_run_transcript_window_with_state_root<'a>(
    project: &'a tekstide_core::project::ProjectSession,
    run: &tekstide_core::domain::AgentRun,
    state_root: Option<std::path::PathBuf>,
) -> Result<
    (
        &'a tekstide_core::domain::Transcript,
        tekstide_core::transcript::TranscriptWindow,
    ),
    AgentRunTranscriptUnavailable,
> {
    let transcript_id = run
        .transcript_ref
        .as_ref()
        .ok_or(AgentRunTranscriptUnavailable::NoTranscriptRef)?;
    let transcript = project
        .transcripts()
        .iter()
        .find(|transcript| &transcript.id == transcript_id)
        .ok_or(AgentRunTranscriptUnavailable::TranscriptRecordMissing)?;
    let state_root = state_root.ok_or(AgentRunTranscriptUnavailable::StateRootUnavailable)?;

    let request = tekstide_core::transcript::TranscriptPathRequest::new(
        state_root,
        project.canonical_root_path().clone(),
        project.id().clone(),
        run.id.clone(),
    );
    let storage_path = tekstide_core::transcript::TranscriptPathResolver
        .resolve_agent_run(request)
        .map_err(|_| AgentRunTranscriptUnavailable::PathResolutionFailed)?;

    let still_being_written = matches!(
        run.status,
        tekstide_core::domain::AgentRunStatus::Preparing
            | tekstide_core::domain::AgentRunStatus::Running
            | tekstide_core::domain::AgentRunStatus::AwaitingApproval
    );
    let window = tekstide_core::transcript::read_window(
        &storage_path,
        tekstide_core::transcript::TranscriptReadPolicy::default(),
        still_being_written,
    )
    .map_err(|_| AgentRunTranscriptUnavailable::ReadFailed)?;

    Ok((transcript, window))
}

/// `the-window-boundary.md` §2: escaping happens **at the widget**,
/// using `text_safety::quote_untrusted` -- the one primitive every
/// other escaped surface in this crate uses, never a second one.
/// `TranscriptWindow::content()` is raw bytes (D3, `transcript::reader`'s
/// own doc), not guaranteed valid UTF-8 -- lossy decoding first, the
/// same treatment invalid UTF-8 gets everywhere else untrusted bytes
/// become displayed text in this project. Factored out from
/// [`agent_run_detail_view`] for the same testability reason
/// `approval_history_entry_body`/`trust_grant_dialog_paths` already
/// are: a test can assert on the escaped string directly, without an
/// `iced` widget tree and without a real file on disk.
fn agent_run_detail_transcript_body(content: &[u8]) -> tekstide_core::text_safety::DisplayText {
    let lossy_text = String::from_utf8_lossy(content);
    tekstide_core::text_safety::quote_untrusted(&lossy_text)
}

/// `the-window-boundary.md`'s own required distinction, rendered as
/// this function's own required shape: **reader window** (this
/// `TranscriptWindow` is a tail slice of a possibly-larger file --
/// `delivered_start() > 0`) and **writer truncation**
/// (`Transcript.truncation_state`, RFC-011's own bounded-writer record
/// of bytes that were never captured to begin with) are two
/// **independent, separately rendered** notices, never merged into one
/// message -- "you are seeing part of this file" and "part of this
/// file was never kept" are different facts about the user's data, and
/// either can be true without the other. `Complete` vs
/// `StillBeingWritten` is a third, also separate. Factored out for the
/// same reason [`agent_run_detail_transcript_body`] is: directly
/// testable against constructed values, no real file needed to prove
/// the two notices render distinctly.
fn agent_run_detail_notices(
    catalog: &Catalog,
    transcript: &tekstide_core::domain::Transcript,
    window: &tekstide_core::transcript::TranscriptWindow,
) -> Vec<String> {
    let mut notices = Vec::new();

    notices.push(catalog.get(match window {
        tekstide_core::transcript::TranscriptWindow::StillBeingWritten { .. } => {
            "agent-run-detail-status-active"
        }
        tekstide_core::transcript::TranscriptWindow::Complete { .. } => {
            "agent-run-detail-status-finished"
        }
    }));

    notices.push(if window.delivered_start() > 0 {
        catalog.get_with_args(
            "agent-run-detail-window-partial",
            &CatalogArgs::new()
                .number("shown_len", window.content().len() as u64)
                .number("total_len", window.total_len())
                .number("delivered_start", window.delivered_start()),
        )
    } else {
        catalog.get_with_args(
            "agent-run-detail-window-full",
            &CatalogArgs::new().number("total_len", window.total_len()),
        )
    });

    if transcript.truncation_state == tekstide_core::domain::TruncationState::Truncated {
        notices.push(catalog.get("agent-run-detail-writer-truncated"));
    }

    notices
}

/// RFC-032, `what-the-trust-dialog-must-say.md` §1: escapes the paths
/// this dialog renders at the widget (`text_safety::quote_untrusted`,
/// the same primitive every other untrusted-text site in this crate
/// uses -- no second one). Factored out from [`trust_grant_dialog_body`]
/// so both are directly testable without going through `iced`'s
/// `Element` tree, the same shape `paste_preview`/`approval_dialog_body`
/// already use.
///
/// Returns the escaped **canonical** path -- what trust actually binds
/// to (`docs/src/contributors/security-decisions.md`) -- and, only when
/// it differs from the escaped root path, the escaped root path too:
/// "show both when they differ," per that same handoff item, for the
/// symlinked-project case.
fn trust_grant_dialog_paths(
    modal: &TrustGrantModal,
) -> (
    tekstide_core::text_safety::DisplayText,
    Option<tekstide_core::text_safety::DisplayText>,
) {
    let canonical = tekstide_core::text_safety::quote_untrusted(
        &modal.canonical_root_path.display().to_string(),
    );
    let secondary = (modal.root_path != modal.canonical_root_path).then(|| {
        tekstide_core::text_safety::quote_untrusted(&modal.root_path.display().to_string())
    });
    (canonical, secondary)
}

fn trust_grant_dialog_body(catalog: &Catalog, modal: &TrustGrantModal) -> String {
    let (canonical, secondary) = trust_grant_dialog_paths(modal);
    let mut body = catalog.get_with_args(
        "trust-grant-dialog-body",
        &CatalogArgs::new().untrusted("path", &canonical),
    );
    if let Some(root) = secondary {
        body.push('\n');
        body.push_str(&catalog.get_with_args(
            "trust-grant-dialog-symlink-notice",
            &CatalogArgs::new().untrusted("root_path", &root),
        ));
    }
    body
}

/// RFC-032: the real trust-grant confirmation dialog --
/// `what-the-trust-dialog-must-say.md`'s own review gate, item by item:
/// the path is escaped at the widget and the canonical one is what's
/// shown ([`trust_grant_dialog_paths`]); focus defaults to `Cancel`
/// (`TrustGrantModal`'s own construction); the canonical sentence and
/// the present-and-future consequence are in `trust-grant-dialog-body`
/// verbatim, not paraphrased; the nine restricted-mode features are not
/// enumerated anywhere here.
fn trust_grant_dialog_view<'a>(
    state: &'a State,
    modal: &'a TrustGrantModal,
) -> Element<'a, Message> {
    let button_line = |target: TrustGrantButton, label_key: &str| {
        let marker = if modal.focus == target { "> " } else { "  " };
        text(format!("{marker}{}", state.catalog.get(label_key))).size(state.theme.font_size_body())
    };

    let lines: Vec<Element<'_, Message>> = vec![
        text(state.catalog.get("trust-grant-dialog-title"))
            .size(state.theme.font_size_heading())
            .into(),
        text(trust_grant_dialog_body(&state.catalog, modal))
            .size(state.theme.font_size_body())
            .into(),
        button_line(TrustGrantButton::Grant, "trust-grant-dialog-grant").into(),
        button_line(TrustGrantButton::Cancel, "trust-grant-dialog-cancel").into(),
        text(state.catalog.get("trust-grant-dialog-hint"))
            .size(state.theme.font_size_status())
            .into(),
    ];

    modal_dialog_box(state, column(lines).spacing(10).into())
}

/// RFC-033 PR-033-C: `$count`/`$bytes` are `TranscriptPurgeModal`'s own
/// captured-at-open-time values -- see its doc comment for why these are
/// not re-read live. `what-purge-must-remove.md`'s own required content:
/// what disappears (this project's transcript bytes, named as such, not
/// "data"), the scope ("this project" -- other projects unaffected), and
/// that it cannot be undone. Does not claim purge removes every trace:
/// a tombstone remains, per `purge_project_transcripts`'s own real
/// behavior, and this message says only what the surface can honestly
/// promise.
fn transcript_purge_dialog_body(catalog: &Catalog, modal: &TranscriptPurgeModal) -> String {
    catalog.get_with_args(
        "transcript-purge-dialog-body",
        &CatalogArgs::new()
            .number("count", modal.transcript_count)
            .number("bytes", modal.retained_bytes),
    )
}

fn transcript_purge_dialog_view<'a>(
    state: &'a State,
    modal: &'a TranscriptPurgeModal,
) -> Element<'a, Message> {
    let button_line = |target: TranscriptPurgeButton, label_key: &str| {
        let marker = if modal.focus == target { "> " } else { "  " };
        text(format!("{marker}{}", state.catalog.get(label_key))).size(state.theme.font_size_body())
    };

    let lines: Vec<Element<'_, Message>> = vec![
        text(state.catalog.get("transcript-purge-dialog-title"))
            .size(state.theme.font_size_heading())
            .into(),
        text(transcript_purge_dialog_body(&state.catalog, modal))
            .size(state.theme.font_size_body())
            .into(),
        button_line(
            TranscriptPurgeButton::Purge,
            "transcript-purge-dialog-purge",
        )
        .into(),
        button_line(
            TranscriptPurgeButton::Cancel,
            "transcript-purge-dialog-cancel",
        )
        .into(),
        text(state.catalog.get("transcript-purge-dialog-hint"))
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
