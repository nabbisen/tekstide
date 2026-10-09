# What the lock must not do

**The likely failure here is not the data loss this RFC exists to stop. It is making the editor
refuse to work.** Rows 1 and 2 carry more risk than row 3.

| # | It must not | Why this one |
| --- | --- | --- |
| 1 | **Leave a user unable to open their project because of a lock it cannot reason about.** | D5. A stale file from a crashed process, an unreadable state directory, a full disk — none of these mean another instance is running. When the mechanism cannot decide, **open the project and do not claim protection**. Refusing is only correct when we *know* another live instance holds it. |
| 2 | **Say the existing window was raised, switched to, or focused.** | D9. On Wayland it was not — `gain_focus` is documented unsupported there, and only an attention request is possible. `0.23.0` was *What The Window Says Is True*; wording that implies a switch the compositor did not perform is that defect returning, in the release that was supposed to make the product more honest about what it is doing. |
| 3 | **Let the lock outlive the process that took it.** | D4. A lock that survives a crash converts a rare conflict into a permanent outage — the strongest form of row 1. This project has met the exact hazard: `a_live_writer_holds_an_exclusive_lock_until_it_is_dropped`, where a **fork-duplicated descriptor kept an `flock` alive past its own drop**. Read that row before choosing the primitive, not after. |
| 4 | **Use an in-memory lock.** | `REQ-PROJ-009` says so in its own words: *"In-memory-only locks are insufficient."* Whatever is chosen must be visible to a process that did not exist when the first one started. |
| 5 | **Build a second mechanism for the audit store.** | D1. SQLite's own file locking already satisfies the requirement there. A requirement satisfied twice, two different ways, is one nobody can reason about later. Confirm and leave it. |
| 6 | **Report a repair that was never reproduced.** | D2. The clobber was read in the code, not watched. A regression test written against a defect nobody triggered proves the test passes. If it will not reproduce, **that is the finding** and it outranks the fix. |
| 7 | **Grow into multi-instance coordination.** | Non-goal. Nothing here synchronises views, selections, terminals or `recent-projects.json`. If the chosen mechanism happens to fix the last one, say so; do not go looking. |
