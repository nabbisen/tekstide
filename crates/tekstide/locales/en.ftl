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
blocked-automation-count = { $count ->
    [not_implemented] blocked automation: not implemented
    [unavailable] blocked automation: not available
    [unknown] blocked automation: unknown
    [one] blocked automation: {$count}
   *[other] blocked automation: {$count}
}
