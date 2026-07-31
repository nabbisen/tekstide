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

Pending implementation.

**Reminder:** this is the security-critical slice. The test of correct structure is that *deleting a guard condition produces a compile error, not a security regression*.

### PR-015-D — Project Board surface

Pending implementation.

### PR-015-E — Mode switching and Content-mode scaffolding

Pending implementation.

### PR-015-F — Measurement: R1 discharge

Pending implementation.

### PR-015-G — Closeout evidence

Pending implementation.

## Known Limitations

- Screen-reader support absent for the life of the `iced` substrate decision (RFC-014 R2, owner-accepted).
- Terminal rendering, editor, explorer, diff, and report surfaces are out of scope; the shell provides only the contract they plug into.
- The modal layer ships with a placeholder dialog for testing; real dialogs are RFC-022.
