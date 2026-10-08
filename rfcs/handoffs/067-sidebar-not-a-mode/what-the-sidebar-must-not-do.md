# What the sidebar must not do

| # | It must not | Why this one |
| --- | --- | --- |
| 1 | **Move the user out of a terminal for any reason but their own explicit activation.** | D8, and the one hazard this RFC creates. Once activating a file can change the mode, every other path that opens a document inherits the power to yank someone out of a terminal they are watching. **RFC-027's recovery offer opens documents, and it ships first (`0.31.0`).** A refresh touches them too. Only a deliberate activation in the tree may switch. |
| 2 | **Add a focus zone, or change what `Tab` does.** | D3. `FocusZone::Sidebar` already exists in both modes — this gives it contents, nothing more. A cycle a user has learned must not grow a step because a panel gained a tree. |
| 3 | **Leave a reworded placeholder behind.** | D1. `sidebar-placeholder-title` exists only to explain an absence. Rewording it keeps a sentence whose subject no longer exists; deleting it is the change. |
| 4 | **Introduce pane or slot vocabulary to the user.** | D6. `Primary`/`Secondary` are internal. Nobody should need to know what a slot is to read their own screen. |
| 5 | **Grow a layout.** | Non-goal, and the thing most likely to happen by accident. Splits and docked panes are a separate RFC with their own design. If this slice starts needing one, it has left its scope — say so. |
| 6 | **Decide D5 by taste.** | The whole point of measuring the switch is that "it feels fast enough" and "it is not" are both opinions until there is a number. **Building nothing, with the number recorded, is a valid and preferred outcome.** |
