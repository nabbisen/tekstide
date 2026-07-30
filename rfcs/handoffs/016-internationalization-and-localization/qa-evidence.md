# RFC-016: Internationalization and Localization - QA Evidence

Status: Proposed — implementation in progress (PR-016-C landed 2026-07-30, not yet reviewed; PR-016-B/D/E/F pending)
Date opened: 2026-07-29
Date accepted: Pending

## Scope

RFC-016 delivers the localization machinery and the text-safety rules for untrusted text rendered in trusted surfaces.

Evidence in this file must not be used to claim confusable/homoglyph protection, RTL layout mirroring, runtime locale switching, terminal-grid bidi reordering, terminal wide-cell CJK, or completed translations into any specific language — unless later reviewed implementation explicitly supports that claim.

## The security finding this RFC exists to fix

Verified 2026-07-29, before implementation:

- The RFC-009 `SecurityFilter` forwards `input(c: char)` unconditionally. Bidi controls are printable Unicode, not C0/C1, so they were never part of the accepted/inert classification.
- `tekstide-core`'s `TerminalSecurityParser` contains no bidi handling; U+202A..U+202E and U+2066..U+2069 pass through as ordinary text.
- RFC-014 C10 demonstrated the editor surface performs full Unicode bidi reordering via `cosmic-text`.

Consequence: a command string containing U+202E would render reversed in RFC-021's approval dialog — Trojan Source (CVE-2021-42574) applied to the surface whose entire purpose is showing the user exactly what will run. PR-016-C closes this.

## Design Review

Pending PR-016-A acceptance.

## Implementation Evidence

### PR-016-B — Catalog, locale selection, fallback

Pending implementation.

### PR-016-C — Text safety: escape and isolate

**Module:** `crates/tekstide-core/src/text_safety.rs` (+ `text_safety/tests.rs`), newly registered in `lib.rs`. Public API: `is_format_char`, `is_untrusted_display_control`, `escape_untrusted_chars(text: &str) -> String`, `DisplayText` (opaque wrapper, no public constructor other than below), `quote_untrusted(text: &str) -> DisplayText`.

**Consolidation, not new invention.** The character-escaping policy (Unicode `Cc`+`Cf` categories → visible `<U+XXXX>` markers) already existed, implemented ad hoc inside `approval::coordinator` under RFC-021 PR-021-E1/response-115 review pressure, before this RFC existed to own it. RFC-016's own README (2026-07-30 addendum) forced the answer to Open Question 1: the function lives in `tekstide-core`, shared, not shell-local. This slice: (1) moved `is_format_char`/`is_escaped_control` (renamed `is_untrusted_display_control`) and the `<U+XXXX>` substitution loop out of `approval/coordinator.rs` verbatim into `text_safety.rs`; (2) rewired `approval::coordinator::display_entry`/`needs_quoting` to call `crate::text_safety::escape_untrusted_chars`/`is_untrusted_display_control` instead of a private copy; (3) added the whole-string API (`quote_untrusted`/`DisplayText`) that `implementation-handoff.md` specifies, for future single-value untrusted-text surfaces (Project Board rows, notifications, viewers) that have no argv-shaped per-entry quoting need.

**Split between the shared module and `approval::coordinator` is deliberate, per the README's explicit carve-out:** the character-escaping half (`Cc`/`Cf` → `<U+XXXX>`) is text-shaped and must not be duplicated, so it moved. The per-entry POSIX-style quoting (`SHELL_METACHARACTERS`, `needs_quoting`'s whitespace/metacharacter checks, single-quote wrapping) is argv-shaped — "argv is a vector of entries," not "text contains untrusted characters" — and has no equivalent in `text_safety`'s single-string API, so it stayed in `approval::coordinator`.

**Zero behavior change confirmed.** Every pre-existing `approval::tests::coordinator::display_command_*` test (6 tests, including the response-115 ten-codepoint probe) passes unchanged after the refactor — same inputs, same outputs, only the implementation moved. Ablation-verified the wiring is real, not a leftover stale copy: temporarily broke `text_safety::is_untrusted_display_control` (`|_| false`) and confirmed both the new `text_safety` tests **and** all pre-existing `approval::tests::coordinator::display_command_*` bidi/control tests failed identically (5 failures across both modules), then restored — proving `approval::coordinator` now genuinely depends on the shared implementation rather than retaining a divergent local one.

**Isolation (`quote_untrusted`).** Wraps the escaped text in Unicode bidi isolate marks: First Strong Isolate (U+2068) ... Pop Directional Isolate (U+2069). Since escaping already converts every live directionality/format control (including U+2068/U+2069 themselves, if present in the untrusted input) into an inert visible marker before the wrap is added, the isolate wrapping is a second, structural layer of defense — RFC-016 §Security point 2 read literally ("the span is directionally isolated ... regardless of what it contains"), not a substitute for point 1.

**Type-safety choice, per `implementation-handoff.md`'s "prefer that" guidance:** `DisplayText` has no public constructor other than `quote_untrusted` — a future widget API that requires `DisplayText` rather than `&str` cannot be handed raw untrusted text at all, mirroring the same "parse, don't validate" reasoning RFC-015 applies to its input classes and RFC-021 applies to `VerifiedCwd`.

**Tests (`text_safety/tests.rs`, 7 new):**
- `every_bidi_and_format_probe_escapes_to_its_visible_marker` — carries over the exact ten-codepoint set from `approval::tests::coordinator` (U+202E, U+2066, U+200E, U+200F, U+061C, U+200B, U+200C, U+200D, U+00AD, U+FEFF), per the README's explicit instruction, so the canonical implementation is tested at least as hard as the copy it replaces.
- `the_full_isolate_initiator_and_terminator_range_is_escaped` — the exact `U+2066..U+2069` range the checklist names, including U+2069 (PDI) specifically: an attacker-supplied PDI inside untrusted text is escaped to a literal marker and cannot be mistaken for, or prematurely close, the isolate `quote_untrusted` itself adds.
- `the_trojan_source_pattern_is_defeated` — a string built exactly like the classic Trojan Source exploit (`good` + RLO + `exe.`); confirms the RLO survives only as an inert `<U+202E>` marker and the stripped-of-wrapper character sequence matches the logical input order exactly, character for character.
- `an_unterminated_override_cannot_affect_adjacent_trusted_text` — an untrusted span with an RLO and no matching PDF, composed inline between two trusted labels; confirms no live bidi/format control survives anywhere in the composed string except the two isolate marks this module itself added, and that the trusted labels are untouched. **Honestly scoped:** this checks structural properties (marker substitution, isolate-mark presence), not a full Unicode Bidi Algorithm resolution — this crate does not implement one, and does not need to, since escaping removes every live directional instruction before isolation is even relevant.
- `legitimate_rtl_letters_are_not_escaped` — genuine Arabic letters (not bidi *control* characters) pass through byte-for-byte unescaped, directly proving the over-escaping risk RFC-016 §Risks flags is not realized.
- `escaping_does_not_mutate_the_caller_s_original_value` — both functions take `&str`, never `&mut str`; explicit regression guard against a future signature change reintroducing ingest-time mutation.
- `an_empty_string_does_not_panic_and_produces_the_bare_isolate_wrapper` — an empty untrusted field is a legitimate input, not a special case requiring different handling.

**Byte fidelity, honestly scoped.** Verified structurally (both functions are pure `&str -> String`/`DisplayText`, no caller value is ever mutated) and verified against the one real integration point that exists today: `approval::coordinator`'s pre-existing `sentinel_command_text_never_reaches_the_durable_audit_store` (unchanged by this refactor) proves neither raw command text nor the escaped display string reaches the durable audit store. **Not yet verified against an RFC-011 transcript path**, because no transcript code calls into `text_safety` at all — there is no integration point to test yet. This is recorded as an honest gap, not silently assumed.

**What this slice does NOT do, and why — RFC-015 has not been implemented.** `crates/tekstide/src` contains only `main.rs`; there is no application shell, no dialog surface, no Project Board row rendering, no notification surface, no audit or transcript viewer, and no editor surface anywhere in the tree. RFC-016 §Security's "where this applies" table (approval/trust/paste/destructive/safe-close dialogs, Project Board rows, notifications, viewers) and the editor/terminal exceptions therefore have **nothing to apply `quote_untrusted` to yet** — every one of those checklist lines is left unchecked below, not on a technicality but because the surfaces genuinely do not exist. `approval::coordinator`'s argv rendering is the one real, live call site, and it already used (now shares) this exact policy before this slice began. This mirrors RFC-021's "headless" precedent: the primitive is built, tested, and reviewable in isolation; wiring it into real surfaces is necessarily deferred to whichever slice first builds each surface (RFC-015's shell, RFC-022's approval dialog, and so on).

Gates run 2026-07-30: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (478 `tekstide-core` — up from 471, 7 net new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed. The `approval::coordinator` wiring ablation-verified as described above.

### PR-016-D — Pluralization and interpolation

Pending implementation.

### PR-016-E — Enforcement

Pending implementation.

### PR-016-F — Closeout evidence

Pending implementation.

## Known Limitations

- **Confusable and homoglyph attacks are not addressed.** Full detection is heavy and error-prone; a partial implementation would imply a guarantee that does not exist. A later RFC may revisit if command approval proves to need it.
- **RTL layout mirroring is deferred.** Text renders correctly RTL; surrounding layout stays LTR.
- **Terminal-grid bidi reordering is out of scope by design** — real terminals do not implement it, and reordering would break cursor and column arithmetic.
- **Terminal wide-cell CJK is a genuine gap owned by RFC-017**, not by this RFC.
- Runtime locale switching is out of scope for M8; selection resolves once at startup.
- **`text_safety::quote_untrusted` has no real caller yet beyond `approval::coordinator`'s (pre-existing, argv-shaped) usage of the character-escaping half.** RFC-015's shell has not been implemented (`crates/tekstide/src` contains only `main.rs`), so there is no dialog, Project Board row, notification, audit viewer, transcript viewer, or editor surface in the tree to apply the whole-string API to. The primitive is built and tested in isolation, matching RFC-021's "headless" precedent; every "applied to: <surface>" line in the acceptance checklist is unchecked for this reason, not because the primitive is unproven.
- **Byte fidelity is verified against the audit path (`approval::coordinator`'s existing sentinel test) but not against an RFC-011 transcript path** — no transcript code calls into `text_safety` yet, so there is no integration point to test.
- **The terminal-surface "no escaping, no reordering" exception is not something this slice implemented** — it is the terminal's pre-existing behavior (no bidi handling exists there at all, confirmed by RFC-014 C10 and `tekstide-gui-spike`'s filter tests), not a deliberate opt-out of a shared render path the terminal would otherwise go through. Recorded as correct-by-absence rather than claimed as an implemented, documented exception under this RFC.
