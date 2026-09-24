//! **RFC-054 PR-054-C.** `[terminal] scrollback_lines`: how many lines a terminal
//! pane keeps above what is on screen.
//!
//! Scrollback is memory per pane, and a hidden pane keeps filling (`NFR-RES-003`,
//! `NFR-RES-004`), so the setting is **bounded by a measured cap** rather than a
//! number that felt large enough. The measurement and the arithmetic are in
//! `docs/src/users/configuration.md` and `rfcs/handoffs/054-user-configuration/
//! qa-evidence.md`; the test that holds a real pane to them is
//! `a_pane_at_the_cap_stays_under_the_memory_budget` in the terminal surface.

use super::model::{FallbackReason, SettingFallback};

/// What a pane keeps when nothing is configured. This is the value the terminal
/// surface has shipped with since RFC-017; it is unchanged.
pub const DEFAULT_SCROLLBACK_LINES: usize = 2_000;

/// **The measured cap.** One pane's memory budget is [`SCROLLBACK_BUDGET_BYTES`].
///
/// What was measured -- the allocator's own count (`alloc_probe`) over real
/// output through the real emulator, not a struct size multiplied out:
///
/// * a pane's memory is a fixed part plus its rows, and `alacritty_terminal`
///   allocates rows **in blocks of 1,024**: a 1,000-line and a 300-line history
///   cost the same, and 2,000 lines of history cost 2,048 rows' worth, not 2,000;
/// * a row costs `24` bytes a column (measured 23.5-23.8) plus the row list's own
///   `64`, and the fixed part is about `1,170` bytes a column (93,603 bytes at 80 columns, 231,963 at 200,
///   462,563 at 400);
/// * **the block count is not `ceil(lines / 1024)`**: across a sweep of history
///   sizes at 80, 200 and 400 columns a pane held up to `floor(lines / 1024) + 2`
///   blocks (11,040 lines held 12, not 11), so the model allows exactly that.
///
/// At 200 columns -- the widest pane an ordinary display gives a terminal --
/// 12,000 lines of history measure 55.7 MiB, and the model (never below any
/// measurement, and reading 62.0 MiB there) is under the 64 MiB budget. A wider
/// pane keeps proportionally fewer lines ([`effective_scrollback_lines`]), so
/// the budget holds at any width.
///
/// **What this does not cover:** output that puts a combining (zero-width)
/// character on every cell costs about 3.8 times as much, because each such cell
/// carries its own heap allocation. The cap is for ordinary output; the
/// configuration page says so.
pub const MAX_SCROLLBACK_LINES: usize = 12_000;

/// RFC-054 D7: one pane at the cap stays under this.
pub const SCROLLBACK_BUDGET_BYTES: usize = 64 * 1024 * 1024;

/// The measured cost model, chosen **never below what was measured**. The
/// terminal surface re-measures it every test run and fails if a dependency
/// update makes any of these an under-estimate.
pub const SCROLLBACK_BYTES_PER_CELL: usize = 24;
/// The row list itself (`Vec<Row>`, 32 bytes a row, up to twice as many rows'
/// worth reserved as are used).
pub const SCROLLBACK_BYTES_PER_ROW: usize = 64;
pub const SCROLLBACK_FIXED_BYTES_PER_COLUMN: usize = 1_200;
pub const SCROLLBACK_ROW_BLOCK: usize = 1_024;
/// Blocks held beyond `floor(lines / block)`, at the worst measured.
pub const SCROLLBACK_BLOCK_SLACK: usize = 2;

/// What the model says a pane `columns` wide holding `history` lines above
/// `screen_rows` on screen costs, at worst.
pub fn scrollback_bytes(history: usize, columns: usize, screen_rows: usize) -> usize {
    let blocks = (history + screen_rows) / SCROLLBACK_ROW_BLOCK + SCROLLBACK_BLOCK_SLACK;
    let rows = blocks * SCROLLBACK_ROW_BLOCK;
    let columns = columns.max(1);
    columns * SCROLLBACK_FIXED_BYTES_PER_COLUMN
        + rows * (SCROLLBACK_BYTES_PER_CELL * columns + SCROLLBACK_BYTES_PER_ROW)
}

/// How many lines of history a pane `columns` wide and `screen_rows` tall may
/// keep: what was configured, but never more than the memory budget allows at
/// that size.
pub fn effective_scrollback_lines(configured: usize, columns: usize, screen_rows: usize) -> usize {
    let columns = columns.max(1);
    let per_row = SCROLLBACK_BYTES_PER_CELL * columns + SCROLLBACK_BYTES_PER_ROW;
    let rows_by_budget = SCROLLBACK_BUDGET_BYTES
        .saturating_sub(SCROLLBACK_FIXED_BYTES_PER_COLUMN * columns)
        / per_row;
    let blocks_by_budget = rows_by_budget / SCROLLBACK_ROW_BLOCK;
    // `floor(lines / block) + slack <= blocks`  <=>  lines < (blocks - slack + 1) * block
    let lines_by_budget = (blocks_by_budget + 1)
        .saturating_sub(SCROLLBACK_BLOCK_SLACK)
        .saturating_mul(SCROLLBACK_ROW_BLOCK)
        .saturating_sub(1);
    configured.min(lines_by_budget.saturating_sub(screen_rows))
}

/// `[terminal]`: what survived. `None` means the shipped default.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerminalSettings {
    pub scrollback_lines: Option<usize>,
}

impl TerminalSettings {
    pub fn scrollback_lines(&self) -> usize {
        self.scrollback_lines.unwrap_or(DEFAULT_SCROLLBACK_LINES)
    }
}

/// `scrollback_lines` from the `[terminal]` table (the caller has already
/// refused the section's withdrawn and permanently-refused keys). A value that is
/// not a whole number, or is negative, falls back; one above the cap is **reduced
/// to it**, with a diagnostic -- D7's "clamps rather than being honoured".
pub(super) fn take_scrollback(
    table: &mut toml::Table,
    fallbacks: &mut Vec<SettingFallback>,
) -> TerminalSettings {
    let Some(value) = table.remove("scrollback_lines") else {
        return TerminalSettings::default();
    };
    let setting = "terminal.scrollback_lines".to_owned();
    match value {
        toml::Value::Integer(lines) if lines >= 0 => {
            let lines = usize::try_from(lines).unwrap_or(usize::MAX);
            if lines > MAX_SCROLLBACK_LINES {
                fallbacks.push(SettingFallback {
                    setting,
                    reason: FallbackReason::ScrollbackAboveCap,
                });
                TerminalSettings {
                    scrollback_lines: Some(MAX_SCROLLBACK_LINES),
                }
            } else {
                TerminalSettings {
                    scrollback_lines: Some(lines),
                }
            }
        }
        _ => {
            fallbacks.push(SettingFallback {
                setting,
                reason: FallbackReason::NotAWholeNumber,
            });
            TerminalSettings::default()
        }
    }
}
