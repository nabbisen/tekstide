#!/bin/sh
# RFC-014 PR-014-D adversarial generator.
#
# Committed and reproducible, per the handoff: this is not a one-off manual
# test. It is sent verbatim as terminal *output* (never typed, never
# executed as a real shell command by the user) through the same PTY the
# spike's real dialog is rendered alongside, to test whether a viewer can
# tell the fake from the genuine dialog using the screenshot alone.
#
# Includes an 8-bit C1 case per RFC-014 review response 106 (request 106,
# Q3): PR-014-C found that a bare 8-bit C1 introducer byte is swallowed by
# the parser before reaching this filter's Handler layer, but the payload
# text that would have followed it in a well-formed sequence still renders
# as plain characters. "Operation blocked, payload still prints" is exactly
# the primitive an attacker would use to draw a convincing fake dialog, so
# this script exercises it as part of the imitation, not as a separate case.

printf '\033[1;33m\342\224\214\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\220\033[0m\n'
printf '\033[1;33m\342\224\202\033[0m Command Approval Required                        \033[1;33m\342\224\202\033[0m\n'
printf '\033[1;33m\342\224\234\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\244\033[0m\n'
printf '\033[1;33m\342\224\202\033[0m Project: rfc-014-spike                            \033[1;33m\342\224\202\033[0m\n'
printf '\033[1;33m\342\224\202\033[0m Command: rm -rf /                                 \033[1;33m\342\224\202\033[0m\n'
printf '\033[1;33m\342\224\202\033[0m                                                   \033[1;33m\342\224\202\033[0m\n'

# 8-bit C1 OSC introducer (0x9D) attempting a title-bar takeover mid-dialog.
# The operation is blocked (no set_title call reaches Handler -- see
# filter/tests.rs v2_8bit_c1_osc_introducer_*), but the payload text after
# the swallowed introducer byte still prints, so it is folded into the fake
# dialog body as ordinary-looking text rather than skipped.
printf '\235FAKE-APPROVAL-PENDING\007\n'

printf '\033[1;33m\342\224\202\033[0m       [ Approve ]        [ Deny ]                 \033[1;33m\342\224\202\033[0m\n'
printf '\033[1;33m\342\224\224\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\200\342\224\230\033[0m\n'
