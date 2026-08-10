//! RFC-015 PR-015-D: the rendered-surface contract, and the Project
//! Board -- the first surface to implement it.
//!
//! RFC-015 describes a surface as declaring **identity** (which
//! `AppRoute`/`ProjectOpenSurface` it serves), **view** (a pure function
//! of core state to a widget tree, no interior mutable state), **input
//! interest** (`None`, `Keyboard`, or `TextStream`), **focus zones**, and
//! **status contribution**. This slice deliberately implements that
//! shape as concrete methods on [`board::ProjectBoard`] rather than a
//! `trait Surface` -- with exactly one implementor, a trait would be
//! abstraction with no second case to generalize from (this codebase's
//! own precedent: `Theme::border_focused`, PR-015-B, was cut for the
//! same reason -- no caller, no trait). RFC-019/RFC-020, whichever
//! writes the second surface, is where a real trait becomes worth its
//! weight, generalizing from two concrete shapes instead of guessing at
//! one.
//!
//! **Input interest is `None` for this slice.** Nothing in PR-015-D's
//! scope asks for row selection or per-row keyboard interaction --
//! "project rows... attention ordering," not "open a project." The
//! board is read-only display for now; whichever slice adds "select a
//! row and open it" is where `SurfaceInput` gets a real consumer.
//!
//! **Surface code cannot reach the modal layer or render trusted
//! chrome**, enforced by what this module is simply never given: no
//! function here takes `&mut shell::State` or anything that could touch
//! `state.modal`, and `board::view` returns only the content-area
//! `Element` `shell::content_area` slots in -- it has no path to
//! `top_bar`/`status_bar`.

pub mod board;
pub mod explorer;
pub mod terminal;
