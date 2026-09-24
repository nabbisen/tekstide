---
title: "RFC-054 — QA evidence"
rfc: "RFC-054"
created: "2026-09-24"
---

# QA evidence

## PR-054-A — keybindings, and the status that meant two things

Commits `98fbb1c` (the type repair) and `b8642f1` (the file, the resolution, the wiring), plus the docs commit.
Captures: `evidence/01-…`, `02-…` — the release binary, `XDG_CONFIG_HOME` and `XDG_STATE_HOME` both a fresh
`mktemp -d`, no path under `$HOME` on screen.

### What changed

| | Change | Where |
| --- | --- | --- |
| Repair | `KeybindingStatus` is `Reserved`, `Bound` or `Dead`, **derived** from a `RuleBinding` that is exactly one of a held chord, a default chord, or a death certificate. `Candidate` and `Configurable` are gone; the fields are private and the only constructors are `reserved`, `bound` and `dead`. | `navigation.rs` |
| Decisions | `CycleVisibleTerminalSession` and `OpenSafeCloseDialog` are **`Dead`**, each with its reachability written out (below). | `navigation.rs` |
| Grammar | `Chord`: modifiers in the order `Ctrl`, `Alt`, `Shift`, then one `A`-`Z`/`0`-`9`; needs `Ctrl` or `Alt`; no `Shift` with a digit. Case-insensitive in, canonical out. | `navigation/chord.rs` |
| Names | `NavigationAction::config_name()` (an exhaustive match) and `ALL`. | `navigation.rs` |
| Resolution | `KeybindingPolicy::with_overrides(&[(action, chord)]) -> { policy, refusals }`. One place that knows the rules, used by the parse and by every reload. | `navigation.rs` |
| File | `[keybindings]` is read, per entry; a bad one is a `SettingFallback`, not a refusal of the file. | `config/load.rs`, `model.rs` |
| Live | The policy in force is `ConfigurationState::keybinding_policy`, rebuilt on load and on `Ctrl+Alt+C`; routing, the Help modal and the subscription read it. | `shell.rs`, `input.rs`, `keyboard_help.rs` |
| Board | One line per fallback: the setting and a closed-set reason from the catalog. | `shell.rs`, `en.ftl` |

### D3′ — the two dead actions, decided

Both have **no handler anywhere in the code** (grepped: the only references are the exhaustive matches that
map them to `None`). Giving either a chord would have been a chord that does nothing, or a new action (§7),
so each is a death certificate with the reachability stated:

- **`CycleVisibleTerminalSession`** — *no handler; a new launch becomes the Primary session, the only one that
  receives keystrokes, and there is no way to bring an existing session back to Primary.* Switching sessions
  is a new action with its own design.
- **`OpenSafeCloseDialog`** — *no handler and none needed: the close-project dialog is reached from a project
  tab's close button and from `Delete` on a focused tab.* A third route was not built.

### The rebindable set

Fifteen actions are `Bound` (rebindable); one is `Reserved` (`open_command_palette`, `Ctrl+Shift+P`); two are
`Dead`. The book's table of rebindable actions is **checked against the policy** by
`the_configuration_page_lists_every_rebindable_action_with_its_default_chord`, so it cannot go stale.

### Resolution, and why it is not "the later one loses"

A collision refuses **every** rebind that takes part in it, so the outcome does not depend on the order things
were written in (`two_rebinds_to_one_chord_are_both_refused_whatever_the_order` compares the resolved policies
for both orders). Refusing one can restore a default another rebind now collides with, so the resolution
repeats until nothing collides (`refusing_one_rebind_can_refuse_another_…` asserts no chord reaches two
actions afterwards). A swap is not a collision and is accepted.

### Live (`evidence/01-…`, `02-…`)

A config file with three entries: `open_help = "ctrl+alt+j"` (valid), `open_folder_browser = "nonsense"` and
`open_diff_review = "Ctrl+Shift+P"`. **01**: the board lists the two refused settings — *"…was not used, so its
default stands. It has a part that is not Ctrl, Alt, Shift or a single letter or digit."* and *"…That chord is
reserved for open_command_palette."* — and neither echoes what was written. **02**: `Ctrl+Alt+J`, pressed
through the focus-verified helper, opens the Keyboard reference, which lists **`Ctrl+Alt+J — This list`** and
`Ctrl+Alt+B — Browse for a project folder` (its default, standing). After Escape, the **old** chord
`Ctrl+Alt+K` was pressed: the screen is unchanged (byte-identical capture to 01), so the chord no longer
reaches Help. That third capture is not committed; it is identical to `01-`.

### Ablations (committed tree, `rfcs/handoffs/ablate.sh`, one file restored each)

| Removed | Failed |
| --- | --- |
| Collisions never detected (last-wins) | 5: the three navigation collision tests and two config tests that name the collision |
| The reserved-chord check | `a_reserved_action_a_reserved_chord_…` and `every_way_a_rebind_can_be_refused_…`, and the shell reload test |
| Routing reads the default chord, not the effective one | `a_rebound_chord_routes_to_its_action_and_the_old_chord_no_longer_does` and the terminal-precedence test |
| `std::env::current_dir()` added to the config module | `the_configuration_module_has_no_way_to_look_beside_a_project` |

The checklist's own wording applies: each fails its own test, and the list above names what shares its fixture.

### §1, and what its test can and cannot show

`a_config_toml_in_the_project_is_never_read` puts a hostile `config.toml` beside the project root **and** in
`.tekstide/`, opens the project, and asserts nothing of it is in force and the resolved path is not under the
project. **That test would also pass if the loader never looked anywhere near a project, which is the point;
it cannot fail on its own for a reader that has to be *added*.** So the structural half —
`the_configuration_module_has_no_way_to_look_beside_a_project` scans the module for `current_dir`,
`ProjectRootHandle`, `project_root`, `canonical_root_path` — is what the ablation trips. It is a scan, not a
proof: a loader given a project path by a *caller* would not mention those words, but the configuration module
takes only the path `ConfigPathResolver` returns, and a new parameter would be visible in the diff.

### §4, §5, §6

§4: refuse-and-default, never last-wins (above). §5: `no_constructor_or_public_field_can_build_a_rule_that_is_both_bound_and_dead`
asserts the fields are private and the old `new(action, Option<_>, status)` shape is gone (a scan, because a
compile-fail test needs a dependency this project does not have), and `every_rule_is_exactly_one_of_…` asserts
the derived status, chord and certificate can only agree. §6: a fallback carries a **closed reason and a name
this crate defines**, never the value; the one place file text reaches a diagnostic is an unknown action name,
which is a warning through `bound_key_segment` (length-capped, escaped), pinned with a bidi override in
`an_unknown_action_warns_bounded_and_a_hostile_value_is_never_echoed`.

### Judgment calls, disclosed

- **A new diagnostic type, not `ConfigDiagnostic`.** That type is content-free by construction
  (`message: &'static str`) and whole-file, which is right for a file that cannot be trusted at all. D4 needs
  per-setting fallback and B needs a *number* (the contrast ratio), so `SettingFallback` is a closed set of
  typed reasons rendered through the catalog. B and C add their variants to it.
- **`--help` prints the shipped defaults.** It runs before the configuration is read; the Help modal prints
  the chords in force. The page and the help both say so. Loading the file for `--help` was more change than
  this slice should carry.
- **The chord grammar is narrower than a keyboard** (letters and digits only; `Ctrl` or `Alt` required; no
  `Shift` + digit). Each restriction stops a rebind that would either take a key from typing or never match.
- **Chords are matched case-insensitively and rendered canonically**, so `ctrl+alt+j` works and the Help
  modal never shows a second spelling.
- **The first parse of `Reserved` chords is by string.** `with_overrides` compares the requested chord's
  rendering with each reserved rule's chord string; there is one reserved chord today, and the comparison is
  exact because both sides are canonical.
