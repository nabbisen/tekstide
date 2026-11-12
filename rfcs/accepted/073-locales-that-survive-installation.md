# RFC-073: Locales That Survive Installation

Status: **Accepted by the human owner 2026-11-12** — D1 through D4 as written, in `0.40.0`, alongside RFC-074's first slice. Proposed the same day; reserved 2026-09-24 as *"The `locales_dir()`/`CARGO_MANIFEST_DIR`
Locale-Loading Defect"*; written now because `0.39.0` turned the channels where it bites into this
product's primary ones. The release slot was decided with `0.40.0`'s scope
on the same day, and the decisions below were accepted immediately after.

## Summary

Every non-`en` catalog this product ships is loaded from a path baked in at **build time**. On the
machine that built it, that path exists and the catalog resolves. On a user's machine it does not,
and the catalog silently falls back to English — not as an error, but through the same
fallback-to-source behaviour that correctly handles a genuinely corrupt file.

The fix is not a second place to look for the file. It is to stop looking: compile every
`locales/*.ftl` in the way `en` already is, and select by name at runtime.

## Why now, measured

The defect was found on 2026-10-14 (review 561) and recorded with its reproduction. Nothing about
the mechanism has changed. **What changed is who is exposed**, and `0.39.0` is what changed it:

| Channel | `locales_dir()` points at | Non-`en` locale | Shipping as of |
| --- | --- | --- | --- |
| `cargo install tekstide` | the registry source dir, which persists | works | always |
| GitHub release binary, and the one-liner `README.md` leads with | CI's own `/home/runner/work/…` | **never loads** | `0.37.0` |
| `tekstide-aarch64-macos` | the same, on the macOS runner | **never loads** | `0.38.0` |
| AUR `tekstide` (source) | the user's own `makepkg` `srcdir` | works until the build dir is cleaned | `0.39.0` |
| AUR `tekstide-bin` | **the GitHub release binary's path** | **never loads** | `0.39.0` |

Three of those five channels are new since the defect was recorded, and the two AUR rows are this
release's own. The channel the README leads with has never worked.

**Re-verified on `0.39.0`'s own artifact, not carried forward from the 2026-10-14 finding.** The
packaged binary from this release's real `makepkg` run contains exactly one reference to the build
directory, and it is immediately followed by the literal `locales`:

```
…/packaging/aur/src/tekstide-0.39.0localesTekstide
```

The package is stripped, so this is a live runtime path, not debug information — which also means
`makepkg`'s own *"Package contains reference to `$srcdir`"* warning, which every AUR bump has
surfaced and every reviewer has waved past as debug noise, **has been this defect reporting itself
all along.** The reviewer waved it past once more at `0.39.0` before checking.

## Why it survived

`i18n/tests.rs` contains `real_polish_catalog_loads_from_the_shipped_locales_directory`, and it
passes. It has always passed. It will pass after this RFC lands and it would pass if the defect
were twice as bad, because it runs from a source tree where the shipped locales directory is
exactly where the baked-in path says it is.

A correct test of the wrong property — the shape of the claim, never its truth. The product's own
Run It section records the same failure in a different place: *"There was even a passing test over
those exact strings."* An installed copy is a different thing from a checkout, and nothing in the
suite has ever asserted anything about one.

**How bad, honestly: small today, and that is not the argument.** `pl.ftl` is 1,026 bytes and its
own header says it exists to prove the pluralization machinery works, not as a translation. No real
translation is lost right now. What is true is that the hole is open, silent, and in the path of the
first real second locale — which would ship into it without a single failing test.

## Decisions

### D1. Every catalog is compiled in. The runtime path is deleted, not kept as a fallback

`include_str!` each `locales/*.ftl`, selected at runtime by name — exactly what
`catalog.rs:21` already does for `en`. `load_locale_from_disk` and `locales_dir()` go away
entirely.

**Not a second place to look.** A disk path kept as a fallback would preserve every behaviour this
RFC exists to remove, including the `$srcdir` reference, and would leave the same two failure modes
indistinguishable. This is the lesson the `NOTICE` embedding fix already taught in this codebase:
the fix for "the file is not where the binary thinks" is to stop having a path.

**The cost, measured, so it is not an objection carried on a guess**: `en.ftl` is 105,406 bytes and
is *already* compiled in. `pl.ftl` is 1,026. The binary grows by about a kilobyte.

### D2. The compiled-in set is an explicit list, and a gate asserts it matches the directory

Rust cannot glob at compile time without a build script. **No build script**: it is a second
mechanism to audit, it complicates both published packages, and it hides the list from review.

An explicit `const` list instead — and because an explicit list is exactly the thing that goes
stale, **a test asserts the list is every `.ftl` in `locales/`**. This codebase's own recurring
failure is the enumerated set with a writer nobody lists; a new catalog file that compiles but is
unreachable would be that failure again, silently. The gate resolves `locales/` from
`CARGO_MANIFEST_DIR`, never by walking up for `.git/`, so it keeps working from an unpacked
package — the same rule PR-074-A's gate follows.

### D3. The test that hid this is replaced by one an installed copy could fail

`real_polish_catalog_loads_from_the_shipped_locales_directory` asserts the wrong property and must
not survive in its current form. What replaces it asserts that a catalog resolves **with no locales
directory present at all** — the state every installed copy is in. That assertion, and not the fix,
is what stops this returning.

### D4. Fallback semantics are unchanged; one state stops existing

A requested locale that this build does not carry still falls back through the chain to the source
bundle, exactly as today. What changes is that **"shipped but unreachable" stops being a state the
product can be in** — the only remaining reason a catalog does not resolve is that it was never
shipped, which is a fact about the build and not about the machine it runs on.

## Out of scope, deliberately

**The `--locale` flag, and the coupling that argued for including it, which this RFC dissolves.**
The 2026-10-14 entry says this should be one slice with the queued CLI-parsing item, reasoning that
*"a future `--locale pl` flag on a release binary would let a user select a catalog they
structurally cannot have under the current disk-loading design."*

That reasoning was right about the design it was written against, and D1 removes it. Once every
shipped catalog is compiled in, there is no catalog a user can name and structurally not have — so
the flag stops being a way to expose a defect and becomes an ordinary feature, which belongs with
the rest of the CLI work and not here. **The coupling existed because of the defect; fixing the
defect ends it.** Recorded rather than quietly dropped, because the earlier entry says the opposite
and a later reader deserves the reason it changed.

**Not translation.** `pl.ftl` stays what it says it is: a proof that the pluralization machinery
works. This RFC makes it reach the user; it does not make it a translation.

## Acceptance

**What the owner decides**: D1 through D4. The release slot is already decided (`0.40.0`), and
nothing here changes the published API of either crate.

**What I am asking to be accepted**: that the runtime path is removed rather than supplemented
(D1), that the explicit list is gated against the directory rather than trusted (D2), and that the
test which hid this is replaced by one an installed copy could fail (D3).
