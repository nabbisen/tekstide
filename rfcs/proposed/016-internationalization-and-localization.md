# RFC-016: Internationalization and Localization

Status: Proposed
Target milestone: M8
Date: 2026-07-29

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- Project rules (`project-instructions-rust-gui.md`) — *"The GUI must support multiple languages (i18n)."*
- [`ROADMAP.md`](../../ROADMAP.md), [`delivery-plan.md`](../delivery-plan.md)

Depends on:

- [RFC-009](../done/009-terminal-security-boundary.md) — terminal output is untrusted
- [RFC-014](./014-desktop-gui-substrate-and-terminal-rendering.md) — substrate; C10 established the editor/terminal bidi asymmetry
- [RFC-015](./015-application-shell-and-rendered-surface-model.md) — creates the i18n seam this RFC fills

Blocks:

- localized release of any rendered surface;
- RFC-021/RFC-022 approval dialog text-safety guarantees (see §Security).

## Summary

RFC-016 fills the i18n seam RFC-015 creates: catalog format, locale selection and fallback, pluralization, interpolation, and RTL policy.

It also resolves a security problem that only becomes visible once text rendering is taken seriously. **Bidirectional control characters are currently handled nowhere in Tekstide**, and the substrate honours them. A command string containing U+202E renders reversed. RFC-021's approval dialog exists to show the user *exactly* what an AI CLI proposes to run — and that guarantee does not survive a text renderer that obeys attacker-supplied directionality overrides.

Localization and text safety are the same subject, so this RFC owns both.

## Motivation

Project rules make multi-language support mandatory, not optional. RFC-015 mandates that no user-facing string is hardcoded, precisely so this RFC can be implemented without retrofitting extraction across a built UI.

The security half is newly discovered and independently urgent:

**Verified facts (2026-07-29):**

- The RFC-009 `SecurityFilter` forwards `input(c: char)` unconditionally. Bidi controls are printable Unicode, not C0/C1, so they are not part of the accepted/inert classification at all.
- `tekstide-core`'s `TerminalSecurityParser` contains no bidi handling — U+202A..U+202E and U+2066..U+2069 pass through as ordinary text.
- RFC-014 C10 demonstrated that the editor surface performs **full Unicode bidi reordering** via `cosmic-text`.

Together: attacker-influenced text is rendered by a renderer that obeys embedded directionality overrides, in surfaces whose entire purpose is showing the user what is true. This is Trojan Source (CVE-2021-42574) applied to a command-approval dialog.

## Goals

- Define the catalog format, locale selection, and fallback chain.
- Define pluralization and interpolation.
- Define RTL policy, including the editor/terminal asymmetry RFC-014 recorded.
- **Define text-safety rules for untrusted text rendered in trusted surfaces.**
- Enforce the no-hardcoded-strings rule mechanically rather than by review attention.
- Keep the localization dependency proportionate to the project's dependency-consciousness.

## Non-Goals

- Translating into any specific language. This RFC delivers the machinery and the source locale; actual translations are content work.
- Localizing untrusted or machine data — see §What Is Never Localized.
- Full Unicode confusable/homoglyph detection. Recorded as a residual limitation.
- Locale-aware collation, calendars, or currency. Tekstide has no such surfaces.
- Terminal-grid bidi reordering. Out of scope by design — see §RTL Policy.
- Configuration-file locale override. RFC-023 supplies it through the same selection API.

## Design Principles

1. **Localization and text safety are one subject.** Both are "what does the user actually see, and can it be trusted?"
2. **Fail visible, never blank.** A missing translation shows something useful. A UI that silently renders empty strings is worse than one showing an untranslated key.
3. **Untrusted text is quoted, never obeyed.** Attacker-influenced text may be *displayed*; it may never change how surrounding text renders.
4. **Data is not UI text.** Commands, paths, ids, and code are not translated and not reflowed.

## Security: bidi and untrusted text in trusted surfaces

This section is normative and binding on RFC-021 and RFC-022.

### The threat

An approval dialog shows an adapter-proposed command. If that string may contain bidi controls, the rendered display can differ arbitrarily from the argv that will execute. The user approves what they read; the system runs what was sent.

UI/UX §12.3 already requires that *"hidden control characters must be made visible or removed."* This RFC makes that concrete and extends it, because "removed" alone is insufficient — silently stripping also produces a display that differs from the real value.

### Policy: escape and isolate

For any **untrusted text rendered in a trusted surface** — approval dialogs, paste-confirmation dialogs, trust prompts, safe-close summaries, notifications, Project Board rows, audit and transcript viewers:

1. **Bidi controls are rendered visibly, not obeyed.** U+202A..U+202E and U+2066..U+2069 are escaped to a visible representation (for example `<U+202E>`), never passed to the shaper as directionality instructions.
2. **The span is directionally isolated.** The whole untrusted run is wrapped in isolate marks so its content cannot alter the directionality of surrounding trusted chrome, regardless of what it contains.
3. **Other invisible or format characters are made visible** on the same principle: zero-width joiners/non-joiners, zero-width space, soft hyphen, and unassigned-but-rendering-invisible codepoints.
4. **Escaping happens at the render boundary**, not at ingest. The stored value stays exact — RFC-013's audit records and RFC-011's transcripts must keep byte fidelity. Only the display is transformed.

Point 4 matters: transforming at ingest would corrupt evidence. Transforming at render keeps the record true and the display honest.

### Where this applies

| Surface | Policy |
| --- | --- |
| Approval / trust / paste / destructive dialogs | **Escape and isolate.** Mandatory |
| Project Board rows (branch names, project names) | Escape and isolate — Git metadata is untrusted display text (threat model §8.10) |
| Notifications, audit viewer, transcript viewer | Escape and isolate |
| Editor surface | **Do not escape.** The user is editing real file content; they must see it as it is. Bidi reordering is correct behaviour here |
| Terminal surface | **Do not escape, do not reorder.** See §RTL Policy |

The editor exception is deliberate: an editor that silently rewrites file content is broken. A future editor-side "show invisibles" affordance is ordinary functionality, not a security control.

### Residual limitation

Confusable and homoglyph attacks — Cyrillic `а` for Latin `a` — are **not** addressed. Full confusable detection is heavy and error-prone, and a partial implementation would imply a guarantee that does not exist. Recorded as a known limitation; a later RFC may revisit it if command approval proves to need it.

## Catalog format and locale model

**Recommended: Fluent** (`fluent-bundle`), for these reasons:

- Native handling of plural categories, gendered forms, and interpolation without hand-rolled rules.
- Designed for exactly this problem, with an asymmetric-translation model where a translator can restructure a message without changing code.
- Rust-native and actively maintained.

**Dependency caveat.** RFC-013's T-033 and RFC-014's R3 both establish that this project weighs dependency surface deliberately. Fluent's cost must be measured at PR-016-B and recorded. If it proves disproportionate, the fallback is a plain key-value catalog (TOML) with an explicit plural-category function — less capable, materially lighter, and adequate for the source locale plus a small number of translations.

Catalogs are **compiled into the binary** for the source locale, so a missing or corrupt catalog file can never leave the application unusable. Additional locales may load from disk.

### Locale selection

Precedence, highest first:

1. Explicit CLI flag
2. Configuration setting (RFC-023 supplies it through this API)
3. OS locale
4. Source locale (`en`)

Selection is resolved once at startup. Runtime locale switching is out of scope for M8.

### Fallback chain

```
requested locale  →  requested language without region  →  source locale (en)  →  the key itself
```

Never blank. Never a panic. **A missing key renders the key**, which is ugly and immediately obvious — the correct failure mode for a translation gap.

Missing-key events are logged at debug level in development builds. They are not audit events; a missing translation is not a security decision.

## What is never localized

These are **data**, not interface text. Translating or reflowing them would corrupt meaning:

- Commands, argv, and command output
- Filesystem paths, project roots, file names
- Identifiers of any kind — project, terminal, AgentRun, approval, audit ids
- Terminal output and transcript content
- File contents in the editor
- Git branch names, commit messages, author strings
- Audit record field values and reason codes
- Configuration keys
- Log and diagnostic payloads

Labels *around* these values are localized; the values themselves never are.

## RTL policy

**Supported:** UI chrome, dialogs, Project Board, notifications, and the editor surface. `cosmic-text` provides shaping and bidi reordering, verified by RFC-014 C10 (Arabic rendered with correct right-to-left visual order and shaping).

**Not supported — and correctly so:** the terminal grid. RFC-014 C10 recorded that the terminal surface renders Arabic in raw cell order without reordering. This matches real terminal emulators, which generally do not implement bidi, and a terminal that reordered its grid would misrepresent cursor positions and column arithmetic.

**Known gap, owned by RFC-017:** the terminal grid lacks wide-cell CJK. `alacritty_terminal` supports it; the spike's minimal renderer did not consume it. CJK text is visually mangled in the terminal until RFC-017 addresses it.

Full UI mirroring for RTL locales — flipping layout direction — is **deferred**. Text renders correctly RTL; the surrounding layout stays LTR. Recorded as a limitation rather than claimed.

## Enforcement

The no-hardcoded-strings rule is only real if it is checked:

- A test scans shell-crate sources for string literals passed to widget text constructors, failing on anything not routed through the lookup.
- A catalog-completeness test asserts every key referenced in code exists in the source locale.
- An unused-key report is advisory, not a failure — keys legitimately outlive a UI revision.

Mechanical enforcement is required because a single hardcoded string is trivial to add, invisible in review, and expensive to find later.

## Data Model Impact

No `tekstide-core` changes. Localization is a presentation concern and lives in the shell crate, alongside the seam RFC-015 creates.

The text-safety escaping function is the one piece that may deserve a home in `tekstide-core`, since RFC-021's approval model and RFC-022's dialogs both need it and it is pure. Decide at PR-016-C.

## Implementation Plan

1. **PR-016-A** — design and handoff acceptance.
2. **PR-016-B** — catalog format decision with measured dependency cost, source-locale catalog, lookup implementation replacing RFC-015's placeholder, locale selection and fallback chain.
3. **PR-016-C** — **text-safety: escape-and-isolate for untrusted text in trusted surfaces.** Security-critical.
4. **PR-016-D** — pluralization and interpolation, with a second locale added purely to prove the machinery works.
5. **PR-016-E** — enforcement tests: no-hardcoded-strings scan, catalog completeness.
6. **PR-016-F** — closeout evidence.

PR-016-C may proceed independently of PR-016-B if scheduling favours it; the escaping function has no catalog dependency.

## Test and Evidence Requirements

- **Bidi corpus:** strings containing each of U+202A..U+202E and U+2066..U+2069, asserting that rendered output shows them escaped and that surrounding trusted chrome is unaffected. Include the Trojan Source pattern — a command whose displayed order differs from its logical order — and assert the display matches the logical argv.
- **Isolation test:** untrusted text containing an unterminated directionality override must not affect adjacent trusted labels.
- **Byte-fidelity test:** the stored value is unchanged; only the display is transformed. Assert against the audit and transcript paths.
- **Editor non-escaping test:** file content renders as-is, proving the exception is implemented deliberately.
- Fallback tests for missing locale, missing region, missing key — each renders something visible, never blank, never panicking.
- Plural-category tests for at least one language whose rules differ from English.
- Enforcement tests as described in §Enforcement.
- A screenshot of a non-Latin locale rendering the shell.

## Acceptance Criteria

- Catalog format chosen with measured dependency cost recorded.
- Locale selection and fallback implemented; missing keys render visibly, never blank.
- **Bidi controls in untrusted text are escaped and isolated in every trusted surface**, with the editor and terminal exceptions implemented deliberately and documented.
- Stored values keep byte fidelity; only display is transformed.
- Pluralization and interpolation work, proven with a second locale.
- No hardcoded user-facing strings, enforced mechanically.
- RTL text renders correctly in chrome and editor; terminal non-reordering and the wide-CJK gap are documented, not silently absent.

## Risks

- **Bidi escaping is applied inconsistently.** A single surface that forgets it reintroduces the vulnerability. Mitigation: escaping belongs to the shared untrusted-text render path, not to each surface; PR-016-C should make bypassing it require deliberate effort.
- **Over-escaping harms legitimate RTL users.** Genuine Arabic or Hebrew UI text must render naturally; only *untrusted* spans are escaped. Mitigation: the trusted/untrusted distinction is per-span, not global, and the test corpus must include legitimate RTL content rendering unescaped in chrome.
- **Fluent's dependency weight.** Mitigation: measure at PR-016-B; the lighter fallback is specified.
- **Enforcement decays.** A hardcoded string added under delivery pressure is invisible without a check. Mitigation: mechanical test, in CI once RFC-029 lands.
- **Confusables remain unaddressed.** Mitigation: recorded honestly; not claimed as covered.

## Open Questions

1. Should the escaping function live in `tekstide-core` (shared with RFC-021's approval model) or remain shell-local? PR-016-C decides with evidence.
2. Should the editor gain a "show invisibles" toggle in M8, or wait for RFC-019?
3. Which second locale proves the machinery — one with complex plural rules (Polish, Russian) or an RTL locale (Arabic)? Arabic exercises more of the risk surface; a Slavic language exercises plurals harder. Possibly both, if catalog cost is low.
