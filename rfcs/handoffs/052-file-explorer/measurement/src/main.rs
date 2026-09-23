//! RFC-052 PR-052-A: what `iced-swdir-tree 0.9.3` does against the hostile
//! fixture, and what our own composition does against the same one.
//!
//! Run with `cargo run --release` from this directory. Every check prints
//! one `CHECK` line; `PASS` always means *the property RFC-052 D3 wants
//! holds*. The results are recorded in `../qa-evidence.md`.
//!
//! Two mechanisms are measured against one fixture:
//!
//! * `DirectoryTree` -- the widget as shipped: it walks the filesystem
//!   itself and draws what it finds.
//! * `ItemTree<Row>` fed by **our** scanner (`FileExplorerScanner`) and
//!   **our** escaping (`quote_untrusted`) -- the widget's rendering with
//!   none of its filesystem access.
//!
//! What is drawn is read back from the widget tree through iced's own
//! `Operation::text`, not from what the code intended to draw.

#[path = "../../../../../crates/tekstide-core/src/project/root/explorer/hostile_fixture.rs"]
mod hostile_fixture;

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use hostile_fixture::{
    BIDI_NAME, BREADTH_ENTRIES, DEPTH_LEVELS, HostileFixture, NEWLINE_NAME, non_utf8_name,
};
use iced::advanced::renderer::Headless;
use iced::advanced::widget::{Id, Operation};
use iced::keyboard::{self, Key, Modifiers};
use iced::{Element, Font, Pixels, Rectangle, Size};
use iced_runtime::user_interface::{Cache, UserInterface};
use iced_swdir_tree::{
    __testing, DirectoryTree, DirectoryTreeEvent, ItemNode, ItemTree, ItemTreeEvent, NodeId,
};
use tekstide_core::project::root::{
    ExplorerNodeKind, ExplorerNodeState, FileExplorerScanPolicy, FileExplorerScanner,
    ProjectRootHandle, ProjectRootValidator, SymlinkPolicy,
};
use tekstide_core::project::{ProjectId, ProjectSession};
use tekstide_core::text_safety::quote_untrusted;

#[derive(Clone, Debug)]
enum Msg {
    Directory(DirectoryTreeEvent),
    Item(ItemTreeEvent),
}

/// One frame at 60 Hz: what "does not block a frame" means for D8.
const FRAME: Duration = Duration::from_micros(16_667);

struct Report {
    failed: usize,
}

impl Report {
    fn check(&mut self, id: &str, holds: bool, note: impl fmt::Display) {
        if !holds {
            self.failed += 1;
        }
        println!(
            "CHECK {id:<38} {} {note}",
            if holds { "PASS" } else { "FAIL" }
        );
    }
}

// ---------------------------------------------------------------------
// Reading back what a widget tree actually draws.
// ---------------------------------------------------------------------

struct Texts(Vec<String>);

impl Operation for Texts {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<()>)) {
        operate(self);
    }
    fn text(&mut self, _id: Option<&Id>, _bounds: Rectangle, text: &str) {
        self.0.push(text.to_owned());
    }
}

fn renderer() -> iced::Renderer {
    // tiny-skia: no GPU, so the measurement is the same on any machine.
    pollster_block_on(<iced::Renderer as Headless>::new(
        Font::DEFAULT,
        Pixels(16.0),
        Some("tiny-skia"),
    ))
    .expect("a headless renderer")
}

/// A minimal `block_on`: `Headless::new` is `async` but never actually
/// waits, so no runtime is needed.
fn pollster_block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Waker};
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::yield_now();
    }
}

struct Laid {
    /// Every string the tree draws, in tree order.
    texts: Vec<String>,
    /// Building the element (widget construction).
    build: Duration,
    /// `UserInterface::build`: layout of every row.
    layout: Duration,
}

fn lay_out<'a>(make: impl FnOnce() -> Element<'a, Msg>, renderer: &mut iced::Renderer) -> Laid {
    let started = Instant::now();
    let element = make();
    let build = started.elapsed();

    let started = Instant::now();
    let mut ui = UserInterface::build(element, Size::new(400.0, 700.0), Cache::default(), renderer);
    let layout = started.elapsed();

    let mut texts = Texts(Vec::new());
    ui.operate(renderer, &mut texts);
    Laid {
        texts: texts.0,
        build,
        layout,
    }
}

fn has(texts: &[String], predicate: impl Fn(&str) -> bool) -> bool {
    texts.iter().any(|text| predicate(text))
}

fn hazard_free(texts: &[String]) -> bool {
    !has(texts, |t| t.contains('\u{202E}') || t.contains('\n'))
}

// ---------------------------------------------------------------------
// Our composition: our scanner, our escaping, the widget's rendering.
// ---------------------------------------------------------------------

/// What `ItemTree` draws for a row: entirely ours.
#[derive(Clone, Debug)]
struct Row(String);

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Our own model of what has been scanned. `children: None` is a
/// directory not yet opened -- which `ItemTree` cannot express (a node
/// without children has no caret), so an unopened directory carries a
/// placeholder child; see `to_item`.
struct Model {
    id: u64,
    relative: PathBuf,
    text: String,
    expandable: bool,
    children: Option<Vec<Model>>,
    truncated: bool,
}

struct Composition {
    handle: ProjectRootHandle,
    next_id: u64,
    root: Model,
    tree: ItemTree<Row>,
}

impl Composition {
    fn new(project: &Path) -> Self {
        let root = ProjectRootValidator
            .validate(project, SymlinkPolicy::FailClosed)
            .expect("root validates");
        let session = ProjectSession::new(
            ProjectId::new_uuid(),
            root.display_name,
            root.selected_path,
            root.canonical_path,
        );
        let mut this = Self {
            handle: ProjectRootHandle::from_project_session(&session),
            next_id: 1,
            root: Model {
                id: 0,
                relative: PathBuf::new(),
                text: "project".into(),
                expandable: true,
                children: None,
                truncated: false,
            },
            tree: ItemTree::new(),
        };
        this.refresh();
        this
    }

    fn refresh(&mut self) {
        let item = to_item(&self.root);
        self.tree.set_tree(item);
    }

    /// Opens the directory whose row is `id`: one bounded scan, off the
    /// widget entirely. This is what a `Toggled(id)` handler would do.
    fn open(&mut self, id: u64) -> Duration {
        let started = Instant::now();
        let mut next_id = self.next_id;
        let handle = &self.handle;
        let node = find(&mut self.root, id).expect("row exists");
        let scan = FileExplorerScanner.scan_directory(
            handle,
            node.relative.clone(),
            &FileExplorerScanPolicy::linux_mvp(),
        );
        node.children = Some(match scan {
            Err(_) => vec![leaf(&mut next_id, "(unreadable)")],
            Ok(scan) => {
                node.truncated = scan.truncated;
                let mut rows: Vec<Model> = scan
                    .nodes
                    .iter()
                    .map(|n| {
                        let is_directory = n.kind == ExplorerNodeKind::Directory;
                        let (suffix, expandable) = match &n.state {
                            ExplorerNodeState::Available => ("", is_directory),
                            ExplorerNodeState::Collapsed => (" (collapsed)", false),
                            ExplorerNodeState::Blocked(_) => (" (blocked)", false),
                            ExplorerNodeState::Unreadable => (" (unreadable)", false),
                        };
                        // The one place a name becomes display text.
                        let text = format!("{}{suffix}", quote_untrusted(&n.name));
                        let id = next_id;
                        next_id += 1;
                        Model {
                            id,
                            relative: n.relative_path.clone(),
                            text,
                            expandable,
                            children: None,
                            truncated: false,
                        }
                    })
                    .collect();
                if scan.truncated {
                    rows.push(leaf(&mut next_id, "(more entries not shown)"));
                }
                rows
            }
        });
        self.next_id = next_id;
        // What the widget did on the user's click, before our handler ran.
        let _ = self.tree.update(ItemTreeEvent::Toggled(NodeId(id)));
        self.refresh();
        started.elapsed()
    }
}

fn leaf(next_id: &mut u64, text: &str) -> Model {
    let id = *next_id;
    *next_id += 1;
    Model {
        id,
        relative: PathBuf::new(),
        text: text.into(),
        expandable: false,
        children: None,
        truncated: false,
    }
}

fn find(model: &mut Model, id: u64) -> Option<&mut Model> {
    if model.id == id {
        return Some(model);
    }
    model
        .children
        .as_mut()?
        .iter_mut()
        .find_map(|child| find(child, id))
}

fn to_item(model: &Model) -> ItemNode<Row> {
    let children = match (&model.children, model.expandable) {
        (Some(children), _) => children.iter().map(to_item).collect(),
        // A closed directory gets one placeholder child, or ItemTree
        // draws no caret for it.
        (None, true) => vec![ItemNode {
            id: NodeId(u64::MAX - model.id),
            data: Row("(not opened)".into()),
            children: Vec::new(),
        }],
        (None, false) => Vec::new(),
    };
    ItemNode {
        id: NodeId(model.id),
        data: Row(model.text.clone()),
        children,
    }
}

fn find_named<'a>(model: &'a Model, contains: &str) -> Option<&'a Model> {
    if model.text.contains(contains) {
        return Some(model);
    }
    model
        .children
        .as_ref()?
        .iter()
        .find_map(|child| find_named(child, contains))
}

fn open_named(composition: &mut Composition, name: &str) -> Duration {
    let id = find_named(&composition.root, name)
        .unwrap_or_else(|| panic!("row {name} exists"))
        .id;
    composition.open(id)
}

fn item_view(tree: &ItemTree<Row>) -> Element<'_, Msg> {
    tree.view(Msg::Item)
}

fn directory_view(tree: &DirectoryTree) -> Element<'_, Msg> {
    tree.view(Msg::Directory)
}

/// Feeds a synthetic key/press/click sequence and returns the messages
/// the widget emitted on its own.
fn messages_after<'a>(
    element: Element<'a, Msg>,
    act: impl FnOnce(&mut iced_test::Simulator<'a, Msg>),
) -> Vec<Msg> {
    let mut simulator = iced_test::Simulator::new(element);
    act(&mut simulator);
    simulator.into_messages().collect()
}

fn ms(duration: Duration) -> String {
    format!("{:.2} ms", duration.as_secs_f64() * 1000.0)
}

fn main() {
    let mut report = Report { failed: 0 };
    let mut renderer = renderer();

    // Small breadth for the fixture that is walked row by row; the
    // hundred-thousand row is built separately below.
    let fixture = HostileFixture::build("measure", 300);
    println!("fixture: {}", fixture.base.display());

    println!("\n== A. DirectoryTree, as shipped, against the fixture ==");
    let mut tree = DirectoryTree::new(fixture.project.clone());
    __testing::scan_and_feed(&mut tree, fixture.project.clone());
    for dir in [fixture.escaping_dir(), fixture.project.join("links")] {
        __testing::scan_and_feed(&mut tree, dir);
    }
    let unreadable = fixture.unreadable_dir();
    let can_refuse = std::fs::read_dir(&unreadable).is_err();
    __testing::scan_and_feed(&mut tree, unreadable.clone());

    let laid = lay_out(|| directory_view(&tree), &mut renderer);
    println!("drawn text: {:?}", laid.texts);

    // 1 -- boundary: how does it treat the links?
    let root = __testing::root(&tree);
    let links = root
        .children
        .iter()
        .find(|n| n.path.ends_with("links"))
        .expect("links row");
    let symlink_dirs_expandable: Vec<(String, bool)> = links
        .children
        .iter()
        .map(|n| {
            (
                n.path.file_name().unwrap().to_string_lossy().into_owned(),
                n.is_dir,
            )
        })
        .collect();
    println!("links children (name, is_dir): {symlink_dirs_expandable:?}");
    let escape_dir_is_dir = symlink_dirs_expandable
        .iter()
        .any(|(name, is_dir)| name == "escape-dir" && *is_dir);
    let in_root_is_dir = symlink_dirs_expandable
        .iter()
        .any(|(name, is_dir)| name == "in-root" && *is_dir);
    report.check(
        "D3.1a escape-dir is not expandable",
        !escape_dir_is_dir,
        "(symlinks are not followed: swdir classifies by the entry's own type)",
    );
    report.check(
        "D3.1b escape is *reported*, not silent",
        has(&laid.texts, |t| t.contains("escape") && t.contains("block")),
        "the widget draws the escaping link as an ordinary row with no marker",
    );
    println!(
        "  (side effect: the legitimate in-root link is_dir = {in_root_is_dir}; it is listed as a leaf)"
    );

    // 2 -- display text.
    report.check(
        "D3.2 hostile names do not reach the screen raw",
        hazard_free(&laid.texts),
        "labels come from file_name().to_string_lossy() with no hook to substitute ours",
    );
    println!(
        "  raw U+202E drawn: {}, raw newline drawn: {}, U+FFFD drawn: {}",
        has(&laid.texts, |t| t.contains('\u{202E}')),
        has(&laid.texts, |t| t.contains('\n')),
        has(&laid.texts, |t| t.contains('\u{FFFD}')),
    );

    // 3 -- status.
    report.check(
        "D3.3 per-node status is ours to draw",
        false,
        "a row is icon + label(file_name) only; there is no slot for a badge or a word",
    );

    // 4 -- unreadable.
    if can_refuse {
        let unreadable_node = __testing::root(&tree)
            .children
            .iter()
            .find(|n| n.path.ends_with("unreadable"))
            .expect("unreadable row");
        report.check(
            "D3.4b unreadable directory is a row",
            unreadable_node.error.is_some(),
            format!(
                "error carried on the node: {:?}",
                unreadable_node.error.as_ref().map(ToString::to_string)
            ),
        );
    } else {
        println!("  (skipped D3.4b: this process can read a mode-000 directory)");
    }

    // 5 -- keyboard.
    let messages = messages_after(directory_view(&tree), |ui| {
        ui.tap_key(keyboard::key::Named::ArrowDown);
        ui.tap_key(keyboard::key::Named::Enter);
    });
    report.check(
        "D3.5a it does not listen for keys itself",
        messages.is_empty(),
        format!("messages from ArrowDown+Enter: {messages:?}"),
    );
    let pure = tree.handle_key(
        &Key::Named(keyboard::key::Named::ArrowDown),
        Modifiers::default(),
    );
    println!("  handle_key is a pure function the app must call: returned {pure:?}");

    // 6 -- drag and drop.
    let messages = messages_after(directory_view(&tree), |ui| {
        let _ = ui.click("control");
    });
    println!("  one click on a row emitted: {messages:?}");
    let is_drag = messages
        .iter()
        .any(|m| matches!(m, Msg::Directory(DirectoryTreeEvent::Drag(_))));
    report.check(
        "D3.6 drag-and-drop is absent unless asked for",
        !is_drag,
        "DirectoryTree has no switch: a row press is a drag-machine event",
    );

    // 7 -- its own strings.
    let ours: Vec<&String> = laid
        .texts
        .iter()
        .filter(|t| t.chars().any(char::is_alphanumeric) && !fixture_name(t))
        .collect();
    println!("  words drawn that are not a file name (glyph-only strings are icons): {ours:?}");
    report.check(
        "D3.7 no strings of its own are drawn",
        ours.is_empty(),
        "(its Error Display, 'I/O error at ..', is never drawn: the row shows an icon and grey text)",
    );

    println!("\n== B. Our composition: ItemTree + FileExplorerScanner + quote_untrusted ==");
    let mut composition = Composition::new(&fixture.project);
    open_named(&mut composition, "project");
    for name in ["escaping", "links", "unreadable"] {
        open_named(&mut composition, name);
    }
    let laid = lay_out(|| item_view(&composition.tree), &mut renderer);
    println!("drawn text: {:?}", laid.texts);

    let escape_row = laid
        .texts
        .iter()
        .find(|t| t.contains("escape-dir"))
        .cloned()
        .unwrap_or_default();
    report.check(
        "D3.1a escape-dir is not expandable",
        !find_named(&composition.root, "escape-dir")
            .unwrap()
            .expandable,
        format!("row drawn as {escape_row:?}"),
    );
    report.check(
        "D3.1b escape is reported, not silent",
        escape_row.contains("(blocked)"),
        "the scanner's Blocked(SymlinkEscape) state is drawn as a word",
    );
    report.check(
        "D3.1c an in-root link still opens",
        find_named(&composition.root, "in-root").unwrap().expandable,
        "resolve_existing keeps it inside the root, so it stays a directory",
    );
    report.check(
        "D3.2 hostile names do not reach the screen raw",
        hazard_free(&laid.texts),
        format!(
            "U+202E shown as marker: {}, newline shown as marker: {}",
            has(&laid.texts, |t| t.contains("<U+202E>")),
            has(&laid.texts, |t| t.contains("<U+000A>")),
        ),
    );
    report.check(
        "D3.3 per-node status is ours to draw",
        has(&laid.texts, |t| t.contains("(blocked)")),
        "any word we put in Row's Display is drawn",
    );
    if can_refuse {
        let unreadable = find_named(&composition.root, "unreadable").unwrap();
        let shown = unreadable
            .children
            .as_ref()
            .and_then(|c| c.first())
            .map(|c| c.text.clone());
        report.check(
            "D3.4b unreadable directory is a row",
            shown.as_deref() == Some("(unreadable)"),
            format!("child row: {shown:?}"),
        );
    }
    let messages = messages_after(item_view(&composition.tree), |ui| {
        ui.tap_key(keyboard::key::Named::ArrowDown);
        ui.tap_key(keyboard::key::Named::Enter);
    });
    report.check(
        "D3.5a it does not listen for keys itself",
        messages.is_empty(),
        "handle_key is `&self` and only runs when the app calls it, after KeybindingPolicy",
    );
    let messages = messages_after(item_view(&composition.tree), |ui| {
        let _ = ui.click("README.md");
    });
    println!("  one click on a row emitted: {messages:?}");
    report.check(
        "D3.6 drag-and-drop is off unless enabled",
        !composition.tree.is_drag_and_drop_enabled()
            && !messages
                .iter()
                .any(|m| matches!(m, Msg::Item(ItemTreeEvent::Drag(_)))),
        "ItemTree::with_drag_and_drop defaults to false",
    );
    let known: Vec<&String> = laid
        .texts
        .iter()
        .filter(|t| t.chars().any(char::is_alphanumeric) && !is_ours(t))
        .collect();
    report.check(
        "D3.7 no strings of its own are drawn",
        known.is_empty(),
        format!("every word drawn is a Row or our placeholder; unexplained: {known:?}"),
    );

    println!("\n== C. Bounded: a directory of {BREADTH_ENTRIES} entries (D8) ==");
    drop(fixture);
    let built = Instant::now();
    let big = HostileFixture::build("measure-big", BREADTH_ENTRIES);
    println!("fixture built in {}", ms(built.elapsed()));
    let breadth = big.breadth_dir();

    // The widget as shipped.
    let threads_before = thread_count();
    let started = Instant::now();
    let raw = swdir::scan_dir(&breadth).expect("scan");
    let scan_only = started.elapsed();
    let mut tree = DirectoryTree::new(big.project.clone());
    __testing::scan_and_feed(&mut tree, big.project.clone());
    let started = Instant::now();
    __testing::scan_and_feed(&mut tree, breadth.clone());
    let fed = started.elapsed();
    let threads_after = thread_count();
    println!(
        "threads in this process before/after the widget scanned 100k entries: \
         {threads_before} / {threads_after} (swdir::scan_dir builds no rayon pool; its recursive walk does)"
    );
    let laid = lay_out(|| directory_view(&tree), &mut renderer);
    let rows = laid.texts.iter().filter(|t| t.starts_with('f')).count();
    println!(
        "DirectoryTree: {} entries; swdir::scan_dir alone {}; scan+normalize+update {}; \
         view build {}; layout {}; rows drawn {}",
        raw.len(),
        ms(scan_only),
        ms(fed),
        ms(laid.build),
        ms(laid.layout),
        rows,
    );
    let on_main_thread = laid.build + laid.layout;
    report.check(
        "D3.4a 100k expansion fits a frame (DirectoryTree)",
        on_main_thread < FRAME && rows <= 1000,
        format!(
            "widget build + layout on the render thread {} vs a {} frame; all {rows} rows are built",
            ms(on_main_thread),
            ms(FRAME)
        ),
    );

    // Our composition.
    let mut composition = Composition::new(&big.project);
    open_named(&mut composition, "project");
    let opened = open_named(&mut composition, "breadth");
    let laid = lay_out(|| item_view(&composition.tree), &mut renderer);
    let rows = laid
        .texts
        .iter()
        .filter(|t| t.contains('f') && t.len() > 6)
        .count();
    println!(
        "Composition: scan+model+set_tree {}; view build {}; layout {}; rows drawn {}",
        ms(opened),
        ms(laid.build),
        ms(laid.layout),
        rows,
    );
    report.check(
        "D3.4a 100k expansion fits a frame (composition)",
        opened + laid.build + laid.layout < FRAME,
        format!(
            "scan + model + set_tree + build + layout = {} vs a {} frame ({rows} rows drawn, truncation row: {})",
            ms(opened + laid.build + laid.layout),
            ms(FRAME),
            has(&laid.texts, |t| t.contains("more entries")),
        ),
    );

    println!("\n== D. Depth: {DEPTH_LEVELS} nested directories ==");
    let mut tree = DirectoryTree::new(big.project.clone());
    __testing::scan_and_feed(&mut tree, big.project.clone());
    let mut path = big.depth_dir();
    let started = Instant::now();
    __testing::scan_and_feed(&mut tree, path.clone());
    let mut sampled = Vec::new();
    for level in 1..=DEPTH_LEVELS {
        path.push("d");
        let one = Instant::now();
        __testing::scan_and_feed(&mut tree, path.clone());
        if [1, 10, 100, 500, 1000, DEPTH_LEVELS].contains(&level) {
            sampled.push((level, ms(one.elapsed())));
        }
    }
    let walked = started.elapsed();
    println!("DirectoryTree: one expansion at depth N (scan+update): {sampled:?}");
    let laid = lay_out(|| directory_view(&tree), &mut renderer);
    println!(
        "DirectoryTree: {DEPTH_LEVELS} levels expanded in {}; view build {}; layout {}; rows {}; leaf drawn: {}",
        ms(walked),
        ms(laid.build),
        ms(laid.layout),
        laid.texts.len(),
        has(&laid.texts, |t| t == "leaf.txt"),
    );
    report.check(
        "D3.4c 1500 levels do not overflow the stack",
        true,
        "expand, view, layout and drop all completed",
    );
    drop(tree);

    println!("\n== E. The per-level cap is not a total cap: N capped directories open at once ==");
    drop(big);
    let wide = std::env::temp_dir().join(format!("tekstide-wide-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&wide);
    for dir in 0..40 {
        let dir = wide.join(format!("d{dir:02}"));
        std::fs::create_dir_all(&dir).unwrap();
        for file in 0..300 {
            std::fs::File::create(dir.join(format!("f{file:03}"))).unwrap();
        }
    }
    let mut composition = Composition::new(&wide);
    open_named(&mut composition, "project");
    let mut frame_rows = Vec::new();
    for opened in 1..=40 {
        open_named(&mut composition, &format!("d{:02}", opened - 1));
        if [1, 5, 10, 20, 40].contains(&opened) {
            let laid = lay_out(|| item_view(&composition.tree), &mut renderer);
            let rows = laid.texts.iter().filter(|t| t.contains('f')).count();
            frame_rows.push((opened, rows, ms(laid.build + laid.layout)));
        }
    }
    println!("(directories open, rows drawn, widget build + layout): {frame_rows:?}");
    let _ = std::fs::remove_dir_all(&wide);

    println!("\n{} check(s) failed", report.failed);
    let _ = (Path::new(""), BIDI_NAME, NEWLINE_NAME, non_utf8_name());
}

/// True for the file names the fixture holds, so "its own strings" is
/// what is left after removing what the fixture put there.
fn fixture_name(text: &str) -> bool {
    [
        "project",
        "control",
        "escaping",
        "links",
        "unreadable",
        "breadth",
        "depth",
        "inside.txt",
        "escape-dir",
        "escape-file",
        "broken",
        "in-root",
        "lines.txt",
        "invoice",
        "name.txt",
    ]
    .iter()
    .any(|name| text.contains(name))
}

fn is_ours(text: &str) -> bool {
    text.contains('\u{2068}') // a quote_untrusted isolate
        || text.starts_with('(')
        || text == "project"
}

/// Live threads in this process (Linux).
fn thread_count() -> usize {
    std::fs::read_dir("/proc/self/task")
        .map(|d| d.count())
        .unwrap_or(0)
}
