# RFC-054: User Configuration Completion

Status: **Implemented and closed 2026-09-24, and M12 closes with it.** Keybindings, theme colours, font family and size, and terminal scrollback are things a user sets; the status that meant two things is repaired. Accepted by the human owner 2026-09-24.** **D1, D2, D4, D6–D9 as written; D3 corrected on acceptance — see D3′, which repairs a category error this project recorded and never fixed.** Proposed 2026-09-24. `0.25.0`, and **M12 closes with it** — M12's own scope names
"keybindings, theme, fonts, terminal scrollback, resource limits, and AI CLI profiles as user
configuration", and three of those six have never shipped.

## Summary

Sixteen releases have added surfaces. **None has added a setting.** `REQ-CONFIG-006`,
`REQ-CONFIG-007`, `NFR-UX-004` and `REQ-TERM-004` have been open since the requirements were written,
and the 2026-09-23 audit found them recorded as implemented when they were not. This RFC makes
keybindings, theme colours, font family, font size and terminal scrollback things a user can set —
and nothing else change.

## What is true today, measured

| | Measured |
| --- | --- |
| 1 | The configuration document has exactly two sections: `agent` (`default_profile`, `transcript_retention_days`, `profiles`) and `resources` (`agent_run_limit`). **Nothing about the UI.** |
| 2 | **No project-local configuration is read, anywhere.** The path is `$XDG_CONFIG_HOME/tekstide` or `~/.config/tekstide` (platform variants in `config/path.rs`). `REQ-CLI-005`'s "workspace-provided configuration" threat is closed *by construction*, not by a check. |
| 3 | `Theme` is already one seam: `background()`, `foreground()`, `accent()`, `border_default()`, `border_focused()`, `surface_elevated()`, `scrim()`, `font_size_body/heading/status()`, with `Color::from_rgb` constants behind them. |
| 4 | **`KeybindingRule` already carries `status: Reserved \| Candidate \| Configurable`.** The model anticipated this RFC; nothing reads the field for configuration yet. |
| 5 | `SCROLLBACK_LINES = 2_000`, a constant in the terminal surface. |
| 6 | The configuration system already has the behaviour this RFC needs for bad input: an unknown or refused key is **named on the Project Board** and the default stands (RFC-045). |

## Decisions required

**D1 — Five things become settable, and nothing else.** Keybindings, theme colours, font family, font
size, terminal scrollback. `REQ-CONFIG-006`, `007`, `NFR-UX-004`, `REQ-TERM-004`.

**D2 — Where configuration comes from does not change.** The user's own config directory, and
nothing else. **No project-local file, ever** — a repository must never be able to rebind a key,
restyle the window, or name a font. Today that holds because no such path exists; this RFC **pins it
with a test**, because it is now worth attacking.

**D3 — Keybindings: only `Configurable` rules, and a conflict refuses.** A rebind naming a
`Reserved` action, or a chord another rule already holds, **refuses with a diagnostic and the default
stands** — never last-wins, never silently unreachable. Rebinding changes *which chord* reaches an
action; it never changes the global-precedence model or modal suppression (RFC-015).

**D4 — A bad value falls back per setting, with the board saying which and why.** This is what
RFC-045 already does for unknown keys, and it is better than refusing a whole file for one colour.
Recommended, and it is what makes D5 and D6 safe.

**D5 — Colour is validated against contrast, not trusted.** `NFR-UX-003` asks for sufficient contrast
for prolonged use. A foreground/background pair below the threshold **falls back to the default pair
and says so** — a user cannot configure the window into unreadability, and we do not silently accept
a setting that makes the product unusable.

**D6 — A font is named, never a path.** A family name resolved through the system font database, not
a file this application then parses. **A font file named in configuration is a parser eating
user-named bytes at startup**, and this project does not open attacker-influenced files without a
reason. An unavailable family falls back and says so. Size is bounded (recommended 8–32 px) with the
same fallback.

**D7 — Scrollback is bounded.** `REQ-TERM-004` asks for configurable; `NFR-RES-003`/`004` bound it,
because it is memory per pane and a hidden pane keeps filling. The bound is stated in the book, and a
value above it clamps with a diagnostic rather than being honoured.

**D8 — `Ctrl+Alt+C` applies all five live.** The reload path already exists. `REQ-CONFIG-005` holds:
a reload must never silently weaken a security-sensitive setting, and **`Reserved` stays reserved
across a reload** — that is the one keybinding property that is security-shaped.

**D9 — Configuration changes values, never structure.** No new actions, no new surfaces, no new
lifetimes, no colour carrying meaning alone. A theme may restyle a badge; it may not remove the word
inside it.

## Non-goals

A configuration UI (this is a file), per-project settings, theme *files* or a theme gallery, icon
fonts (RFC-052 D3 rejected the dependency), and any setting that changes what the product does rather
than how it looks or which key reaches it.

## Risks

- **A user configures the window into unusability.** D5 and D6's bounds and the per-setting fallback
  exist for this; the escape hatch is that a bad value never persists into a state you cannot read.
- **A keybinding becomes unreachable** through a conflict the user did not notice. D3 refuses rather
  than accepts.
- **A font family name is still input.** It reaches a font database, not our parser — but the
  diagnostic must not echo it unescaped (`text_safety::quote_untrusted`, as everywhere else).
- **Scope creep into a settings UI.** The requirement is a file. It stays a file.

## Acceptance criteria

- A configuration file setting all five takes effect **after `Ctrl+Alt+C`**, shown in a live capture.
- **A `config.toml` in the project root is never read** — pinned by a test, with an ablation that the
  test fails if the loader ever looks there.
- A low-contrast pair, an unavailable font, an out-of-range size, an over-bound scrollback and a
  conflicting or reserved keybinding each **fall back with a diagnostic naming the setting**, and the
  product stays usable.
- The i18n completeness and colour-alone scans still pass, and no diagnostic echoes a configured
  string unescaped.
- `REQ-CONFIG-006`, `REQ-CONFIG-007`, `NFR-UX-004` and `REQ-TERM-004` move to implemented **with the
  evidence that they are reachable**, not merely parsed.


## Decided on acceptance (2026-09-24)

**D3 was wrong, and the project had already written down why.** I wrote "only `Configurable` rules
may be rebound" from the identifier's name. Measured:

```
Reserved      Ctrl+Shift+P   OpenCommandPalette
Candidate     Ctrl+Alt+P …   sixteen actions with real chords
Configurable  (none)         CycleVisibleTerminalSession, OpenSafeCloseDialog
```

`future-work.md` records the trap in its own words: **`Configurable` with a `None` binding *reads* as
"a user can bind this" and means "dead until RFC-023 exists"**, and it instructs the keybinding pass
to *"either give each action a default or mark it explicitly dead; the current state makes the two
indistinguishable at a glance, which is exactly how both misses above happened."* Two surfaces
shipped unreachable because of it (RFC-022's Approval History, RFC-032's Trust Settings).

**D3′ — the rebindable set, and the repair.**

1. **Rebindable = has a default binding and is not `Reserved`.** That is the set a user can see. The
   two `Configurable`/`None` rules are *dead*, not configurable, and rebinding them would configure
   nothing.
2. **`Reserved` stays unrebindable** — `Ctrl+Shift+P` belongs to a command palette that does not
   exist, and a user binding something to it would lose it the day the palette lands.
3. **A conflict or a reserved chord refuses, with a diagnostic, and the default stands.** Never
   last-wins, never silently unreachable.
4. **This RFC is the keybinding pass `future-work.md` asked for**, so it carries the repair: every
   action ends with either a real default binding or an **explicitly dead** status, and the two
   states are distinguishable by the type rather than by reading a `None`. Whether
   `CycleVisibleTerminalSession` and `OpenSafeCloseDialog` get a chord or a death certificate is the
   slice's call, per action, with its reachability stated either way.

**D5 — the threshold is 4.5:1**, WCAG AA for body text, computed from relative luminance. Below it,
the pair falls back to the default and the diagnostic names the **measured ratio**, not just the
setting — a number a user can act on.

**D6 — font size 8–32 px**, outside that the default stands with a diagnostic. The family is a name
resolved by the system font database; an unavailable family falls back and says so.

**D7 — the scrollback cap is chosen by measurement against a stated budget**, not picked: the slice
measures bytes per line at a realistic width and sets the cap so that **one pane at the cap stays
under 64 MB**, with the number and the measurement in the book. `NFR-RES-003`/`004` are why there is
a cap at all; a hidden pane keeps filling.

**D8′ — a chord is spelled the way the product already prints it.** The Help modal and `--help` show
`Ctrl+Alt+P`; that exact spelling is what a user types in the file, round-tripped by a test over
every advertised binding. No second grammar to learn, and no chance of the file and the help
disagreeing.


## Closed (2026-09-24) — and M12 with it

Three slices. **The sixteenth release added surfaces; the seventeenth adds settings.**

**D3′ repaired a category error the project had carried since RFC-022.** `KeybindingStatus::Configurable`
with a `None` binding read as *bindable* and meant *dead*; two surfaces shipped unreachable because of
it. `RuleBinding` is now a three-variant sum with a private field, so *bound and dead* is
unrepresentable, and `Dead` carries a **written reachability claim** pinned by a test. That claim
immediately surfaced a product fact nobody had written down: a project can hold several terminals and
**only the `Primary` one receives keystrokes**.

**The scrollback cap was measured, and the measurement was right where its author was not.** An
allocator-counting probe found the first numbers included the feeding buffer; that rows are allocated
in blocks of 1,024 rather than per line; and that the block count is not `ceil(lines/1024)` — a
6,143-line pane held seven blocks. **`a_pane_at_the_cap_stays_under_the_memory_budget` failed at
67,924,707 bytes against 67,108,864**: the measurement caught the cap, not the review. The cap is
12,000 lines, and **a wider pane keeps fewer**, recomputed on resize, because otherwise the 64 MiB
claim is false for anyone with a wide display.

**Two defects that only a live capture could find**, continuing this project's run: the Help modal's
binding column was a fixed 110 px and ran into its descriptions at a serif 17 px; and buttons did not
follow the theme, which made `theme.rs`'s own claim false.

**An ablation that failed nothing was treated as a finding** — `theme::button` not applying its style
broke no test until one was written for it.

**Left open, deliberately**: at 12,000 lines a pane whose output puts a combining character on every
cell costs about four times as much, so the 64 MiB budget is about ordinary output. Ruled at review
429: the cap stands, the book states the worst case in bytes rather than a multiplier, and **bounding
combining marks per cell is its own slice (RFC-061)** on the terminal boundary's ground — where the
bound must be chosen against real Devanagari, Arabic, Hebrew and Vietnamese, not against Zalgo.
