# RFC-016 PR-016-B: source-locale catalog.
#
# The source locale (`en`) is compiled into the binary (see
# `i18n::catalog`), so a missing or corrupt catalog file on disk can
# never make the application unusable -- this file only needs to exist
# at build time.
#
# Actual translations are content work, not this RFC's job (RFC-016
# §Non-Goals). These keys exist to prove the lookup, locale-selection,
# and fallback machinery works, not to cover a real UI -- RFC-015 has
# not been implemented yet, so there is no real UI to cover. Real
# surfaces add real keys as they are built.

app-title = Tekstide
project-board-title = Project Board

# RFC-016 PR-016-D: proves both pluralization and interpolation through
# one realistic key, modeled on `shell::render_text`'s real "blocked
# automation: N" field (response 123/124's own example of why PR-016-D
# unblocks RFC-015 PR-015-D). The selector doubles as the response's
# design constraint made concrete: `$count` may be a genuine number
# (plural-category selection applies) or one of `CountDisplay`'s three
# non-numeric states, expressed as literal string variants alongside the
# plural ones -- the same interpolation argument, the same key, no
# second API. `CountDisplay::label()` itself is not called from here;
# this key only proves the catalog CAN express all four shapes, per
# response 123/124's explicit "record it, do not solve it in PR-016-D."
#
# `[one]` and `*[other]` deliberately use distinct wording (singular
# "automation" vs. plural "automations"), not the same shape with a
# number spliced in -- response 125 Required 2: identical branches made
# `plural_categories_apply_for_english_too_with_its_simpler_one_other_split`
# pass even with the `[one]` variant deleted entirely, proving nothing
# about plural selection despite the test's name.
blocked-automation-count = { $count ->
    [not_implemented] blocked automation: not implemented
    [unavailable] blocked automation: not available
    [unknown] blocked automation: unknown
    [one] {$count} blocked automation
   *[other] {$count} blocked automations
}

# RFC-015 PR-015-B: chrome and layer-composition-demo keys. No real
# surface exists yet (PR-015-D), so the content area shows only this
# placeholder; `status-bar-summary` covers both the two possible routes
# (a literal-variant selector, exactly `blocked-automation-count`'s
# non-numeric branches) and a genuine plural count in one lookup, per
# the same one-key pattern PR-016-D established.
content-area-placeholder-title = No surface rendered yet
content-area-placeholder-body = RFC-015 PR-015-D adds the Project Board surface here.

status-bar-summary = { $route ->
    [project-board] Project Board
   *[active-project-workspace] Project Workspace
} | { $count ->
    [one] {$count} project
   *[other] {$count} projects
}

# Scaffolding for this slice's own layer-composition screenshot evidence
# only (see `shell.rs`'s module doc) -- not a real dialog. RFC-022 supplies
# real trusted dialogs. PR-015-C made it genuinely dismissible (Tab/
# Shift+Tab cycles Acknowledge/Dismiss, Enter or Escape closes it) so the
# modal-exclusivity and focus-trap properties have something real to
# exercise -- a placeholder that never closed could not prove either one.
layer-demo-modal-title = Layer Composition Demo
layer-demo-modal-body = This placeholder proves the modal layer renders above content, never inside it.
layer-demo-modal-acknowledge = Acknowledge
layer-demo-modal-dismiss = Dismiss
layer-demo-modal-dismiss-hint = Tab/Shift+Tab moves focus; Enter or Escape dismisses.
