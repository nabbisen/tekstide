---
title: "What the configuration must not allow"
source_rfc: "../../accepted/060-command-approval-a-user-can-reach.md"
---

# What the configuration must not allow

**This document does not repeat
[`what-the-dialog-must-not-lie-about.md`](../022-adapter-spawn-and-command-approval-surface/what-the-dialog-must-not-lie-about.md).**
That one governs what the dialog may claim and still applies in full. This one governs the new
surface RFC-060 adds — a configuration file that can now ask for adapter-mediated approval — and
the ways that surface could lie on the dialog's behalf.

## §1 It must not be able to express a `Managed` profile without an adapter

Today the impossibility is accidental: `ConfiguredAiCliProfile` has no field for a compatibility
level, so it cannot ask for the wrong one. **After this RFC the impossibility must be
deliberate.**

`config/profile.rs`'s own doc already argues the shape: a configuration that is *structurally
incapable* of expressing the dangerous combination, not one that expresses it and is refused. A
validation error is a worse answer than an unrepresentable state, because a validation error can
be removed by a later edit that looks like a cleanup.

Concretely: `Managed` must follow from declaring an adapter, and `structured_action_approval` must
follow from the same declaration. **Three independent fields a user can set inconsistently is the
defect**, even if every inconsistent combination is currently rejected.

## §2 It must not let a configuration file turn an ordinary AI CLI into a "supervised" one by
## saying so

A user can write any command they like. Declaring that command as an adapter does **not** make it
one, and Tekstide cannot check that it is: the protocol is cooperative, so a program that never
proposes anything is indistinguishable from one that has nothing to propose (D6).

So nothing in the product may read a configuration file's claim back to the user as a fact about
the program. The profile says *this command is declared as an adapter*; it never says *this
command reports its actions*.

**The dangerous phrasing to avoid is the reassuring one**: "approval enabled", "supervised",
"protected". What is true is narrower and must stay narrower.

## §3 A declared adapter that proposes nothing must not look like a failure

D6. An adapter chooses what to propose, and may propose nothing for an entire run. A run that
produces no approval request is a completely ordinary outcome.

Nothing may report it as an error, a timeout, or a degraded state — and nothing may report its
absence as evidence that the run did nothing dangerous.

## §4 The capability token must not acquire a new description

RFC-022 §3 settled this: the token says *which run is asking*, not *that the asker is
trustworthy*, and against a hostile process running as the same user it is worthless.

This RFC gives the token its first real users. That is exactly when someone will be tempted to
describe it in the configuration documentation as securing the channel. **It does not.** Whatever
the book says about configuring an adapter must not upgrade the token's own claim.

## §5 It must not let the audit trail imply enforcement either

`REQ-SEC-014` records approvals and denials, and `0.35.0` shipped the view that reads them. A
`command_approval` / `rejected` record means *the user rejected the proposal* — **not** that the
command did not run.

A reader of the audit view, a year from now, must not be able to conclude from a rejection record
that a command was prevented. Whatever the audit view, the book, or an exported report says about
these records has to leave that door shut.

## §6 The sweep is part of the work, not a review step

D4. `REQ-AGENT-013` is a prohibition: the product must not claim command-level enforcement. Meeting
it means reading every place the product describes approval — `en.ftl`, both book pages, the
README, this release's changelog entry — and checking each against what is actually true.

**A grep for "approval" is the start of that sweep, not the end of it.** The dangerous sentences
are the ones that imply enforcement without using the word: *protected*, *blocked*, *prevented*,
*secured*, *requires approval before running*.
