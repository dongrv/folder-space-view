#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui;

mod ui;
mod treemap;
use std::{
    ffi::OsStr,
    ops::Range,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Debug)]
struct Node {
    // Only the final component is stored; the root path is kept once in ScanResult.
    name: Box<OsStr>,
    size: u64,
    files: u64,
    parent: usize,
    // Children are inserted together, so no per-directory index allocation is needed.
    children: Range<usize>,
    directory: bool,
}
impl Node {
    fn new(name: Box<OsStr>, parent: usize, directory: bool, size: u64) -> Self {
        Self {
            name,
            size,
            files: if directory { 0 } else { 1 },
            parent,
            children: 0..0,
            directory,
        }
    }
    fn name(&self) -> std::borrow::Cow<'_, str> {
        self.name.to_string_lossy()
    }
}
const ERROR_DETAIL_LIMIT: usize = 50;
const LARGEST_FILE_LIMIT: usize = 100;

struct ScanResult {
    root: PathBuf,
    nodes: Vec<Node>,
    skipped_links: u64,
    skipped_errors: u64,
    error_details: Vec<String>,
    largest_files: Vec<usize>,
}
impl ScanResult {
    fn path(&self, index: usize) -> PathBuf {
        node_path(&self.root, &self.nodes, index)
    }
}

fn node_path(root: &Path, nodes: &[Node], mut index: usize) -> PathBuf {
    let mut components = Vec::new();
    while index != 0 {
        let node = &nodes[index];
        components.push(node.name.as_ref());
        index = node.parent;
    }
    let mut path = root.to_path_buf();
    for component in components.into_iter().rev() {
        path.push(component);
    }
    path
}

fn is_link_or_special_directory(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_type().is_symlink()
            || (meta.is_dir() && meta.file_attributes() & 0x400 != 0)
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

fn aggregate(nodes: &mut [Node], cancel: &AtomicBool) -> bool {
    // Parents precede children; propagate one child at a time so cancellation stays responsive.
    for i in (1..nodes.len()).rev() {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let parent = nodes[i].parent;
        nodes[parent].size = nodes[parent].size.saturating_add(nodes[i].size);
        nodes[parent].files = nodes[parent].files.saturating_add(nodes[i].files);
    }
    !cancel.load(Ordering::Relaxed)
}

fn record_error(count: &mut u64, details: &mut Vec<String>, path: &Path, reason: impl std::fmt::Display) {
    *count += 1;
    if details.len() < ERROR_DETAIL_LIMIT {
        details.push(format!("{}：{reason}", path.display()));
    }
}

fn record_largest(largest: &mut Vec<usize>, nodes: &[Node], index: usize) {
    let position = largest.binary_search_by(|&other| {
        nodes[other].size.cmp(&nodes[index].size).reverse().then(other.cmp(&index))
    }).unwrap_or_else(|position| position);
    if position < LARGEST_FILE_LIMIT {
        if largest.len() == LARGEST_FILE_LIMIT {
            largest.pop();
        }
        largest.insert(position, index);
    }
}

// Iterate without following reparse points/symlinks; aggregate in reverse insertion order.
fn scan(
    root: PathBuf,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<Option<ScanResult>, String> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let meta = std::fs::symlink_metadata(&root).map_err(|e| format!("无法读取所选目录：{e}"))?;
    if is_link_or_special_directory(&meta) {
        return Err("所选目录是链接或 Windows 特殊目录，无法扫描".into());
    }
    if !meta.is_dir() {
        return Err("所选路径不是文件夹".into());
    }
    let mut nodes = vec![Node::new(Box::from(OsStr::new("")), 0, true, 0)];
    let mut stack = vec![0usize];
    let mut skipped_links = 0u64;
    let mut skipped_errors = 0u64;
    let mut error_details = Vec::new();
    let mut largest_files = Vec::new();
    let mut seen = 0u64;
    let mut last = Instant::now();
    while let Some(index) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let directory_path = node_path(&root, &nodes, index);
        let entries = match std::fs::read_dir(&directory_path) {
            Ok(v) => v,
            Err(e) => {
                if index == 0 {
                    return Err(format!("无法读取所选目录：{e}"));
                }
                record_error(&mut skipped_errors, &mut error_details, &directory_path, e);
                continue;
            }
        };
        let first_child = nodes.len();
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let entry = match entry {
                Ok(v) => v,
                Err(e) => {
                    record_error(&mut skipped_errors, &mut error_details, &directory_path, e);
                    continue;
                }
            };
            let entry_path = entry.path();
            let meta = match std::fs::symlink_metadata(&entry_path) {
                Ok(v) => v,
                Err(e) => {
                    record_error(&mut skipped_errors, &mut error_details, &entry_path, e);
                    continue;
                }
            };
            if is_link_or_special_directory(&meta) {
                skipped_links += 1;
                continue;
            }
            let directory = meta.is_dir();
            if !directory && !meta.is_file() {
                record_error(&mut skipped_errors, &mut error_details, &entry_path, "不是普通文件或目录");
                continue;
            }
            let child = nodes.len();
            nodes.push(Node::new(
                entry.file_name().into_boxed_os_str(),
                index,
                directory,
                if directory { 0 } else { meta.len() },
            ));
            if directory {
                stack.push(child);
            } else {
                record_largest(&mut largest_files, &nodes, child);
            }
            seen += 1;
            if last.elapsed() >= Duration::from_millis(150) {
                progress(seen);
                last = Instant::now();
            }
        }
        nodes[index].children = first_child..nodes.len();
    }
    if !aggregate(&mut nodes, cancel) {
        return Ok(None);
    }
    Ok(Some(ScanResult {
        root,
        nodes,
        skipped_links,
        skipped_errors,
        error_details,
        largest_files,
    }))
}

fn size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut n = bytes as f64;
    let mut i = 0;
    while n >= 1024.0 && i < UNITS.len() - 1 {
        n /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{n:.2} {}", UNITS[i])
    }
}
fn percent(part: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (part as f64 / total as f64 * 100.0) as f32
    }
}

struct App {
    theme: ui::Theme,
    view: ui::View,
    show_errors: bool,
    result: Option<ScanResult>,
    trail: Vec<usize>,
    rx: Option<Receiver<u64>>,
    cancel: Option<Arc<AtomicBool>>,
    worker: Option<JoinHandle<Result<Option<ScanResult>, String>>>,
    pending: Option<(PathBuf, egui::Context)>,
    scanned: u64,
    search: String,
    sort_name: bool,
    selected: Option<usize>,
    cached_key: Option<(usize, String, bool)>,
    cached_children: Vec<usize>,
    map: ui::MapCache,
    message: Option<String>,
    last_path: Option<PathBuf>,
}
impl Default for App {
    fn default() -> Self {
        Self {
            theme: ui::Theme::Light,
            view: ui::View::Treemap,
            show_errors: false,
            result: None,
            trail: vec![0],
            rx: None,
            cancel: None,
            worker: None,
            pending: None,
            scanned: 0,
            search: String::new(),
            sort_name: false,
            selected: None,
            cached_key: None,
            cached_children: Vec::new(),
            map: ui::MapCache::default(),
            message: None,
            last_path: None,
        }
    }
}
impl App {
    fn start(&mut self, path: PathBuf, ctx: egui::Context) {
        if self.worker.is_some() {
            self.stop();
            self.pending = Some((path, ctx));
            return;
        }
        self.last_path = Some(path.clone());
        self.scanned = 0;
        self.message = None;
        self.result = None;
        self.view = ui::View::Treemap;
        self.show_errors = false;
        self.trail = vec![0];
        self.selected = None;
        self.cached_key = None;
        self.cached_children = Vec::new();
        self.map = ui::MapCache::default();
        self.search.clear();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let worker = thread::spawn(move || {
            let result = scan(path, &flag, |n| {
                let _ = tx.send(n);
                ctx.request_repaint();
            });
            ctx.request_repaint();
            result
        });
        self.rx = Some(rx);
        self.cancel = Some(cancel);
        self.worker = Some(worker);
    }
    fn stop(&mut self) {
        self.pending = None;
        if let Some(c) = &self.cancel {
            c.store(true, Ordering::Relaxed);
        }
    }
    fn stopping(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Relaxed))
    }
    fn events(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.rx {
            while let Ok(n) = rx.try_recv() {
                self.scanned = n;
            }
        }
        if self.worker.as_ref().is_some_and(JoinHandle::is_finished) {
            let worker = self.worker.take().unwrap();
            let cancelled = self.stopping();
            self.cancel = None;
            self.rx = None;
            match worker.join() {
                Err(_) => self.message = Some("后台扫描线程异常退出（panic）".into()),
                Ok(_) if cancelled => self.message = Some("扫描已取消".into()),
                Ok(Ok(Some(result))) => self.result = Some(result),
                Ok(Ok(None)) => self.message = Some("扫描已取消".into()),
                Ok(Err(error)) => self.message = Some(error),
            }
            if let Some((path, ctx)) = self.pending.take() {
                self.start(path, ctx);
            }
        }
        if self.worker.is_some() {
            // A panicked worker cannot request a repaint, so poll without blocking the UI.
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.stop();
    }
}

fn main() -> eframe::Result {
    let initial_path = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1050.0, 740.0])
            .with_min_inner_size([700.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "空间视图",
        options,
        Box::new(move |cc| {
            ui::configure(&cc.egui_ctx);
            let mut app = App::default();
            if let Some(path) = initial_path {
                app.start(path, cc.egui_ctx.clone());
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait_for_worker(app: &App) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.worker.as_ref().unwrap().is_finished() {
            assert!(Instant::now() < deadline, "worker did not finish");
            thread::yield_now();
        }
    }

    #[test]
    fn formats_and_percentages() {
        assert_eq!(size(1024), "1.00 KB");
        assert_eq!(percent(10, 0), 0.0);
        assert_eq!(percent(25, 100), 25.0);
    }
    #[test]
    fn scans_nested_files_and_skips_links() {
        let root = std::env::temp_dir().join(format!("disk-insight-test-{}", std::process::id()));
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("a"), [0; 7]).unwrap();
        std::fs::write(root.join("nested/b"), [0; 11]).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
        let result = scan(root.clone(), &AtomicBool::new(false), |_| {})
            .unwrap()
            .unwrap();
        assert_eq!(result.nodes[0].size, 18);
        assert_eq!(result.nodes[0].files, 2);
        let folder = result.nodes[0].children.clone()
            .find(|&i| result.nodes[i].name() == "nested")
            .unwrap();
        let file = result.nodes[folder].children.clone().next().unwrap();
        assert_eq!(result.nodes[folder].size, 11);
        assert_eq!(result.nodes[folder].files, 1);
        assert_eq!(result.path(folder), root.join("nested"));
        assert_eq!(result.path(file), root.join("nested/b"));
        assert_eq!(result.largest_files, vec![file, result.nodes[0].children.clone().find(|&i| result.nodes[i].name() == "a").unwrap()]);
        #[cfg(unix)]
        assert_eq!(result.skipped_links, 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn largest_files_include_empty_files() {
        let root = std::env::temp_dir().join(format!("disk-insight-empty-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("empty"), []).unwrap();
        let result = scan(root.clone(), &AtomicBool::new(false), |_| {}).unwrap().unwrap();
        assert_eq!(result.largest_files.len(), 1);
        assert_eq!(result.path(result.largest_files[0]), root.join("empty"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn largest_files_are_bounded_and_ties_use_node_index() {
        let root = std::env::temp_dir().join(format!("disk-insight-ranking-{}", std::process::id()));
        std::fs::create_dir_all(root.join("nested")).unwrap();
        for i in 0..105 {
            std::fs::write(root.join("nested").join(format!("file-{i}")), [0; 3]).unwrap();
        }
        std::fs::write(root.join("empty"), []).unwrap();
        let result = scan(root.clone(), &AtomicBool::new(false), |_| {}).unwrap().unwrap();
        let mut expected: Vec<_> = result.nodes.iter().enumerate()
            .filter_map(|(i, node)| (!node.directory && node.size == 3).then_some(i))
            .collect();
        expected.truncate(LARGEST_FILE_LIMIT);
        assert_eq!(result.largest_files, expected);
        assert_eq!(result.largest_files.len(), LARGEST_FILE_LIMIT);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn error_details_are_bounded_and_lazily_formatted() {
        struct Reason(bool);
        impl std::fmt::Display for Reason {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                assert!(self.0, "reason formatted after limit");
                f.write_str("failure")
            }
        }
        let mut count = 0;
        let mut details = Vec::new();
        for i in 0..ERROR_DETAIL_LIMIT + 3 {
            record_error(&mut count, &mut details, Path::new("actual-path"), Reason(i < ERROR_DETAIL_LIMIT));
        }
        assert_eq!(count, (ERROR_DETAIL_LIMIT + 3) as u64);
        assert_eq!(details.len(), ERROR_DETAIL_LIMIT);
        assert!(details.iter().all(|detail| detail.contains("actual-path") && detail.contains("failure")));
    }

    #[cfg(unix)]
    #[test]
    fn non_regular_entries_have_real_error_details() {
        use std::os::unix::net::UnixListener;
        let root = std::env::temp_dir().join(format!("disk-insight-special-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("socket");
        let _socket = UnixListener::bind(&path).unwrap();
        let result = scan(root.clone(), &AtomicBool::new(false), |_| {}).unwrap().unwrap();
        assert_eq!(result.skipped_errors, 1);
        assert!(result.error_details[0].contains(&path.to_string_lossy().into_owned()));
        assert!(result.error_details[0].contains("不是普通文件或目录"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_honours_preexisting_cancellation() {
        assert!(scan(PathBuf::from("unused"), &AtomicBool::new(true), |_| panic!("unexpected progress"))
            .unwrap().is_none());
    }

    #[test]
    fn panic_ends_scanning_and_reports_failure() {
        let mut app = App::default();
        app.cancel = Some(Arc::new(AtomicBool::new(false)));
        app.worker = Some(thread::spawn(|| panic!("simulated scan panic")));
        wait_for_worker(&app);
        app.events(&egui::Context::default());
        assert!(app.worker.is_none());
        assert!(app.cancel.is_none());
        assert!(app.message.as_deref().unwrap().contains("panic"));
    }

    #[test]
    fn replacement_waits_for_cancelled_worker() {
        let root = std::env::temp_dir().join(format!("disk-insight-replace-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("file"), [0; 7]).unwrap();
        let (release, blocked) = mpsc::channel();
        let mut app = App::default();
        app.cancel = Some(Arc::new(AtomicBool::new(false)));
        app.worker = Some(thread::spawn(move || {
            blocked.recv().unwrap();
            Ok(None)
        }));
        app.last_path = Some(PathBuf::from("previous"));
        let ctx = egui::Context::default();
        app.start(root.clone(), ctx.clone());
        assert!(app.stopping());
        app.events(&ctx);
        assert!(app.pending.is_some());
        assert_eq!(app.last_path.as_deref(), Some(Path::new("previous")));
        release.send(()).unwrap();
        wait_for_worker(&app);
        app.events(&ctx);
        assert!(app.pending.is_none());
        assert_eq!(app.last_path.as_deref(), Some(root.as_path()));
        wait_for_worker(&app);
        app.events(&ctx);
        assert_eq!(app.result.as_ref().unwrap().nodes[0].size, 7);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancelling_waits_and_discards_a_late_result() {
        let (release, blocked) = mpsc::channel();
        let mut app = App::default();
        app.cancel = Some(Arc::new(AtomicBool::new(false)));
        app.worker = Some(thread::spawn(move || {
            blocked.recv().unwrap();
            Ok(Some(ScanResult {
                root: PathBuf::from("late"),
                nodes: vec![Node::new(Box::from(OsStr::new("")), 0, true, 0)],
                skipped_links: 0,
                skipped_errors: 0,
                error_details: Vec::new(),
                largest_files: Vec::new(),
            }))
        }));
        let ctx = egui::Context::default();
        app.start(PathBuf::from("not-started"), ctx.clone());
        assert!(app.pending.is_some());
        app.stop();
        assert!(app.pending.is_none());
        app.events(&ctx);
        assert!(app.worker.is_some());
        assert!(app.stopping());
        release.send(()).unwrap();
        wait_for_worker(&app);
        app.events(&ctx);
        assert!(app.worker.is_none());
        assert!(app.result.is_none());
        assert_eq!(app.message.as_deref(), Some("扫描已取消"));
    }

    #[test]
    fn aggregation_honours_cancellation() {
        let mut nodes = vec![
            Node::new(Box::from(OsStr::new("")), 0, true, 0),
            Node::new(Box::from(OsStr::new("file")), 0, false, 7),
        ];
        assert!(!aggregate(&mut nodes, &AtomicBool::new(true)));
        assert_eq!(nodes[0].size, 0);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_root_and_preserves_non_unicode_paths() {
        use std::os::unix::{ffi::OsStringExt, fs::symlink};
        let root = std::env::temp_dir().join(format!("disk-insight-links-{}", std::process::id()));
        let target = root.join("target");
        std::fs::create_dir_all(&target).unwrap();
        let name = std::ffi::OsString::from_vec(vec![b'f', 0x80]);
        std::fs::write(target.join(&name), [0; 3]).unwrap();
        let link = root.join("link");
        symlink(&target, &link).unwrap();
        let error = scan(link, &AtomicBool::new(false), |_| {}).err().unwrap();
        assert!(error.contains("链接"));
        let result = scan(target.clone(), &AtomicBool::new(false), |_| {}).unwrap().unwrap();
        let file = result.nodes[0].children.clone().next().unwrap();
        assert_eq!(result.path(file), target.join(name));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn rejects_junction_root_and_skips_junction_child() {
        let root = std::env::temp_dir().join(format!("disk-insight-junction-{}", std::process::id()));
        let target = root.join("target");
        let junction = root.join("junction");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("file"), [0; 5]).unwrap();
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .status()
            .unwrap();
        assert!(status.success(), "failed to create junction");
        let error = scan(junction.clone(), &AtomicBool::new(false), |_| {}).err().unwrap();
        assert!(error.contains("特殊目录"));
        let result = scan(root.clone(), &AtomicBool::new(false), |_| {}).unwrap().unwrap();
        assert_eq!(result.skipped_links, 1);
        assert_eq!(result.nodes[0].size, 5);
        std::fs::remove_dir(junction).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
