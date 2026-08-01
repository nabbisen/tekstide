# RFC-015: Application Shell and Rendered Surface Model - QA Evidence

Status: Proposed — implementation in progress (PR-015-B landed 2026-07-31, not yet reviewed; PR-015-C onward pending)
Date opened: 2026-07-29
Date accepted: Pending

## Scope

RFC-015 builds the application shell and defines the rendered-surface contract, layer model, and input routing that RFC-017, RFC-019, RFC-020, and RFC-022 all build on.

Evidence in this file must not be used to claim terminal rendering, editor/explorer/diff/report surfaces, real security dialogs, locale catalogs, configuration loading, multi-window support, or screen-reader accessibility — unless later reviewed implementation explicitly supports that claim.

## Inherited obligations

Carried in from RFC-014's approved decision record:

- **R1 — latency unverified.** C2/C3/C4 were not measured; `iced::window::frames()` forces continuous redraw once subscribed. **PR-015-F discharges this or re-records the residual honestly.** Another all-zero figure is not an acceptable outcome.
- **R6 — focus-trap property does not transfer.** The spike's property held only because its terminal emitted no messages. **PR-015-C must re-establish it under an input-accepting design**, with a real test rather than a structural argument.
- **R2 — no screen-reader support**, owner-accepted 2026-07-29. Public claims must state the limitation; no simulated affordance.
- **R9 — survivorship bias** in confirmed-only percentiles applies to any reused synthetic-input harness.

## Design Review

Pending PR-015-A acceptance.

## Implementation Evidence

### PR-015-B — Window, layers, chrome, seams

**Scope followed.** `crates/tekstide` is now a real `iced` (0.14) application; `main.rs`'s text harness (`print!("{}", shell.render_text())`) is gone. `crates/tekstide/src/shell.rs` owns layer composition and the (currently uninhabited) `Message`/`update`/`view`; `crates/tekstide/src/theme.rs` owns the `Theme` seam; `crates/tekstide/src/main.rs` owns process boot (recent-project restore, CLI project-path arguments, catalog resolution), matching `implementation-handoff.md` §1's suggested layout.

**This slice is deliberately non-interactive.** `shell::Message` is an uninhabited enum (`pub enum Message {}`) and `shell::update` is unreachable (`match message {}`). No keyboard subscription exists. This is not an oversight: PR-015-C is the slice that introduces any input at all, and `pr-015-c-input-routing.md` is explicit that a slice inventing its own ad hoc key handling ahead of that document is exactly the shape of mistake it warns against. There was therefore nothing for this slice to route.

**Layer composition** — chrome (top bar, status bar) / content (placeholder; no surface exists until PR-015-D) / modal, composed via `stack`/`opaque`, the mechanism RFC-014 C8 proved. The modal layer's only occupant in this slice is a placeholder demo, per `implementation-handoff.md` §8's explicit allowance ("a placeholder dialog for testing the layer is fine and should be clearly marked as such"). It is env-gated (`TEKSTIDE_LAYER_DEMO`, read once at boot), not keyboard-gated — the same convention RFC-014's own spike used for its measurement/demo flags (`TEKSTIDE_MEASURE_CRITERION`, `TEKSTIDE_I18N_DEMO`) — specifically so this slice adds no input path of its own.

**Theme seam.** `theme::Theme` (background/foreground/accent/border/surface colours, three font sizes), compiled default via `Default`. No `border_focused` field: nothing in this slice has a focus concept yet (no surfaces, no input routing), so a field with no caller was removed rather than shipped dead — the same discipline `LocalePreference`'s ahead-of-caller fields are held to elsewhere in this codebase (those are still *reachable*, just always `None` today; an unused method is a different, weaker case and was cut).

**i18n seam.** Wired through the already-reviewed `i18n::Catalog`/`i18n::CatalogArgs` (RFC-016 PR-016-B/D, response 126 approved). New keys added to `en.ftl`: `content-area-placeholder-title`, `content-area-placeholder-body`, `status-bar-summary` (a two-part key: a literal-variant route selector plus a genuine plural count, in one lookup — the same one-key pattern PR-016-D established for `blocked-automation-count`), and the three `layer-demo-modal-*` keys. Not mirrored into `pl.ftl`: those keys are chrome, not plural-machinery content, and `pl.ftl` exists only to prove CLDR plural-category selection (RFC-016 §Non-Goals); leaving them absent there deliberately exercises the real fallback-to-source-locale path rather than being an oversight.

**Seam enforcement — mechanical, per the review gate's stated preference.** Three heuristic scans (`shell::tests`), each ablation-verified by temporarily reintroducing the violation and confirming the specific test fails, then reverting:
- `no_raw_string_literal_is_passed_to_text_anywhere_in_the_crate` — no `text("literal")` call anywhere scanned; every string comes from `state.catalog.get(...)` or a helper that does.
- `no_raw_color_construction_anywhere_in_the_crate` — no `Color::from_rgb`/`from_rgba` anywhere scanned; every colour comes from `state.theme`.
- `no_raw_font_size_literal_anywhere_in_the_crate` — no bare numeric literal passed to `.size(...)` anywhere scanned; every size comes from `state.theme.font_size_*()`.

**Response 128 Required, fixed:** the original scans named `shell.rs` directly, so `main.rs` was unscanned then, and PR-015-C's routing module and PR-015-D's surface modules would have landed unscanned later — silently, since a passing test reads as coverage. Fixed by walking `crates/tekstide/src` recursively (`scannable_source_files`), so a new source file anywhere in the tree is scanned automatically, with no list to fall out of date. Two stated exemptions: `theme.rs` (the seam's own implementation — it is what other files must source colours *from*, not a violation) and any `tests.rs` (test code legitimately contains literals that are not user-facing shell output). Re-ablation-verified after the fix: temporarily added a raw `text("literal")` call to `main.rs` — previously unscanned, now caught (`main.rs:20 passes a string literal directly to text(...)`); reverted immediately.

These are heuristics over each file's own text, not a full parse — recorded as a limitation, not a full mechanical guarantee (a literal split across an expression the scan doesn't recognize could slip past). No hardcoded-strings scan exists yet at the `tekstide-core::shell::render_text` level; that is RFC-016 PR-016-E's job. Response 128 flagged that this scan and PR-016-E's future scan will overlap in policy (one covers `crates/tekstide`, the other will cover `tekstide-core`) — noted in RFC-016's handoff as a consolidation point for whichever lands second, not solved here.

**Behavioral tests**, decomposed so the underlying string logic is testable without going through `iced`'s `Element` tree (`status_bar_summary` returns a `String`, called by both `view` and the tests directly):
- `window_title_resolves_through_the_catalog_not_a_literal` — proves the title comes from a real catalog key (`Catalog::get`'s "missing key renders as the key itself" fallback would fail this loudly if the key name were ever mistyped).
- `status_bar_summary_reflects_the_default_route_and_zero_projects` and `status_bar_summary_pluralizes_a_single_project_correctly` — the route-label/plural-count summary resolves correctly at zero and at exactly one project (the English singular/plural boundary), reusing PR-016-D's own plural machinery through a real, non-numeric-plus-numeric one-key lookup (`status-bar-summary`, structured like `blocked-automation-count`). Both assertions include Fluent's automatic bidi isolate marks explicitly (two select-expression placeables in one pattern each get isolated, with the inner `{$count}` isolated a second time nested inside) — the same accepted double-isolation response 125 ruled on, arising here from adjacent select expressions rather than `CatalogArgs::untrusted`, and asserted literally rather than stripped, matching `i18n::tests`' own convention.

**No GUI dependency in `tekstide-core`, verified not assumed.** `cargo tree -p tekstide-core --edges normal | grep -i iced` returns nothing.

**No shell-local state mirrors core state.** `shell::State` holds exactly one `ApplicationShell` (the sole source of model state) plus purely presentational fields: `catalog`, `theme`, and `layer_composition_demo_modal_open` (this slice's only "which zone has focus, whether a modal is open"-shaped field, and it is scaffolding, not a real modal occupant). This is recorded by inspection, per `implementation-handoff.md` §2's own examples of what counts as legitimate shell-local state — there is no automated check for "no field secretly duplicates a core value" and none is claimed.

**Screenshot evidence** (response 127's standing convention: `niri msg action screenshot-window --id <id> --path <repo-relative-file>`, targeted by window ID so the owner's desktop focus was never touched — no `focus-window` call was made or needed, since this slice has no input to deliver):

- `evidence/pr-015-b/shell-chrome-over-real-state.png` — the shell running with no projects: window titled "Tekstide" (confirming the i18n-sourced window title), top bar, content placeholder, status bar reading "Project Board | 0 projects" (confirming `status_bar_summary` against real, empty `ApplicationShell` state).
- `evidence/pr-015-b/layer-composition-demo-modal-above-content.png` — `TEKSTIDE_LAYER_DEMO=1`: the placeholder dialog renders centered, above the content area, chrome still visible above and below it — the `stack`/`opaque` composition holds.

**A property this slice established, not only tested (response 128): `iced` honours Fluent's automatic bidi isolation.** `status_bar_summary` demonstrably produces `⁨Project Board⁩ | ⁨⁨0⁩ projects⁩` (isolate marks and all, asserted literally in `shell::tests`), and `shell-chrome-over-real-state.png` shows it rendering clean as `Project Board | 0 projects` — no stray glyphs, no tofu. `iced`/`cosmic-text` consumes U+2068 (First Strong Isolate) and U+2069 (Pop Directional Isolate) as directional instructions rather than drawing them visibly. This closes RFC-016 `qa-evidence.md`'s open limitation ("hasn't been evaluated against every possible downstream renderer... no such renderer exists yet") — one now exists, on this exact interpolated string, and it handles the marks correctly. Not claimed here: that *every* possible interpolated string or every other rendering path in `iced` behaves identically — only that the one real, shipped case this slice renders does.

**What these screenshots prove, and what they must not be cited for** (response 127, so a future closeout does not overclaim from them):
- They prove Tekstide's own shell — not the RFC-014 spike — actually boots on `iced`, renders chrome over real `ApplicationShell` state, and composes a layer above content. That is "the product is wired up," and nothing more.
- **They do not re-prove substrate modal composition.** RFC-014 PR-014-D already did that, with a stronger artifact (`evidence/pr-014-d/genuine-and-adversarial-dialog-one-frame.png`: a real modal beside an adversarial terminal imitation in one frame). A placeholder box here adds nothing to that claim.
- **They do not make the RFC-009 §212 trusted-UI claim.** That claim is about a trusted dialog composited above *untrusted terminal output*; there is no terminal surface until RFC-017, and no untrusted content renders anywhere in this slice. The security-relevant version of this evidence lands in RFC-018, not here.

Gates run 2026-07-31: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (490 `tekstide-core` + 23 `tekstide` — up from 15, 8 net new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

### PR-015-C — Input routing and focus model

**The security-critical slice.** `pr-015-c-input-routing.md` read first, as instructed. New module `crates/tekstide/src/input.rs` (+ `input/terminal_surface.rs`) owns classification; `shell.rs` owns turning a classified input into a real state change.

**The three input classes, as distinct types, per the required structure:**
- `ShellInput(NavigationAction)` — private tuple field, defined in `input.rs`. Only `route_non_modal_input` (in the same module) constructs one.
- `SurfaceInput { target: FocusZone, key: KeyPress }` — private fields, same module.
- `TextStream` — defined in `input::terminal_surface`, a *submodule* of `input`. Its sole constructor, `from_terminal_key`, is `pub(super)` rather than `pub(crate)` — deliberately narrower than the crate-visibility level, because `pub(crate)` would let a future `crates/tekstide/src/surface/*` module (PR-015-D) construct one directly, exactly the bypass this type exists to close. `pub(super)` permits only `input` (the router) and `input`'s own descendants (its `#[cfg(test)]` code) — never `shell.rs`, `main.rs`, or a surface module.

**Guard-deletion resistance, made structural via a proof token, not asserted by convention.** `route_non_modal_input` requires an `input::ModalAbsent` argument. The only constructor is `ModalAbsent::check(&Option<T>)`, which inspects the real modal state at the call site; `ModalAbsent`'s single field is a private `()`, so there is no other way to produce one. Deleting the `match input::ModalAbsent::check(&state.modal) { ... }` guard in `shell::subscription` and calling `non_modal_subscription` unconditionally is therefore not a runtime behaviour change — the compiler has no `ModalAbsent` value to pass, since none can be constructed outside `input`. Same "make the invalid state unrepresentable" pattern already used for `DisplayText`, `VerifiedCwd`, `RunCapabilityToken`, and `CatalogArgs` (RFC-016 response 125), applied here to a routing decision.

**Three compile-fail probes, run and confirmed, not asserted from reading the code** (manual probe-then-revert, `tekstide` has no `[lib]` target for a permanent `compile_fail` doctest — same disclosed limitation as `CatalogArgs`, response 125):
1. `crate::input::TextStream::from_terminal_key(...)` called from `shell.rs` → `error[E0624]: associated function 'from_terminal_key' is private`.
2. `crate::input::ShellInput(NavigationAction::OpenProjectBoard)` called from `shell.rs` → `error[E0603]: tuple struct constructor 'ShellInput' is private`.
3. `non_modal_subscription(input::ModalAbsent(()), state.focus)` called from `shell.rs`, bypassing `ModalAbsent::check` entirely (the literal "delete the guard" scenario) → `error[E0603]: tuple struct constructor 'ModalAbsent' is private`.

All three probed by temporarily adding the offending code to `shell.rs`, confirming the exact compile error, then reverting — `git diff --check` clean afterward.

**Response 130 Required 1 — corrected: the *call gate* is structural; *exclusivity* additionally depends on `iced`'s subscription lifecycle, and that dependency was unstated.** The original wording here claimed "modal exclusivity is structural... not 'checked'" without qualification. That overstates what the type proves. `route_non_modal_input` genuinely cannot be called without a `ModalAbsent`, and none can be constructed outside `input` — the *call* is gated structurally, confirmed above. But `ModalAbsent` is `Copy` (required by `.with()`'s `Hash` bound, so it can be threaded into a long-lived subscription closure), which means a proof obtained once can outlive the instant it was true. The architect probed this directly:
```
modal active = true
routed anyway with a stale proof -> Surface(SurfaceInput { target: MainArea, ... })
SurfaceInput produced while a modal is open? true
```
**A captured proof does produce `SurfaceInput` while a modal is active.** This is not exploitable today: `shell::subscription` re-evaluates on each rebuild and returns `modal_subscription()` once `state.modal` is `Some`, so the non-modal subscription — and its captured proof — is torn down by `iced` before that stale proof could ever be used against a real key event. **But the property holds because of `iced`'s subscription-rebuild lifecycle, not because of the type alone.** That is the corrected claim: the call gate is structural (type-level, tested by the three compile-fail probes above); full exclusivity is that gate *plus* a framework-lifecycle assumption that `iced` actually tears down a dropped subscription branch promptly. This matters beyond bookkeeping because RFC-022's approval-dialog property ("no keystroke reaches a PTY while a dialog is open") rests on exactly this chain, and RFC-017 will put real PTY keystrokes through it — so it needs to inherit an accurate description of what it depends on, not an overstated one.

To make at least the branch-selection half assertable rather than left implicit, the decision is now extracted into `input::SubscriptionMode`:
```rust
pub enum SubscriptionMode {
    NonModal(ModalAbsent),
    Modal,
}
impl SubscriptionMode {
    pub fn for_modal<T>(modal: &Option<T>) -> Self { /* wraps ModalAbsent::check */ }
}
```
`shell::subscription` matches on `SubscriptionMode::for_modal(&state.modal)`; `shell::tests::subscription_mode_reflects_whether_a_modal_is_active` asserts directly that a `State` with a modal yields `Modal` and one without yields `NonModal(_)` — converting that half from an implicit assumption into a tested one, without touching the routing design or fighting `Subscription`'s opacity (`Copy` was kept on `ModalAbsent`, per the architect's explicit instruction not to remove it — `.with()` needs it, and removing it would fight the framework for no gain now that the branch itself is tested).

`modal_subscription()` remains a *separate* function producing only `Message::Modal*` variants — it has no path to constructing `RoutedInput::Surface` or `RoutedInput::Terminal` at all. While a modal is shown, the subscription that could produce those is not subscribed (modulo the framework-lifecycle dependency named above), matching the required property's language: "not produced," not "produced and discarded."

**Precedence, and the one deliberate simplification recorded rather than silently resolved.** `route_non_modal_input` checks, in order: (1) a key matching a live `KeybindingPolicy` rule — global keybindings always win, proven with a terminal nominally focused (`a_global_keybinding_wins_over_a_focused_terminal`); (2) Tab/Shift+Tab, the shell's own focus cycle; (3) `terminal_focus`, if set; (4) otherwise `SurfaceInput` for the focused zone. Tab is checked *ahead of* terminal focus deliberately — whether Tab should instead reach a terminal's text input (shell completion, etc.) is a real, currently unanswerable question with no terminal surface yet to decide it against; recorded for RFC-017, not resolved here (this is RFC-015's Open Question 2, "raw key events or pre-interpreted intents" — answered narrowly: raw `KeyPress` for now, since the actual raw-vs-intent tradeoff has no real surface yet to weigh it against; deferred to whichever of PR-015-D/RFC-019 first needs to decide).

**The `TerminalId` liveness check** (`pr-015-c-input-routing.md` requirement 2: "a stale or cross-project id is dropped, not best-effort delivered") lives in `shell::terminal_stream_targets_a_live_terminal`, checking `app_shell.state().active_project().and_then(|p| p.terminal_session(id)).is_some()`. **Only the negative path is testable from `tekstide`'s own test suite today**, and this is a real, disclosed gap, not a shortcut: attaching a genuinely live `TerminalSession` needs `tekstide_core::AppState::project_mut`, which is `#[cfg(test)]`-gated *inside `tekstide-core` itself* with zero production call sites anywhere in the tree (`grep`-confirmed — every `add_terminal_session` caller is a `tekstide-core` test; terminal creation is RFC-017's job). `#[cfg(test)]` does not cross the crate boundary, so `tekstide`'s tests cannot reach it, and widening `tekstide-core`'s API to manufacture a fixture would itself be "a change to `tekstide-core` state models without raising it first" (`implementation-handoff.md` §8) — not something to do unilaterally to make a test easier. Proven instead: a never-existed `TerminalId` against a real (empty) active project (`a_never_added_terminal_id_is_not_live`), and no active project at all (`with_no_active_project_no_terminal_id_is_ever_live`) — both correctly `false`. The positive case becomes testable the moment RFC-017 gives `tekstide-core` a real way to attach a terminal.

**No PTY-writing path exists anywhere yet** (RFC-017 has not landed) — `shell::update`'s handling of `RoutedInput::Terminal` only exercises the liveness check and discards the result; there is nothing to deliver to or drop from in practice. The type and the check are proven correct now so RFC-017 inherits a working, tested contract rather than discovering the check wrong once it matters.

**A real focus-trap test, discharging RFC-014 R6** — not a structural argument. `shell::tests::modal_focus_cycling_never_touches_the_shell_focus_cycle` dispatches real `Message::ModalFocusNext`/`ModalFocusPrevious` through `update` while `state.modal` is `Some`, and asserts both that the modal's own `ModalButton` cycles correctly (Acknowledge ↔ Dismiss) *and* that `state.focus` (the shell's own focus cycle) never moves. `dismissing_the_modal_clears_it_and_leaves_shell_focus_undisturbed` proves dismissal (`Enter`/`Escape` → `ModalActivate`/`ModalDismiss`) clears the modal. Focus-return-to-invoking-element (UI/UX §18) falls out for free rather than needing separate restore logic: because `state.focus` was never touched while the modal was shown (proven by the same test), whatever it held before is simply still there after dismissal.

**The modal became genuinely dismissible.** PR-015-B's placeholder (env-gated, never closing) could not exercise either property above — a modal that never closes cannot prove exclusivity ends correctly or that focus returns anywhere. `ModalContent { focus: ModalButton }` now has two real targets (`Acknowledge`/`Dismiss`, mirroring the RFC-014 spike's `DialogButton` shape), Tab/Shift+Tab cycles between them, Enter/Escape dismisses. Still explicitly scaffolding (`implementation-handoff.md` §8's placeholder allowance) — RFC-022 supplies real dialogs; opening this one remains `TEKSTIDE_LAYER_DEMO`-gated, since there is still no real trigger to open a dialog, only now a real way to close one.

**`ShellInput` dispatch is real, and honestly partial.** `app_command_for` maps `NavigationAction::OpenProjectBoard` to `AppCommand::OpenProjectBoard` — the only pairing where both a live default keybinding (`KeybindingPolicy::linux_mvp()`) and an existing `AppCommand` exist today. `OpenCommandPalette` has a real, reserved binding (`Ctrl+Shift+P`) but no command-palette feature to dispatch to yet; every other `NavigationAction` has no default binding at all until RFC-023. `a_project_board_shell_input_dispatches_the_real_app_command` proves the dispatch is genuine: starts from `ActiveProjectWorkspace`, feeds a real `ShellInput` through `update`, confirms the route actually changed via `status_bar_summary`.

**Focus cycling exists and is proven, even though it is a no-op today.** `FocusZone` has a single real variant (`MainArea`, `#[non_exhaustive]`) — PR-015-B built no sidebar (PR-015-E's scaffolding). `focus_next_and_previous_route_through_update` proves `FocusNext`/`FocusPrevious` route through `update` correctly now, so the day PR-015-E adds `Sidebar`, this test either still passes trivially or fails pointing at exactly what needs updating.

**No `tekstide-core` changes.** All of the above is new `tekstide` code; nothing in `crates/tekstide-core` was touched.

Gates run 2026-07-31 (original submission): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (490 `tekstide-core` + 35 `tekstide` — up from 23, 12 net new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

**Response 130 fixes, gates re-run 2026-07-31:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (490 `tekstide-core` + 37 `tekstide` — up from 35, 2 net new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

**Screenshot evidence, with real synthetic input.** Flagged before running, per response 129's explicit instruction, and the owner approved it explicitly before any command ran. Captured exactly per RFC-014's precedent: relaunched with `WAYLAND_DISPLAY` unset (`env -u WAYLAND_DISPLAY TEKSTIDE_LAYER_DEMO=1 ./target/debug/tekstide`) to force the X11/XWayland `winit` backend, `xdotool search --name Tekstide` for the X11 window id, `xdotool windowfocus <id>` (required here — a first attempt via `key --window <id>` alone, without focusing, delivered nothing; `windowactivate` failed outright since niri's `XWayland` support does not answer `_NET_ACTIVE_WINDOW`, but plain `windowfocus` worked), then `xdotool key --window <id> <Key>`. Screenshots captured by niri's own window id (`niri msg action screenshot-window --id <id> --path <file>`, response 127's convention) — a *different* id namespace than xdotool's X11 id, confirmed by cross-checking `niri msg windows`.

- `evidence/pr-015-c/modal-1-initial-dismiss-focused.png` — modal open (`TEKSTIDE_LAYER_DEMO=1`), `ModalContent::default()`'s `Dismiss` focused (`>` marker), matching the code.
- `evidence/pr-015-c/modal-2-tab-to-acknowledge-focused.png` — after one real `Tab` keystroke: focus moved to `Acknowledge`.
- `evidence/pr-015-c/modal-3-tab-cycles-back-to-dismiss.png` — after a second real `Tab`: focus cycled back to `Dismiss`.
- `evidence/pr-015-c/modal-4-escape-dismisses.png` — after a real `Escape` keystroke: the modal is gone, content area and chrome visible underneath, undamaged.

**The cycle proof (response 130), stated precisely, not just "byte-different."** `md5sum` of all four:
```
modal-1-initial-dismiss-focused.png        0ce3842440110517b7f51cb963309fc4
modal-2-tab-to-acknowledge-focused.png     ed18602bd19b1746522409da655e2558
modal-3-tab-cycles-back-to-dismiss.png     0ce3842440110517b7f51cb963309fc4
modal-4-escape-dismisses.png               f359fcdcd86c3434c3753364c844000e
```
**1 and 3 are byte-identical; 2 differs from both.** If the second `Tab` press had done nothing, 3 would equal 2, not 1. It equals 1 — focus genuinely traversed to `Acknowledge` and back to `Dismiss`. That is proof of a real two-way *cycle*, the property `modal_focus_cycling_never_touches_the_shell_focus_cycle`'s name actually claims, not merely proof that a keystroke changed something on screen.

**What these screenshots prove, and what they do not** (same discipline as response 127 applied to PR-015-B's): they prove the modal's Tab-cycle and Escape-dismiss genuinely work end-to-end through real input, on this exact `iced` substrate. They do not prove modal *exclusivity* (that surface/terminal input is *not produced* while the modal is shown) — that property is structural (`ModalAbsent`, a different subscription function entirely) and is proven by code inspection and the compile-fail probes above, not by a screenshot; there is no surface or terminal yet whose non-delivery a screenshot could show either way.

### PR-015-D — Project Board surface

**Scope followed.** `crates/tekstide/src/surface.rs` (module doc, contract statement) + `surface/board.rs` (the Project Board) render `ApplicationShell::project_board()` — an existing, already-tested `ProjectBoardViewModel` — into the content area, replacing the placeholder for `AppRoute::ProjectBoard` (`AppRoute::ActiveProjectWorkspace` keeps the placeholder; no real workspace surface exists yet).

**The rendered-surface contract is concrete methods, not a `trait Surface`, deliberately.** RFC-015 names identity/view/input-interest/focus-zones/status-contribution as what a surface declares. With exactly one implementor, a formal trait would be abstraction with no second case to generalize from — the same reasoning that cut `Theme::border_focused` in PR-015-B (no caller, no trait). `board::view` is the pure view function (`&ProjectBoardViewModel, &Catalog, &Theme -> Element`); input interest is `None` (nothing in this slice's scope asks for row selection); focus zones/status contribution are not yet meaningful with a read-only, single-zone board. RFC-019/RFC-020 — whichever writes the second surface — is where generalizing to a real trait pays for itself.

**Untrusted text is escaped, proven three ways.** `display_name` and `root_path_hint` (filesystem-derived, attacker-influenceable) go through `text_safety::quote_untrusted` before reaching any widget, never `CatalogArgs::trusted_symbol`, never raw into `text(...)`.
1. Unit tests (`surface::board::tests`): `an_untrusted_project_name_with_a_bidi_override_is_escaped_not_live` and the `root_path_hint` equivalent, using the reviewer's own probe shape (`"proj\u{202E}gpj.exe"`), asserting no live `U+202E` and the escaped `<U+202E>` marker present. `an_ordinary_project_name_renders_unescaped` proves escaping is conditional on content, not blanket mangling.
2. **Ablation-verified**: temporarily rendered `row.display_name` raw (bypassing `quote_untrusted`) — the bidi-override test failed with the live override present; reverted.
3. **Real screenshot, with a real directory on disk** — anticipating the architect's own stated probe plan (response 130: "a project directory named to carry a bidi override... as a real directory on disk"), rather than waiting to be shown the gap: created `proj\u{202E}gpj.exe` as a genuine directory via `mkdir`, launched `tekstide` with it and a clean project as CLI arguments, screenshotted the real running board. `evidence/pr-015-d/project-board-with-untrusted-name-escaped.png` shows the row rendering `proj<U+202E>gpj.exe` — escaped, not reordered — alongside `clean-demo-project` rendering intact. A third row (`tekstide-git`) appeared from a prior test session's persisted `~/.local/state/tekstide/recent-projects.json` — real evidence that recent-project persistence (PR-016-B era) works across process runs, left untouched (real user-local application state outside the repo, not a repo artifact to edit).

**`CountDisplay` fidelity — the acceptance criterion, decided deliberately, not by calling `label()` and moving on** (response 130's explicit framing of what it would look at first). `CountDisplay::label()` is **never called** from this surface. Every `CountDisplay` field (`branch_status`, `terminal_count`, `agent_run_count`, `approval_count`, `review_count`, `dirty_file_count`) routes through `count_display_args` into real `en.ftl` keys (`project-board-terminal-count` etc.), reusing PR-016-D's `not_implemented`/`unavailable`/`unknown` symbolic vocabulary. `label()` keeps its existing (and only) caller, `tekstide_core::shell::render_project_board` — see the `render_text` decision below.
- `unavailable_and_not_implemented_never_render_as_zero`: every `CountDisplay` field set to `Unavailable` or `NotImplemented`, across two full rows, asserts no rendered line contains a bare `0` anywhere.
- `a_genuine_known_zero_count_does_render_as_zero`: the positive case — `KnownCount(0)` legitimately renders `"⁨0⁩ terminals"` (real CLDR plural selection, isolate marks and all, matching `i18n`/`shell` tests' own convention of asserting them literally) — proving the rule is "never *fake* zero," not "never show zero at all."
- `unavailable_terminal_count_uses_the_catalog_not_label`: exact wording (`"terminals: not available"`) only exists in this surface's own catalog key, not in `CountDisplay::label()`'s output (`"not available"`, no prefix) — **ablation-verified**: temporarily replaced the catalog call with `row.terminal_count.label()` directly — failed with the bare, unprefixed string; reverted.
- **Mechanical scan, extending the existing family** (`shell::tests::no_count_display_or_attention_label_is_called_anywhere_in_the_crate`): no scanned file may contain `.label()` at all — **ablation-verified** the same way, confirming it independently of the unit test above.

**`attention` (an `AttentionState` enum, not just the pre-baked `attention_label: String`) is also routed through the catalog**, for the same reason `CountDisplay` is: the enum is available, so there is no reason to fall back to core's English. `attention_state_renders_through_the_catalog` proves it.

**Known limitation, disclosed rather than silently accepted: `trust_label`, `security_mode_label`, `availability_label`, `blocked_automation_labels` are rendered as-is, not catalog-driven.** Unlike `CountDisplay`/`AttentionState`, `ProjectBoardRow` exposes **only** a pre-rendered `String` for these — no underlying enum (`WorkspaceTrust`, etc.) alongside it. These are trusted (fixed-set, not filesystem-derived), so rendering them as-is is not a security gap, but it is an i18n-completeness gap: closing it needs `tekstide-core::ProjectBoardRow` to expose the underlying enums, a `tekstide-core` API change out of this slice's scope (`implementation-handoff.md` §8 — raise core changes first, don't fold them into an unrelated slice). Recorded here as a new site for whichever future slice takes on `tekstide-core`'s i18n-completeness work, alongside the `render_text`/`CountDisplay::label()` sites RFC-016 already tracks.

**Empty state is catalog-driven from a structural signal, not core's pre-baked strings.** `ProjectBoardEmptyState.heading/primary_action/secondary_action` are hardcoded English in `tekstide-core`; this surface reads only `Option::is_some()` from it and selects its own three `en.ftl` keys — no `tekstide-core` change needed (unlike the label-string fields above), since the only signal required was already structural. `empty_state_keys_resolve_to_real_catalog_text` proves the three keys exist and resolve to real text.

**`tekstide_core::shell::render_text` is *not* deleted, despite the RFC-015 handoff README inviting it** ("PR-015-D is expected to delete it... If you do not delete it, say so explicitly"). Investigated first rather than assumed: `render_text` (and its private helpers `render_project_board`/`render_active_project_workspace`) is not dead weight — it is the primary assertion mechanism for roughly twenty existing `tekstide-core::shell::tests` covering route transitions, mode switching, terminal pane visibility, document editing, and explorer scanning, none of which this slice touches. Deleting `render_text` would require rewriting all of them to structured field-by-field assertions — a substantial `tekstide-core` test-suite refactor, not a side effect of building GUI rendering, and exactly the kind of `tekstide-core` change `implementation-handoff.md` §8 says to raise first rather than fold into an unrelated slice. Kept; `CountDisplay::label()`'s only remaining caller is this harness. `render_text` itself is `pub fn` on a library-shaped crate (`tekstide-core` has a `lib.rs`), so it does not trip `dead_code` regardless of real callers — its unused-in-production status is a QA fact, not a compiler warning, and is recorded here rather than left to be discovered.

**No shell-local state, no trusted chrome, no modal access — by construction, not by a runtime check.** `surface::board` holds no persistent struct or field at all; every function is `(&ViewModel, &Catalog, &Theme) -> Element` or `-> Vec<String>`, called fresh on every `view()`. It has no path to `shell::State`'s `modal` field (never passed in) and no path to `top_bar`/`status_bar` (`board::view`'s return value only ever fills `shell::content_area`'s content slot).

Gates run 2026-07-31 (original submission): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (490 `tekstide-core` + 48 `tekstide` — up from 37, 11 net new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

**Response 132 Required, fixed: the status bar's project count contradicted the board's own row count.** The screenshot submitted with the original review showed three rows (`clean-demo-project`, `proj<U+202E>gpj.exe`, and `tekstide-git` — a project restored from a prior session's persisted `recent-projects.json`) but the status bar read **"Project Board | 2 projects."** Both numbers were individually correct about *different* collections: `status_bar_summary` counted `state.app_shell.state().projects().len()` (open sessions only — 2), while `ProjectBoardViewModel::from_app_state` deliberately also lists recent-but-not-open projects (RFC-005's model), producing a genuinely larger row count (3) whenever an unopened recent project exists. Neither slice was wrong in isolation — PR-015-B's status bar count was correct before any board existed to disagree with it — the seam between them was.

**Fixed** by having `status_bar_summary` count `state.app_shell.project_board().rows.len()` — the identical computation `surface::board::view` renders from — instead of `state.app_shell.state().projects().len()`. The two numbers can no longer independently drift, because they are now, literally, the same collection's length read twice, not two separately-arrived-at counts.

**New test, proving the fix with a scenario where the two collections are genuinely different sizes** (not one where they coincidentally already agreed): `status_bar_project_count_matches_the_board_row_count_including_recent_projects` adds one open project via `add_project_from_path` and one recent-but-not-open project via `restore_recent_projects`, asserts as a precondition that `project_board().rows.len()` and `state().projects().len()` genuinely differ (2 vs. 1), then asserts the status bar's rendered count matches the board's row count, not the open-session count. **Ablation-verified**: reverted the fix temporarily (back to counting `state().projects().len()`) — the new test failed exactly as the original screenshot showed ("the status bar must count what the board renders (2 rows), not just open sessions (1)"); restored the fix immediately after observing the failure.

**Re-verified against the real screenshot scenario, not only the unit test.** Relaunched the same real setup from the original submission (the two on-disk directories, including the genuine `proj\u{202E}gpj.exe`) — `evidence/pr-015-d/status-bar-count-matches-board-rows-after-fix.png` shows the status bar now reading **"Project Board | 3 projects"**, agreeing with the three visible rows.

Gates re-run 2026-07-31 (response 132 fix): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (490 `tekstide-core` + 49 `tekstide` — up from 48, 1 net new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

### PR-015-E — Mode switching and Content-mode scaffolding

Pending implementation.

### PR-015-F — Measurement: R1 discharge

**R1, closed.** When the owner approved `iced` on 2026-07-29, input latency was accepted explicitly *unverified*, conditional on this RFC discharging it. This slice discharges it for warm start (C5) and typing latency (C2); mode switch (C4/`NFR-PERF-002`) is out of scope here — moved to `0.4.1` with PR-015-E (response 133): M8 today has no real mode to switch into that isn't the Project Board against an empty placeholder, so measuring it now would measure scaffolding, not the substrate.

**Why this does not reuse RFC-014 PR-014-E's measurement shape as-is.** The spike proved `iced::window::frames()` is the only application-level "a frame was painted" signal, and subscribing to it forces continuous compositor-driven redraw (~57 Hz, ~2.7% of one core, idle) — every one of its C2/C3/C4 samples read exactly `0µs` as a direct, disclosed consequence, a degenerate result rather than a pass. RFC-015 anticipated this and requires a specific fallback: "measure input-to-state-change and frame cost separately, and report the decomposition... another all-zero figure is not an acceptable outcome." `crates/tekstide/src/measurement.rs` implements that fallback **structurally** rather than reusing `frames()` for typing latency at all:
- **input-to-state-change**: wall-clock time from a measurement keystroke's arrival (timestamped the instant the subscription receives it) to `shell::update` returning. Pure Rust function-call timing — no `frames()` involved, nothing for it to contaminate.
- **view-build cost**: wall-clock time for `shell::view` to construct its `Element` tree, timed by wrapping the view function passed to `iced::application` in `main.rs` (`timed_view`), not by anything inside `shell::view` itself — also `frames()`-free.

Neither figure is "full paint-to-screen time" — that would still need `frames()` and reintroduce the exact contamination this design avoids. Disclosed precisely: this is the "app-internal, not end-to-end" framing RFC-014 already established, carried one level further — even "app-internal" here means this app's own `update`/`view` functions, not `iced`'s internal render pipeline (GPU submission, compositor).

**`Startup` (C5) is the one criterion that still uses `frames()`**, exactly as the spike did — safely, because the process exits immediately after the first frame (`shell::update`'s `MeasurementFrame` handling calls `std::process::exit(0)` once recorded), so there is no *sustained* redraw-forcing during any real interactive session for it to contaminate.

**Machine identification** (same machine as RFC-014's own measurements, for direct comparability): AMD Ryzen 9 9950X (16c/32t); 59 GiB RAM; NVIDIA GeForce RTX 5060 Ti, driver 610.43.03, OpenGL 4.6; compositor niri (Wayland); CachyOS, kernel 7.1.5-1-cachyos; Rust 1.97.1; display 2560×1440@59.951Hz (non-native; native 3840×2160@59.997Hz), fractional scale 1.2×. All figures below from `cargo build --release` — no debug-build numbers recorded.

**Idle-CPU comparison, proving non-contamination empirically, not assumed.** `/proc/<pid>/stat` `utime+stime` ticks (100 ticks/s), diffed over a fixed 3s window:

| Configuration | Ticks / 3s idle |
| --- | --- |
| No measurement env var set (default, every normal run) | **0** |
| `TEKSTIDE_MEASURE_CRITERION=typing`, process idle, no keystrokes sent | **3** (~1% of one core) |

The `3` ticks are the periodic 100ms `MeasurementTick` subscription's real, small, disclosed overhead — used solely to detect "target sample count reached, self-exit," active *only* during an explicit measurement run (never when `state.measurement` is `None`, which is every normal interactive session). This is markedly lower than RFC-014's `frames()`-based ~8 ticks/3s (~2.7%), a direct consequence of avoiding `frames()` for `Typing` entirely rather than a tuning difference.

**C5 — warm start (`NFR-PERF-001`, budget ≤ 800ms).** 15 consecutive release-binary launches (`TEKSTIDE_MEASURE_CRITERION=startup`), first discarded as cold:

| n (warm) | min | median | mean | max | Budget | Result |
| --- | --- | --- | --- | --- | --- | --- |
| 14 | 156.1ms | **163.8ms** | 165.2ms | 178.1ms | ≤ 800ms | **Met, comfortably** |

(Cold first launch: 237.2ms — excluded, matching RFC-014's own methodology of discarding a cold first sample.)

**C2 — typing latency (`NFR-PERF-003`, budget p95 ≤ 16ms), decomposed.** Delivered per RFC-014's established methodology: relaunched with `WAYLAND_DISPLAY` unset (forcing the X11/XWayland `winit` backend), `xdotool search --name Tekstide` for the window id, `xdotool windowfocus <id>` (not `windowactivate` — confirmed again not to work under this niri/XWayland setup, per the PR-015-C finding), then **global** `xdotool key --repeat 100 --repeat-delay 15 j` in batches — **never** `--window`-targeted (RFC-014's own finding: `--window` delivery drops events; global delivery after focusing does not). Batches sent only until the **on-disk log line count** (not a trusted dispatched-count) reached the 1,100 target, per RFC-014's methodology of never padding for a possibly-lost key:

| Dispatched | Confirmed (on-disk) | Delivery loss |
| --- | --- | --- |
| 1,100 | 1,100 | **0.00%** |

First 100 samples of *each* stream discarded as warmup (RFC-014's convention), leaving:

| Stream | n | min | p50 | p95 | p99 | max | mean |
| --- | --- | --- | --- | --- | --- | --- | --- |
| input-to-state-change | 1,000 | 8µs | 23µs | **42µs** | 75µs | 688µs | 25.7µs |
| view-build cost | 1,475 | 33µs | 75µs | **131µs** | 141µs | 164µs | 84.1µs |

(`view` has more samples than `input` because `view()` also runs on each `MeasurementTick` and at boot, not only after a processed keystroke — expected, and irrelevant to the percentiles, which are computed per-stream.)

**Result: the sum of the two streams' p95s is 173µs (0.173ms) against a 16ms budget — met by roughly two orders of magnitude, and this is a genuine, non-degenerate figure, not RFC-014's `0µs` artifact.** Response 134 non-blocking: **this sum is not itself a p95** — summing two independently-computed p95s is not the p95 of a combined distribution, and the two streams are not even paired samples (n=1,000 vs. n=1,475, since `view()` also runs on reasons other than a processed keystroke, e.g. `MeasurementTick`). Stated precisely, not as "typing p95 = 173µs": **the sum of the two measured streams' p95s, used as an upper-bound proxy for end-to-end typing latency.** The margin (173µs against 16,000µs) makes the distinction immaterial to the pass/fail verdict, but the wording matters for anything that quotes this figure later without the caveat attached. This sum approximates the cost of this app's own `update`+`view` logic per keystroke; it is **not** a claim about full paint-to-screen latency, which would require the `frames()` path this design deliberately avoids (see above). The `max` (688µs on `input`) is worth naming rather than hiding behind percentiles — still 0.688ms, over 20× under budget even at the single worst observed sample.

**Worth recording for the closeout: this slice's warm start came in faster than the spike's own figure the budget expectation was set against** — 163.8ms median here vs. 227.9ms recorded in RFC-015 §175 (quoting the spike). Both comfortably clear `NFR-PERF-001`'s ≤800ms; the difference is not itself a claim of anything beyond "this build, this run, was faster," but is worth having in the closeout alongside the number itself.

**Response 134 Required: measurement and the demo modal are now mutually exclusive at construction, and the exclusivity statement above is updated to name this branch.** `shell::subscription`'s measurement check (see PR-015-F above) runs *before* `SubscriptionMode::for_modal` is ever consulted — reachable (both `TEKSTIDE_LAYER_DEMO` and `TEKSTIDE_MEASURE_CRITERION` are independent env vars a developer could set together), and would have meant the demo modal on screen while modal exclusivity was not in effect at all: `route_non_modal_input`'s `ModalAbsent` gate is bypassed entirely by the *measurement* branch, not defeated by a stale proof this time, but by a structurally earlier return that never reaches the modal check in the first place. **Fixed**: `shell::modal_for_state(measurement_active: bool, layer_demo_requested: bool) -> Option<ModalContent>` makes measurement and the modal mutually exclusive by construction — measurement wins (`TEKSTIDE_LAYER_DEMO` is not even read when measurement is active), since a bounded, self-terminating diagnostic run has no reason for PR-015-C's structural property to apply to it, whereas the demo modal exists only to be screenshotted interactively. `shell::tests::measurement_and_the_demo_modal_are_mutually_exclusive` asserts all four input combinations against the pure decision function directly (not by setting the two process-global env vars, which would race against concurrently-running tests also constructing a `State`); ablation-verified by temporarily reverting the function to ignore `measurement_active` — the test failed exactly as expected, then was reverted. **This is now a third stated exception to "modal exclusivity is structural,"** alongside the call-gate/framework-lifecycle split response 130 already recorded: the property holds for the reviewed PR-015-C routing path, is additionally contingent on `iced`'s subscription-rebuild timing (response 130), and does not apply at all while a measurement run is active, by a mutual-exclusion invariant enforced at `State` construction rather than by the routing gate itself.

**Survivorship bias (RFC-014 R9) — the caveat is inherited, and moot here specifically because it doesn't apply.** RFC-014 recorded that percentiles computed over confirmed-received samples only would carry survivorship bias if delivery loss correlated with the app being busy (dropped samples being disproportionately the slow ones). At **0.00% delivery loss** in this run, there is no dropped-sample population for that bias to hide in — the caveat is carried forward as a standing methodology note for any *future* reuse of this harness that sees nonzero loss, not because this run needed it.

**Unit tests** (`measurement::tests`), proving the bookkeeping independent of any real process launch: `record_input_writes_a_real_nonzero_elapsed_sample` (a real 2ms sleep is not measured as ~0µs — the same non-degeneracy property proven live above, proven first at the unit level); `typing_is_done_exactly_at_target`; `startup_is_done_after_one_frame_and_does_not_record_a_second`; `record_startup_frame_is_a_no_op_for_typing`. `shell::tests` adds `is_measuring_typing_is_false_by_default` (the off-by-default contract the idle-CPU comparison's first row depends on) and `tail_lines_keeps_only_the_last_n_lines` (the typing-measurement surface's only view logic worth testing outside `iced`'s `Element` tree).

**Escalation policy check**: no figure here misses its budget by any margin, let alone 2×, so RFC-014 handoff §5's "a >2× miss stops work" policy was never triggered.

Gates run 2026-08-01 (original submission): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (490 `tekstide-core` + 55 `tekstide` — up from 49, 6 net new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

Gates re-run 2026-08-01 (response 134 fix): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (490 `tekstide-core` + 56 `tekstide` — up from 55, 1 net new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

## Known Limitations (PR-015-F)

- **The measurement-only `MeasurementTick` overhead (~1% of one core) exists only during an active measurement run**, never during normal use — disclosed precisely above, not folded into the "0 ticks" idle-CPU claim, which is specifically about the default (no env var) state.
- **View-build cost is a proxy for rendering cost, not full paint-to-screen time.** `iced`'s own internal render pipeline (layout solving beyond tree construction, GPU submission, compositor presentation) remains unmeasured — the same category of gap RFC-014's "app-internal, not end-to-end" disclosure already named, here applied one level further in.
- **C4 (mode switch, `NFR-PERF-002`) is not measured in this slice** — deferred to `0.4.1` with PR-015-E, per response 133's explicit sequencing decision, not an oversight.
- **The typing-measurement surface (`shell::typing_measurement_view`) is deliberately not a real editor** — RFC-019's job. It exists solely to give `view()` a realistically-sized document to build a tree from, matching RFC-014 spike's own precedent (the identical source file, `tekstide-core/src/project/session.rs`, reused for the same reason).
- **No external driver script is committed** — RFC-014's own C2-C5 driver was run ad hoc and never committed either (confirmed by inspection: no `xdotool`/`TEKSTIDE_MEASURE`-referencing script exists anywhere in the tree). The exact commands used here are recorded above in enough detail to reproduce, matching that precedent rather than introducing a new one.

### PR-015-G — Closeout evidence

Pending implementation.

## Known Limitations

- Screen-reader support absent for the life of the `iced` substrate decision (RFC-014 R2, owner-accepted).
- Terminal rendering, editor, explorer, diff, and report surfaces are out of scope; the shell provides only the contract they plug into.
- The modal layer ships with a placeholder dialog for testing; real dialogs are RFC-022.
- **The "genuinely live terminal" positive path of `terminal_stream_targets_a_live_terminal` is untested from `tekstide`'s own suite** — `tekstide_core::AppState::project_mut` is `#[cfg(test)]`-gated to `tekstide-core`'s own crate boundary, and no production call site attaches a `TerminalSession` anywhere in the tree yet (RFC-017's job). Only the negative (not-live) path is proven today; recorded as a real gap, not hidden behind a fixture that only exercises the easy half.
- **No post-dismissal keystroke queuing is tested** — the current subscription design has no queue to test draining from (each `subscription()` call is stateless), so "no post-dismissal delivery" is a property of the design shape rather than something separately verified by a test.
- **`xdotool windowactivate` does not work under this niri/XWayland setup** (`_NET_ACTIVE_WINDOW` unanswered); `xdotool windowfocus` does. Recorded for the next slice needing synthetic input (PR-015-E onward) so it isn't rediscovered.
- `SurfaceInput`'s payload has no real consumer yet (PR-015-D); `shell::update` receives and discards it, proven only at the routing layer (`input::tests`).
- `format_binding`'s modifier ordering (Ctrl, Alt, Shift) matches the two bindings `KeybindingPolicy::linux_mvp()` currently ships; a future binding combining modifiers in an order this function doesn't anticipate would need the function extended, not the policy.
- **Response 130 Recommended, closed: `format_binding` only handles `Key::Character`.** A `KeybindingPolicy` rule bound to a named key (`F1`, `Escape`, `Delete`) would make `matching_global_action` return `None` for it, silently falling through to `SurfaceInput` and quietly defeating "global keybindings are not capturable by a surface" for that one binding — no live gap today (both real bindings are single-character), but a real one the day a named-key binding is added. `input::tests::every_default_binding_in_linux_mvp_round_trips_through_format_binding` now asserts every real `default_binding` round-trips through `format_binding`; ablation-verified by temporarily giving `OpenSafeCloseDialog` a `Some("F1")` binding in `tekstide-core` — the test failed with a clear message pointing at the unhandled named key — then reverted (`tekstide-core` untouched in the committed diff).
