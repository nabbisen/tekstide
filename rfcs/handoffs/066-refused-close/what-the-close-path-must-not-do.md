# What the close path must not do

| # | It must not | Why this one |
| --- | --- | --- |
| 1 | **Terminate anything live before the close is known to be going ahead.** | The defect itself. A terminal session is process state: unlike a buffer, nothing records it and nothing can offer it back. RFC-027 is giving unsaved text somewhere to return from; a killed shell has no such thing and never will. |
| 2 | **Leave a confirmed action unexplained.** | The user pressed a button and something other than what it said happened. Silence here teaches a user that confirmations in this product are unreliable, and that lesson does not stay in this dialog. |
| 3 | **Force the close.** | The assessment exists so unsaved files, pending approvals and review-ready changes are not closed over. Nothing in this RFC bypasses it, and a fix that "solves" the problem by closing anyway has inverted it into data loss. |
| 4 | **Grow a second opinion about whether a close may proceed.** | `assess_close` already computes every blocking reason. A new predicate beside it would be a second source of truth about the user's own unsaved work, and the two would drift. Read the result; do not re-derive it. |
| 5 | **Report a repair that was never reproduced.** | D5. The chain was read, not observed. A regression test written against a defect nobody triggered proves the test passes, not that the defect is gone. |
