# RFC-054: User Configuration Completion

Status: **Proposed 2026-09-24.** `0.25.0`, and **M12 closes with it** — M12's own scope names
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
