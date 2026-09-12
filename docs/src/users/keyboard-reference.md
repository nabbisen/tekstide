# Keyboard reference

The shell is keyboard-navigable by design. This table is the same set the application itself
lists on the Project Board, in the Help modal (`Ctrl+Alt+K`), and in `tekstide --help`.

| Binding | Action |
| --- | --- |
| `Ctrl+Alt+P` | Open the Project Board |
| `Ctrl+Alt+N` | Switch to the next open project, cycling with wraparound |
| `Ctrl+Alt+O` | Open a field to type or paste a project path |
| `Ctrl+Alt+B` | Open a folder browser to choose a project without typing a path |
| `Ctrl+Alt+M` | Toggle Content / Terminal mode for the active project |
| `Ctrl+Alt+T` | Launch a real terminal in the active project (switches to Terminal mode) |
| `Ctrl+Shift+V` | Paste into the focused terminal |
| `Ctrl+S` | Save the open file |
| `Ctrl+Alt+A` | Launch an AI CLI run in the active project — refused unless the project is trusted |
| `Ctrl+Alt+U` | Open the Workspace Trust surface for the active project (grant or revoke) |
| `Ctrl+Alt+H` | Open the Approval History surface for the active project |
| `Ctrl+Alt+R` | Open the AgentRun Report for the most recently launched run |
| `Ctrl+Alt+D` | Open the Change Review surface for the active project's most recent change set |
| `Ctrl+Alt+K` | Open the keyboard reference, from anywhere |
| `Ctrl+Alt+C` | Re-read the configuration file |
| `Tab` / `Shift+Tab` | Cycle keyboard focus between shell zones |

`Ctrl+Shift+P` is reserved for a command palette that does not exist yet — it is bound in the
keybinding policy and currently does nothing. That is why `tekstide --help` prints fifteen chords
while the policy holds sixteen.

## Within a surface

With `Tab` focused on the sidebar in Content mode, `Up`/`Down` move the explorer highlight and
`Enter` opens the highlighted file or directory.

With focus on the main area, typing edits the open document at the real cursor position;
`Up`/`Down`/`Left`/`Right` move the cursor without editing; `Enter` inserts a newline; and
`Backspace` deletes the character before the cursor. There is no undo.

## Modals

`Esc` dismisses and `Enter` activates the focused choice on every modal. Each choice also has a
real clickable button, routed through the same code the keyboard handling uses rather than a
second parallel path.

**This table is maintained by hand.** The application generates its own list from the keybinding
policy, so `tekstide --help` and the Help modal are the authority if this page ever disagrees with
them.
