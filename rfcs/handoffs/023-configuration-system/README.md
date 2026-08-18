# RFC-023: Configuration System - Developer Handoff Pack

Source RFC: [RFC-023](../../proposed/023-configuration-system.md)
Target milestone: **M12** (all slices headless — start immediately)
Source RFC status: **Proposed**

**Start here.** This file is the entry point. Everything is linked below in reading order.

## Read in this order

| # | Document | Purpose |
| --- | --- | --- |
| 1 | [RFC-023](../../proposed/023-configuration-system.md) | Format, precedence, atomic validation, security-sensitive settings, audit vocabulary. |
| 2 | This file | Orientation and what is binding. |
| 3 | [`implementation-handoff.md`](./implementation-handoff.md) | Module layout, validation pipeline, the profile-bypass trap, audit mapping. |
| 4 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Slice boundaries and review gates. |
| 5 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | Required evidence. |
| 6 | [`qa-evidence.md`](./qa-evidence.md) | Where you record gates, findings, and limitations. |

Read before starting, because this RFC conforms to them rather than amending them:

- [RFC-004](../../done/004-security-baseline-and-restricted-mode.md) — Restricted Mode feature vocabulary you will extend.
- [RFC-010](../../done/010-agentrun-launch-model-and-ai-cli-profiles.md) — executable provenance rules that configuration must not weaken.
- [RFC-013](../../done/013-durable-audit-store-and-local-data-policy.md) — the frozen `sensitive_config_changed` family.

## Where to start work

**Begin at PR-023-B.** PR-023-A is design acceptance. All slices are headless — no GUI dependency, so this runs in parallel with RFC-014 and RFC-021.

## Five things that are binding

1. **Configuration is a security surface, not a convenience feature.** It can name the executable Tekstide launches. Treat it with the discipline RFC-010 applies to profiles.
2. **Validation is atomic.** Parse → validate whole document → construct → swap. A partially applied configuration is a defect, not a degraded mode.
3. **Configuration cannot bypass RFC-010.** A config-defined AI CLI profile passes through identical provenance validation. This is the single most likely place to introduce a real vulnerability — see `implementation-handoff.md` §4.
4. **Security-sensitive settings never hot-reload.** They require explicit confirmation and produce an audit event.
5. **Missing config is not an error.** Compiled defaults are total; Tekstide starts normally without a file, and an invalid file must not become a denial of service.

## One vocabulary trap

RFC-013 froze two audit action kinds whose names are ambiguous:

- `config_policy_increase` — **increases the permitted capability surface** (weakens security). Requires authorization.
- `config_policy_reduce` — **reduces the permitted capability surface** (tightens security). Applied directly.

The names read the other way round to most people. RFC-023 pins the semantics; the authorization asymmetry in the frozen schema is what settles it. Do not guess from the names.

## Carried in 2026-08-18 — the contrast gate does not survive configurability

`theme-contrast-verification` (landed `55f53d8`) added a real WCAG gate over
`Theme::default`: 4.5:1 for text pairs, 3:1 for non-text pairs, plus `composite_over` so
the translucent scrim is measured against its actual backdrop. It caught and fixed a real
failure — `border_default` at 2.63:1.

**That module is `#[cfg(test)]`-gated**, correctly for today: nothing in the production
render path needs a contrast ratio, and clippy confirmed it as dead code otherwise. But the
gate therefore validates exactly one palette — the compiled default — at build time.

**RFC-023 is what breaks that.** The moment `NFR-UX-004`'s configurable colours land, a
user-supplied palette reaches the renderer having passed no contrast check at all, because
the only check that exists is compiled out of the shipping binary. A user can silently
configure an unreadable UI, including one where focus indication disappears.

Decide deliberately, do not inherit it:

- **Promote the module to production** and validate a loaded palette when config is
  applied — refuse, warn, or fall back. Note that refusing a palette is a config-rejection
  path, and `Missing config is not an error` above means an invalid file must not become a
  denial of service; the same reasoning applies here.
- **Or keep it test-only and state plainly** that configured colours are unchecked and
  accessibility is the user's responsibility once they override the default.

Either is defensible. Silently shipping the second while the changelog says a WCAG gate
exists is not.

## Scoping added 2026-08-19 — read this before PR-023-B

RFC-023's own §Scoping section (at the end of the RFC) was added at handover and **changes what
this pack is for**. In short:

**This RFC delivers the configuration mechanism. It does not deliver every setting that names
it.** Five things in shipped code point here expecting their settings — keybindings, theme
values, locale preference, resource limits, and transcript capture defaults — and this RFC's
Goals name none of them. Each is to be recorded as **out of scope with a stated owner**, not
silently left pointing at a promise this RFC never made.

Three previously-open questions are answered there too, and they bind:

- **Workspace configuration does not ship in v1.** Vocabulary reserved, defaults + user-global
  only. A file inside a project root is the untrusted surface RFC-032's trust model gates.
- **An invalid configuration file produces a notification, not a blocking dialog** — this RFC's
  own goal says an invalid file must not become a denial of service, and a modal nobody can
  dismiss without valid configuration is exactly that.
- **Configuration-defined AI CLI profiles require a one-time confirmation on first use.**
  RFC-010's provenance validation still applies; provenance is not intent.

**One thing this pack predates and must now carry** (recorded in this file 2026-08-19, see
below): the WCAG contrast gate added by `theme-contrast-verification` is `#[cfg(test)]`-gated,
so it validates exactly one compiled palette at build time. The moment configurable colours
land, a user-supplied palette reaches the renderer having passed no contrast check at all.
Decide that deliberately — promote the module and validate on load, or state plainly that
configured colours are unchecked. Shipping the second silently while the changelog advertises a
WCAG gate is not an option.
