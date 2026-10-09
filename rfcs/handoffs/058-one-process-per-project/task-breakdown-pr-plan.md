# RFC-058 task breakdown and PR plan

## PR-058-A — reproduce, and nothing else

**No lock, no IPC, no product change.** Two real Tekstide processes, one real project root, the same
file edited differently in each, the record file read afterwards.

- Prove the two instances get the **same project id** (the reuse in `app.rs` is the mechanism) and
  therefore the same record path.
- Show the clobber: one instance's buffer in the record, the other's gone, nothing said.
- **If it does not reproduce, stop and report that.** It outranks the rest of the RFC.

The reproduction becomes the regression test, and it is what PR-058-B is checked against.

## PR-058-B — the mechanism

- Process-visible, under the state directory, scoped to the project (D3, row 4).
- **Released when the holder dies** (D4, row 3) — and read
  `a_live_writer_holds_an_exclusive_lock_until_it_is_dropped` first, because this project has
  already been bitten by a descriptor outliving its drop.
- **Cannot-decide opens the project** (D5, row 1). Test the ambiguous cases deliberately: a lock
  naming a dead process, an unreadable state directory, a lock this build does not understand.
- The audit store is confirmed untouched (D1, row 5) — a test that it still works with two live
  processes is worth having, since it is the one piece already satisfied.

## PR-058-C — saying it

- A second attempt names the holder and does not open a duplicate (D8).
- **Attention is requested; no wording claims a raise** (D9, row 2). `iced`'s
  `window::request_user_attention` is the only thing that works on Wayland, and the compositor
  decides whether it shows.
  - **Decision taken, reversible:** send the attention request even though its effect is not
    guaranteed. It costs nothing, it is the only mechanism that can help, and the message carries
    the truth regardless of whether the compositor renders anything.
- The IPC that lets B reach A (D10). Prior art: the approval adapter's `AF_UNIX` channel —
  **including its path-length limit**, which this project has hit more than once.
- **Live capture**: two real instances, the second showing what it says. This is the slice a user
  meets, so the capture is the evidence that the wording is honest.
- `REQ-PROJ-009` returns to the Project-lifecycle coverage row, naming **what the mechanism actually
  is** — not "a lock exists now".

## Not in scope

`recent-projects.json`'s last-writer-wins; multi-instance view coordination; RFC-036's
recent-project pruning gap; anything that depends on raising a window.
