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

use iced::widget::{center, column, container, opaque, row, stack, text};
use iced::{Background, Border, Element, Length, Subscription, Task, keyboard};

use tekstide_core::command::AppCommand;
use tekstide_core::navigation::{KeybindingPolicy, NavigationAction};
use tekstide_core::project::ProjectMode;
use tekstide_core::route::AppRoute;
use tekstide_core::shell::ApplicationShell;

use crate::i18n::{Catalog, CatalogArgs};
use crate::input::{self, FocusZone, RoutedInput, TextStream};
use crate::measurement::{self, Measurement};
use crate::theme::Theme;

/// RFC-015 PR-015-F: the synthetic typing-measurement surface's preloaded
/// content -- a real ~1,500-line source file, not a lorem-ipsum
/// placeholder, so the layout cost the measurement exercises is the same
/// shape a real editor would see. `tekstide-gui-spike` (`publish = false`)
/// used the RFC-014 spike's own precedent of `include_str!`-ing this file
/// directly out of `tekstide-core`; `tekstide` is a published crate, and a
/// package tarball can never contain a sibling crate's source, so this is
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

pub(crate) struct ModalContent {
    focus: ModalButton,
}

impl Default for ModalContent {
    fn default() -> Self {
        // Defaulting to the less destructive-sounding target, the same
        // reasoning the RFC-014 spike's `DialogButton::Deny` default
        // used -- this modal has no real consequence either way, but the
        // convention is cheap to keep consistent.
        Self {
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
    /// RFC-017 PR-017-E: empty unless `TEKSTIDE_TERMINAL_DEMO` is set --
    /// see [`launch_terminal_demo_panes`]. Rendering state only; *which*
    /// slot each pane's session occupies is asked of `tekstide-core`
    /// fresh each time (`active_project_terminal_sessions`), not cached
    /// alongside these panes.
    terminal_demo: Vec<crate::surface::terminal::TerminalPane>,
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
        let mut audit_health = tekstide_core::audit::AuditHealth::default();
        let terminal_demo = launch_terminal_demo_panes(&mut app_shell, &mut audit_health);

        Self {
            app_shell,
            catalog,
            theme: Theme::default(),
            focus: FocusZone::MainArea,
            modal,
            measurement,
            typing_doc,
            terminal_demo,
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
    /// RFC-017 PR-017-C/E: periodic poll for every pane in
    /// `state.terminal_demo` -- see [`launch_terminal_demo_panes`] and
    /// [`terminal_demo_subscription`]. Every pane is polled every tick
    /// regardless of its session's visible slot (the hidden-session
    /// decision, `surface::terminal`'s module doc).
    TerminalDemoTick,
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Input(RoutedInput::Shell(shell_input)) => {
            if let Some(command) = app_command_for(shell_input.action()) {
                state.app_shell.dispatch(command);
            }
        }
        Message::Input(RoutedInput::Surface(_surface_input)) => {
            // No surface exists yet to receive this (PR-015-D). The
            // routing that produced it is proven correct in
            // `input::tests`; there is nothing to consume here yet.
        }
        Message::Input(RoutedInput::Terminal(text_stream)) => {
            // Defense in depth, not the modal-exclusivity boundary
            // itself: `non_modal_subscription` structurally cannot
            // produce this message while a modal is open (see `input`'s
            // module doc), so `state.modal.is_none()` here should always
            // already be true. Checked anyway, at the one place bytes
            // would actually reach a PTY, rather than trusting that
            // upstream property alone -- ablated in `shell::tests`.
            //
            // `terminal_stream_targets_a_live_terminal` gets its first
            // real caller this slice: the demo panes are now registered
            // `TerminalSession`s on the real active project (RFC-017
            // PR-017-E), so the check RFC-015 wrote against the real
            // project model finally has something real to check.
            if state.modal.is_none()
                && terminal_stream_targets_a_live_terminal(&state.app_shell, &text_stream)
                && let Some(bytes) = text_stream.to_pty_bytes()
                && let Some(pane) = state
                    .terminal_demo
                    .iter_mut()
                    .find(|pane| pane.terminal_id() == text_stream.target())
            {
                pane.write_input(&bytes);
            }
        }
        Message::Input(RoutedInput::FocusNext) => state.focus = state.focus.next(),
        Message::Input(RoutedInput::FocusPrevious) => state.focus = state.focus.previous(),
        Message::ModalFocusNext => {
            if let Some(modal) = state.modal.as_mut() {
                modal.focus = modal.focus.next();
            }
        }
        Message::ModalFocusPrevious => {
            if let Some(modal) = state.modal.as_mut() {
                modal.focus = modal.focus.previous();
            }
        }
        // Both dismiss. Real distinct outcomes (e.g. an actual
        // accept/reject decision) belong to RFC-022's real dialogs; this
        // placeholder has no decision to record.
        Message::ModalActivate | Message::ModalDismiss => {
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
        Message::TerminalDemoTick => {
            for pane in &mut state.terminal_demo {
                pane.poll();
            }
        }
    }
    Task::none()
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
/// has something real to demonstrate -- and each is registered on the
/// real active project via `AppState::attach_terminal_session`/
/// `assign_terminal_visible_slot`.
///
/// **Registering for real is a change from PR-017-D**, disclosed rather
/// than silent: that slice's demo pane stayed deliberately unregistered
/// because nothing needed the real session model yet. This slice's job
/// *is* that model's layout/chrome ("no parallel layout model" applies
/// to session bookkeeping as much as to `TerminalPanePolicy` itself), so
/// registering is what discharges that requirement rather than
/// violating it.
///
/// Requires an active project (a CLI project-path argument); returns an
/// empty `Vec` (silently -- this is a diagnostic path, not a
/// user-facing feature yet) otherwise, if the env var is unset, or if a
/// given pane's launch/registration fails.
///
/// **RFC-017 PR-017-F**: each successful launch also records a
/// `plain_terminal_observation` `Started` event via `AuditCoordinator`
/// (never directly to `audit_store`) -- best-effort
/// (`.record_plain_terminal_started`'s own `AuditObservationStatus` is
/// discarded here, matching every other producer call in this crate: an
/// audit write failing must never fail the terminal launch it
/// observes). The store itself is opened here, after both early-return
/// gates, rather than unconditionally in `State::new` -- response 152
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
        let Ok((pane, session)) = crate::surface::terminal::TerminalPane::launch(
            project_id.clone(),
            format!("terminal demo {}", index + 1),
            root,
            std::path::PathBuf::from("/bin/sh"),
        ) else {
            continue;
        };
        let terminal_id = session.id.clone();
        if app_shell
            .state_mut()
            .attach_terminal_session(session)
            .is_err()
        {
            continue;
        }
        if app_shell
            .state_mut()
            .assign_terminal_visible_slot(&terminal_id, slot)
            .is_err()
        {
            continue;
        }

        if let Some(store) = audit_store.as_mut() {
            let _ = tekstide_core::audit::AuditCoordinator::new(store, audit_health)
                .record_plain_terminal_started(project_id.clone(), terminal_id);
        }

        panes.push(pane);
    }
    panes
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

/// Periodic poll driving [`State::terminal_demo`] -- only ever added to
/// the real subscription tree when a demo pane exists (see
/// [`subscription`]), so this changes nothing about `subscription`'s
/// reviewed non-modal/modal routing for any normal run.
fn terminal_demo_subscription() -> Subscription<Message> {
    iced::time::every(std::time::Duration::from_millis(50)).map(|_| Message::TerminalDemoTick)
}

/// `OpenProjectBoard` and, since PR-015-E, `ToggleProjectMode` map to
/// existing `AppCommand`s. `OpenCommandPalette` has a real, reserved
/// binding (`KeybindingPolicy::linux_mvp()`) but no command palette
/// feature exists yet to dispatch to; every other `NavigationAction` has
/// no default binding at all until RFC-023 supplies one. Not a
/// placeholder -- an honest reflection of what is real right now.
fn app_command_for(action: NavigationAction) -> Option<AppCommand> {
    match action {
        NavigationAction::OpenProjectBoard => Some(AppCommand::OpenProjectBoard),
        NavigationAction::ToggleProjectMode => Some(AppCommand::ToggleActiveProjectMode),
        NavigationAction::OpenCommandPalette
        | NavigationAction::SwitchActiveProject
        | NavigationAction::CycleVisibleTerminalSession
        | NavigationAction::OpenCurrentAgentRunDetail
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

pub fn view(state: &State) -> Element<'_, Message> {
    let base: Element<'_, Message> =
        column![top_bar(state), content_area(state), status_bar(state)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

    if let Some(modal) = &state.modal {
        stack![
            base,
            opaque(center(layer_composition_demo_modal(state, modal)))
        ]
        .into()
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
    if let Some(measurement) = &state.measurement {
        return measurement_subscription(measurement.criterion());
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
    if !state.terminal_demo.is_empty() {
        Subscription::batch([routing, terminal_demo_subscription()])
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

    row![sidebar_view(state), main_area_view(state, mode)]
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

fn sidebar_view(state: &State) -> Element<'_, Message> {
    let focused = state.focus == FocusZone::Sidebar;
    container(text(sidebar_label(state)))
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
    let content: Element<'_, Message> = match (mode, state.terminal_demo.is_empty()) {
        (Some(ProjectMode::TerminalImmersion), false) => terminal_workspace_view(state),
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
                .terminal_demo
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

    column![bar, panes_view].spacing(8).into()
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

fn layer_composition_demo_modal<'a>(
    state: &'a State,
    modal: &ModalContent,
) -> Element<'a, Message> {
    let button_line = |target: ModalButton, label_key: &str| {
        let marker = if modal.focus == target { "> " } else { "  " };
        text(format!("{marker}{}", state.catalog.get(label_key))).size(state.theme.font_size_body())
    };

    container(
        column![
            text(state.catalog.get("layer-demo-modal-title")).size(state.theme.font_size_heading()),
            text(state.catalog.get("layer-demo-modal-body")).size(state.theme.font_size_body()),
            button_line(ModalButton::Acknowledge, "layer-demo-modal-acknowledge"),
            button_line(ModalButton::Dismiss, "layer-demo-modal-dismiss"),
            text(state.catalog.get("layer-demo-modal-dismiss-hint"))
                .size(state.theme.font_size_status()),
        ]
        .spacing(10),
    )
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

#[cfg(test)]
mod tests;
