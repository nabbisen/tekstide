---
title: "What a configuration file must not do"
rfc: "RFC-045"
rfc_file: "../../accepted/045-configuration-reachability.md"
source_rfc_status: "Accepted 2026-09-12 — M12"
target_milestone: "M12"
created: "2026-09-12"
---

# What a configuration file must not do

**Required reading before writing code.** A configuration file is a security surface that a user
edits with a text editor and a project can ship. Every failure mode below is either a lie the user
reads or an executable they did not choose.

## §1 A key that does nothing is a lie the user reads

Today `to_ai_cli_profile` reads `command` and `display_name` and silently drops `args`. A
user writes `args = ["--model", "x"]`, the file validates, the profile appears, and `--model`
never happens. Nothing says so.

**Rule:** the parser accepts only keys with a consumer, and refuses the rest with a diagnostic that
names the key and says *it has no effect yet*. Not a warning — warnings are read once. A file that
does not load is noticed; a file that half-loads is trusted.

## §2 A profile the audit trail cannot record must not be definable

RFC-046 launches a profile whose id fails `AuditReference::new` **unaudited**, by design — a launch
path must never panic on valid data. A *definition* is different: a user who can write
`[agent.profile."my tool"]` has created an executable every launch of which is silently unrecorded.

**Rule:** the id is validated at parse **by calling `AuditReference::new`**, never by a copied
character class. If the two ever disagree, the parser is wrong, and it is wrong in the direction of
accepting something the trail will not hold.

## §3 No configuration-defined executable runs without a deliberate act

OQ3's answer, restated as the invariant the tests must hold: **between the file and the process
there is always one click that names the executable.** At first use, because a file present at boot
was never confirmed. At reload, because a changed profile is a new executable with an old name.

Test the invariant, not the dialog: a config-defined profile launched with no confirmation on record
must be refused with a notice, and the ablation is to delete the confirmation check and watch that
test fail.

## §4 Values do not reach the audit trail; keys do not either, yet

RFC-023's acceptance: *no configuration values reach durable audit.* The producers take no field.
Whether a `SecuritySensitiveField` name is a "value" is D7's reserved question. **Rule for this
slice:** wire `record_sensitive_config_policy_increase`/`_reduce` exactly as built. Do not add a
field, a reference, or a reason code to the record to be helpful — `valid_config_change` will
reject it, and if it did not, you would have reopened a frozen vocabulary inside a reachability
slice.

## §5 The confirmation names the resolved executable, never the display name

`display_name` is user-controlled text. A profile with `display_name = "Claude Code"` and
`command = "/tmp/x"` is the spoof this dialog exists to make visible. **Rule:** the first-use
confirmation shows the **path `resolve_executable` actually produced** and the **file the profile
came from**. It may show the display name too, but never *instead*. This is RFC-018's trusted-UI
evidence rule applied to a launch: what the user confirms must be the thing that will run.

RFC-047 §5's wording constraints carry over: state the fact, do not imply danger the profile does
not carry, do not imply the user can fix anything from the dialog.

## §6 The board line says which of three things is true

A configuration can be **ignored** (invalid; defaults in force), **loaded with warnings**, or
**loaded with changes pending confirmation**. These are different states and the line must say
which — RFC-047 §4.1's lesson, in advance: a sentence describing something adjacent to what was
measured passed every test that RFC had. Absent when nothing is wrong, per RFC-047 §2.
