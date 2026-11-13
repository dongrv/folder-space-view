#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui;

mod ui;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
struct Node {
    path: PathBuf,
    size: u64,
    files: u64,
    directory: bool,
    children: Vec<usize>,
}
impl Node {
    fn new(path: PathBuf, directory: bool, size: u64) -> Self {
        Self {
            path,
            size,
            files: if directory { 0 } else { 1 },
            directory,
            children: Vec::new(),
        }
    }
    fn name(&self) -> String {
        self.path
            .file_name()
            .unwrap_or(self.path.as_os_str())
            .to_string_lossy()
            .into_owned()
    }
}
struct ScanResult {
    nodes: Vec<Node>,
    skipped_links: u64,
    skipped_errors: u64,
    first_error: Option<String>,
}
enum Event {
    Progress(u64),
    Finished(ScanResult),
    Failed(String),
}

// Iterate without following reparse points/symlinks; aggregate in reverse insertion order.
fn scan(
    root: PathBuf,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<Option<ScanResult>, String> {
    let meta = std::fs::symlink_metadata(&root).map_err(|e| format!("无法读取所选目录：{e}"))?;
    if !meta.is_dir() {
        return Err("所选路径不是文件夹".into());
    }
    let mut nodes = vec![Node::new(root, true, 0)];
    let mut stack = vec![0usize];
    let mut skipped_links = 0u64;
    let mut skipped_errors = 0u64;
    let mut first_error = None;
    let mut seen = 0u64;
    let mut last = Instant::now();
    while let Some(index) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let entries = match std::fs::read_dir(&nodes[index].path) {
            Ok(v) => v,
            Err(e) => {
                if index == 0 {
                    return Err(format!("无法读取所选目录：{e}"));
                }
                skipped_errors += 1;
                first_error.get_or_insert_with(|| format!("{}：{e}", nodes[index].path.display()));
                continue;
            }
        };
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let entry = match entry {
                Ok(v) => v,
                Err(e) => {
                    skipped_errors += 1;
                    first_error
                        .get_or_insert_with(|| format!("{}：{e}", nodes[index].path.display()));
                    continue;
                }
            };
            let meta = match std::fs::symlink_metadata(entry.path()) {
                Ok(v) => v,
                Err(e) => {
                    skipped_errors += 1;
                    first_error.get_or_insert_with(|| format!("{}：{e}", entry.path().display()));
                    continue;
                }
            };
            // Windows junctions are reparse points even when reported as directories.
            #[cfg(windows)]
            let is_link = {
                use std::os::windows::fs::MetadataExt;
                meta.file_type().is_symlink()
                    || (meta.is_dir() && meta.file_attributes() & 0x400 != 0)
            };
            #[cfg(not(windows))]
            let is_link = meta.file_type().is_symlink();
            if is_link {
                skipped_links += 1;
                continue;
            }
            if !meta.is_dir() && !meta.is_file() {
                skipped_errors += 1;
                continue;
            }
            let child = nodes.len();
            nodes.push(Node::new(
                entry.path(),
                meta.is_dir(),
                if meta.is_file() { meta.len() } else { 0 },
            ));
            nodes[index].children.push(child);
            if meta.is_dir() {
                stack.push(child);
            }
            seen += 1;
            if last.elapsed() >= Duration::from_millis(150) {
                progress(seen);
                last = Instant::now();
            }
        }
    }
    // Every parent is inserted before its children, so reverse order is postorder.
    for i in (0..nodes.len()).rev() {
        let (size, files) = nodes[i].children.iter().fold((0u64, 0u64), |(s, f), &c| {
            (
                s.saturating_add(nodes[c].size),
                f.saturating_add(nodes[c].files),
            )
        });
        if nodes[i].directory {
            nodes[i].size = size;
            nodes[i].files = files;
        }
    }
    Ok(Some(ScanResult {
        nodes,
        skipped_links,
        skipped_errors,
        first_error,
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
    result: Option<ScanResult>,
    trail: Vec<usize>,
    rx: Option<Receiver<Event>>,
    cancel: Option<Arc<AtomicBool>>,
    scanned: u64,
    search: String,
    sort_name: bool,
    selected: Option<usize>,
    cached_key: Option<(usize, String, bool)>,
    cached_children: Vec<usize>,
    cached_chart_current: Option<usize>,
    cached_chart: Vec<usize>,
    message: Option<String>,
    last_path: Option<PathBuf>,
}
impl Default for App {
    fn default() -> Self {
        Self {
            result: None,
            trail: vec![0],
            rx: None,
            cancel: None,
            scanned: 0,
            search: String::new(),
            sort_name: false,
            selected: None,
            cached_key: None,
            cached_children: Vec::new(),
            cached_chart_current: None,
            cached_chart: Vec::new(),
            message: None,
            last_path: None,
        }
    }
}
impl App {
    fn start(&mut self, path: PathBuf, ctx: egui::Context) {
        self.stop();
        self.last_path = Some(path.clone());
        self.scanned = 0;
        self.message = None;
        self.result = None;
        self.trail = vec![0];
        self.selected = None;
        self.cached_key = None;
        self.cached_children.clear();
        self.cached_chart_current = None;
        self.cached_chart.clear();
        self.search.clear();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        thread::spawn(move || {
            let result = scan(path, &flag, |n| {
                let _ = tx.send(Event::Progress(n));
                ctx.request_repaint();
            });
            let event = match result {
                Ok(Some(r)) => Some(Event::Finished(r)),
                Ok(None) => None,
                Err(e) => Some(Event::Failed(e)),
            };
            if let Some(event) = event {
                let _ = tx.send(event);
                ctx.request_repaint();
            }
        });
        self.rx = Some(rx);
        self.cancel = Some(cancel);
    }
    fn stop(&mut self) {
        if let Some(c) = self.cancel.take() {
            c.store(true, Ordering::Relaxed);
        }
        self.rx = None;
    }
    fn events(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    Event::Progress(n) => self.scanned = n,
                    Event::Finished(r) => {
                        self.result = Some(r);
                        self.cancel = None;
                    }
                    Event::Failed(e) => {
                        self.message = Some(e);
                        self.cancel = None;
                    }
                }
            }
        }
        if self.cancel.is_none() {
            self.rx = None;
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
        #[cfg(unix)]
        assert_eq!(result.skipped_links, 1);
        std::fs::remove_dir_all(root).unwrap();
    }
}
