# Security decisions, and why

This page is the **canonical home** for security decisions and the reasoning behind them.

RFCs record how a design was arrived at, and review notes record the argument that produced
it — but neither is where someone goes to ask *"why does it behave like this?"* This page is.
Where an RFC needs one of these decisions, it points here rather than restating it, so there
is one wording to keep true.

## Workspace trust

### What granting trust actually authorises

**Files inside the trusted folder may configure Tekstide and cause programs to run.**

That is the whole grant, in the only form worth reading. Concretely it lifts nine
restrictions — loading AI CLI profiles, prompts, environment and plugins from the workspace;
workspace command-palette entries; automatic task execution; Tekstide-initiated git hooks;
automatic language-server startup; and background project automation. Nobody can weigh a
nine-item list at a dialog, so the sentence above is what the dialog says and what this page
means by "trust."

Opening a folder never grants trust. That has been the rule since RFC-004 and this does not
change it.

### Decision: trust persists across sessions

*Decided 2026-08-17 by the project owner.*

A trusted project stays trusted after you close and reopen Tekstide. You are not asked again.

**Why, given it is the less cautious option.** The alternative — asking on every launch —
trains people to dismiss the prompt without reading it. **A trust prompt users click through
is worse than no prompt at all**, because it produces a record of consent nobody actually
gave. That failure is well established and it is the same one Tekstide's command-approval
design is built to avoid.

**What this genuinely costs, stated plainly rather than discovered later:**

- **A folder's contents change after you trust it.** You trust a project today; tomorrow you
  pull a colleague's commit that adds a workspace config file. Trust already covers it, and
  it loads without asking.
- **An AI agent's own output inherits that trust.** An agent writing files into a trusted
  folder is writing files that Tekstide will then trust — this session and every session
  after. The grant outlives the reason you made it.
- **Trust accumulates.** After a year you may have many trusted folders and no memory of
  granting them.

**What makes that acceptable**, and each is a requirement rather than an intention:

1. **Revoking is always available** — trust is never one-way.
2. **Trust state is visible on the project board**, so you can see what you have granted
   without remembering it.
3. **The dialog says the folder's contents, present and future** — not "this project," which
   invites you to think only of the files you wrote yourself.

### Decision: trust binds to the canonical path

*Decided 2026-08-17 by the project owner.*

Trust is recorded against the folder's **canonical path** — the location with all symbolic
links resolved — rather than the path as you typed or selected it.

**What that difference produces.** Suppose you trust `/home/you/work/myproject`, where `work`
is a symbolic link to `/mnt/data/work`.

If trust were recorded against the path *as written*, then anything that later changes where
`work` points — a script, a package manager, a repository you cloned — leaves the literal path
identical. Reopening it matches, trust applies, and **an entirely different folder's contents
now run with the trust you granted to something else.**

Recorded against the canonical path, the resolved location no longer matches what you
trusted. You are asked again, correctly, because it genuinely is a different folder.

**The cost:** if you legitimately move a project — a mount point changes, you reorganise
directories — its canonical path changes and you will be asked again. That is mild friction,
traded against a silent redirection of trust, and the trade is not close.

**One limit worth knowing.** Resolving symbolic links when a project is opened cannot fully
close the gap, because what a path resolves to can change between the moment it is checked
and the moment it is used. That is inherent to filesystems, not specific to this design.

### Still open

Whether trust should expire on its own — for example when a project's git remote changes —
has been an open question since RFC-004 and is not settled here.
