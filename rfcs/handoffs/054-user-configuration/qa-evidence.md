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

## PR-054-B — theme and fonts

### The caller pin required at review 427

`the_configuration_comes_from_the_resolved_user_path_and_no_other` (in `shell/tests.rs`) counts **production**
source — no `tests` directory, no `tests.rs`, cut at each file's `#[cfg(test)] mod tests`, comment lines dropped —
and asserts four things: `ConfigStore::load(` has **exactly one** caller; its argument is exactly
`storage_path.config_file().to_path_buf()`; inside that function `storage_path` is bound only from
`ConfigPathResolver.resolve(&provider)`; and `ConfigPathProvider::linux_default(` is called in exactly one
place, `main.rs`, with `linux_from_env(` in none. Ablations: a second caller handed `PathBuf::from("config.toml")`
fails it; handing the existing caller `config_file().parent().join("x.toml")` fails it.

It is still a source scan — a caller that built the path across a helper function would have to be a *new
caller*, which the count catches, or change the argument, which the text catches. It cannot see a
`config_file()` that has itself been made to return a project path; that lives in `ConfigPathProvider`, whose
only inputs are `XDG_CONFIG_HOME` and `HOME`.

### What was added

`[theme]` (seven roles, `#RRGGBB`, the scrim `#RRGGBBAA`) and `[font]` (`family`, `body_size`, `heading_size`,
`status_size`). The pure half is `tekstide-core/src/config/appearance.rs`; the shipped palette and sizes moved
there so **the palette a colour is measured against is the palette drawn when nothing is set** — `Theme::default`
is built from it, and `theme/tests.rs` still holds it to its own contrast claims. The contrast arithmetic also
moved there; `theme/contrast.rs` now calls it, so its known-anchor tests (including the `0.03928` boundary) hold
the shipped math, not a copy.

### D5 — the rule, precisely

Pairs measured at **4.5:1**: `foreground`/`background` and `foreground`/`surface_elevated` — the two surfaces text
sits on. A colour the user did not set is measured at its shipped value. When a pair fails, **every configured
member** goes back to its default, each with its own fallback naming the ratio and the role it was measured
against; then the palette is measured again, because taking one colour back can fail a pair it had passed (a
light `background` with a dark `foreground` both fail the *shipped* dark `surface_elevated`). Every round
removes a configured colour and the shipped palette passes, so it terminates.
`the_palette_that_comes_out_always_meets_the_minimum` states the result over 3,000 generated overrides: whatever
the input, the palette in force meets D5, every colour kept is one the user wrote, and every colour dropped was
reported.

The ratio in the diagnostic is **floored** to hundredths, so a miss can never print as a pass (`4.499` reads
`4.49`). The catalog receives it as three digits rather than a number, so a locale cannot reformat a
measurement.

### D6 — sizes and the family

Each size takes an integer or a float, is accepted on `8..=32` inclusive, and falls back alone; `nan` and `inf`
fall out because a range's `contains` is false for both. **The family is a name**: trimmed, 1–64 characters, no
control character, no `/` or `\`. Whether it is *installed* is asked of `iced_graphics`' global font system —
the database the renderer itself resolves against, so a `Some` is a family the window will find — by comparing
the name with the family names it holds. **Nothing is opened**: `no_shipped_code_loads_a_font` scans production
source for every font-loading entry point (`font::load(`, `load_font_file`, `Source::File`, …) and fails if any
appears. What comes back is the database's own spelling, interned so `Font::with_name` can have a `&'static
str`; the leak is bounded by the number of installed families, not by anything a user types.

### Live — a family applies without a restart

iced fixes its default font when the application is built and offers no way to change it, so a family that
applied only at startup would break D8. Every ordinary text widget is now built with `theme::text`, which sets
the configured face from one process-global that boot and every reload set. That is five files' `use` lines
changed (some 157 call sites, none edited): `no_shipped_view_builds_text_with_the_icedcrate_text_function` fails if any
file imports iced's own `text`, and `the_ui_font_is_set_at_boot_and_at_every_reload` fails if either setter is
removed. (A behavioural test of the global would race every other test that reloads, so the live capture is the
evidence that it works.)

Release binary, `mktemp -d` config and project under `/dev/shm`, focus-verified keystrokes:

- `03-default-dress-release.png` — no file.
- `04-ctrl-alt-c-applies-colours-family-and-sizes-release.png` — after writing `[theme]` (dark blue background,
  cream text, amber accent and focus border) and `[font]` (`DejaVu Serif`, 18/24/15) and pressing
  `Ctrl+Alt+C`: the same window, in the user's colours, in a serif face, larger. Nothing restarted.
- `05-bad-appearance-values-fall-back-and-are-named-release.png` — the file rewritten with a white
  `background`, a family that does not exist, `body_size = 40` and a valid `status_size = 9`, then reloaded: the
  window is back in the shipped dress (removing a setting reverts it), the status text is 9 px (the valid
  setting applied), and the board says *"theme.background was not used… Its contrast with theme.foreground is
  1.25:1, below the 4.5:1"*, *"font.body_size… must be between 8 and 32"* and *"font.family… No installed
  font family has that name."* — none echoing the value.

### Ablations (committed tree, `rfcs/handoffs/ablate.sh`)

| Removed | Failed |
| --- | --- |
| The contrast check | `a_low_contrast_pair_falls_back_…`, `reverting_one_colour_can_fail_another_pair_…`, `the_minimum_is_inclusive_at_exactly_4_5`, `the_palette_that_comes_out_always_meets_the_minimum`, and the shell reload test that names the ratio |
| `/` as a forbidden family character | `a_family_is_a_name_and_only_a_name` |
| Upper size bound made exclusive | `each_size_bound_is_inclusive_and_refuses_just_outside` |
| Ratio rounded, not floored | `the_measured_ratio_is_floored_…` and the shell sentence test (`4.49` must not read `4.50`) |
| A view imports iced's `text` | `no_shipped_view_builds_text_with_the_icedcrate_text_function` |
| `iced::font::load(..)` added to shipped code | `no_shipped_code_loads_a_font` |
| Reload does not put the theme in force | `a_reload_applies_theme_and_sizes_live_…` |
| Contrast checked once, not to a fixed point | `reverting_one_colour_can_fail_another_pair_…` and the 3,000-input property |
| An unavailable family not reported | `a_family_is_used_only_if_the_lookup_finds_it_…` and the boot/reload board test |
| Reload does not set the UI font | `the_ui_font_is_set_at_boot_and_at_every_reload` |

**The plan asked for the contrast ablation to fail "alone".** It fails five tests, all of them about contrast;
none is unrelated. The checklist's wording applies: what shares a fixture is named.

### Judgment calls, disclosed

- **D5's pairs are the two text pairs, at 4.5:1. Borders, `accent` and the scrim are not measured.** The
  project's own `derived_contrast_pairs` test holds the *defaults* of the borders and accent to 3:1 (WCAG
  1.4.11); I did not extend that to user colours because D5 says 4.5:1 for a foreground/background pair and I
  did not want to invent a second threshold. The consequence: a user can make a border invisible. Focus never
  relies on colour alone (NFR-UX-002), so the window stays usable; say if you want the 3:1 rule applied.
- **The scrim can be set to any alpha, including opaque.** RFC-018's reason for a translucent scrim is that an
  opaque one is indistinguishable from a spoofed full-window rectangle. A user restyling their own window is not
  the attacker that argument is about, but it is the same visual property, and I did not add a rule D5 does not
  contain.
- **`[ui]` stays withdrawn, with a message saying where its settings went.** "No effect yet, returns with the
  feature" would be false about `ui.font_size` now that the feature exists under `[font]`.
- **Buttons do not follow the theme.** In `04-…` the blue `Trust Settings` and `?` buttons keep iced's default
  palette: they were never drawn from `Theme`'s seven roles. That is older than this slice, and outside "nothing
  else becomes settable" — but a user who sets `background` will notice, and `theme.rs`'s claim that "every colour
  the shell draws comes from a `Theme` value" is not true of them.
- **The family reaches the editor's text as well.** The editor draws with the default face, so it follows the
  configured family; the terminal and the tree do not. Stated on the configuration page.
- **The board's ordering of two related fallbacks is the order colours are reverted in** (alphabetical by role for
  a single pair, then the cascade). It is deterministic and named per setting; it is not "the first mistake in the
  file".
