use crate::{percent, size, App, ScanResult};
use eframe::egui::{
    self, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, RichText,
    Sense, Stroke, Vec2,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum View {
    Treemap,
    Directory,
    LargestFiles,
}

type MapItem = (Option<usize>, u64, usize);
type MapTile = (Option<usize>, u64, usize, Rect);

// Cached by directory: layout scratch buffers are reused on every frame.
#[derive(Default)]
pub(crate) struct MapCache {
    current: Option<usize>,
    top: Vec<usize>,
    nested: Vec<Vec<usize>>,
    items: Vec<MapItem>,
    tiles: Vec<MapTile>,
    nested_tiles: Vec<MapTile>,
}

const MAP_TOP_LIMIT: usize = 64;
const MAP_NESTED_LIMIT: usize = 32;

#[derive(Clone, Copy)]
struct Palette {
    bg: Color32,
    surface: Color32,
    line: Color32,
    text: Color32,
    muted: Color32,
    accent: Color32,
    accent_text: Color32,
    widget: Color32,
    hover: Color32,
    selected: Color32,
    warning: Color32,
    folder: Color32,
    chart_bg: Color32,
    chart_other: Color32,
    chart_text: Color32,
    chart: [Color32; 8],
}

impl Theme {
    fn palette(self) -> Palette {
        match self {
            Self::Light => Palette {
                bg: Color32::from_rgb(245, 247, 250),
                surface: Color32::WHITE,
                line: Color32::from_rgb(218, 226, 235),
                text: Color32::from_rgb(24, 37, 55),
                muted: Color32::from_rgb(80, 94, 112),
                accent: Color32::from_rgb(31, 96, 196),
                accent_text: Color32::WHITE,
                widget: Color32::from_rgb(237, 242, 248),
                hover: Color32::from_rgb(231, 237, 245),
                selected: Color32::from_rgb(226, 237, 253),
                warning: Color32::from_rgb(142, 75, 16),
                folder: Color32::from_rgb(177, 126, 36),
                chart_bg: Color32::from_rgb(237, 242, 248),
                chart_other: Color32::from_rgb(214, 223, 233),
                chart_text: Color32::from_rgb(24, 37, 55),
                chart: [
                    Color32::from_rgb(189, 228, 205), Color32::from_rgb(200, 227, 185),
                    Color32::from_rgb(214, 228, 176), Color32::from_rgb(232, 224, 162),
                    Color32::from_rgb(240, 210, 167), Color32::from_rgb(239, 193, 168),
                    Color32::from_rgb(238, 179, 170), Color32::from_rgb(234, 168, 171),
                ],
            },
            Self::Dark => Palette {
                bg: Color32::from_rgb(20, 27, 37),
                surface: Color32::from_rgb(28, 38, 52),
                line: Color32::from_rgb(58, 74, 93),
                text: Color32::from_rgb(235, 240, 247),
                muted: Color32::from_rgb(180, 192, 207),
                accent: Color32::from_rgb(139, 185, 255),
                accent_text: Color32::from_rgb(14, 29, 48),
                widget: Color32::from_rgb(45, 57, 73),
                hover: Color32::from_rgb(55, 69, 87),
                selected: Color32::from_rgb(47, 70, 98),
                warning: Color32::from_rgb(255, 195, 129),
                folder: Color32::from_rgb(231, 184, 92),
                chart_bg: Color32::from_rgb(35, 46, 61),
                chart_other: Color32::from_rgb(63, 75, 90),
                chart_text: Color32::from_rgb(249, 251, 255),
                chart: [
                    Color32::from_rgb(35, 83, 58), Color32::from_rgb(52, 86, 49),
                    Color32::from_rgb(68, 86, 39), Color32::from_rgb(87, 79, 31),
                    Color32::from_rgb(100, 70, 34), Color32::from_rgb(107, 62, 40),
                    Color32::from_rgb(112, 54, 49), Color32::from_rgb(110, 45, 52),
                ],
            },
        }
    }

    fn toggle(&mut self, ctx: &egui::Context) {
        *self = match self { Self::Light => Self::Dark, Self::Dark => Self::Light };
        apply_theme(ctx, *self);
    }
}

pub fn configure(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for path in [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyh.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ] {
        if let Ok(data) = std::fs::read(path) {
            fonts
                .font_data
                .insert("chinese".into(), Arc::new(FontData::from_owned(data)));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "chinese".into());
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .push("chinese".into());
            break;
        }
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.spacing.button_padding = Vec2::new(9.0, 5.0);
    style.spacing.item_spacing = Vec2::new(7.0, 4.0);
    ctx.set_style(style);
    apply_theme(ctx, Theme::Light);
}

fn apply_theme(ctx: &egui::Context, theme: Theme) {
    let palette = theme.palette();
    let mut visuals = match theme {
        Theme::Light => egui::Visuals::light(),
        Theme::Dark => egui::Visuals::dark(),
    };
    visuals.panel_fill = palette.bg;
    visuals.window_fill = palette.surface;
    visuals.extreme_bg_color = palette.bg;
    visuals.faint_bg_color = palette.widget;
    visuals.override_text_color = Some(palette.text);
    visuals.widgets.noninteractive.bg_fill = palette.surface;
    visuals.widgets.noninteractive.bg_stroke.color = palette.line;
    visuals.widgets.inactive.bg_fill = palette.widget;
    visuals.widgets.hovered.bg_fill = palette.hover;
    visuals.widgets.active.bg_fill = palette.selected;
    visuals.selection.bg_fill = palette.selected;
    visuals.selection.stroke.color = palette.accent;
    visuals.hyperlink_color = palette.accent;
    ctx.set_visuals(visuals);
}
fn txt(ui: &mut egui::Ui, value: impl Into<String>, px: f32, color: Color32) {
    ui.label(RichText::new(value.into()).size(px).color(color));
}
fn fit_text(ui: &egui::Ui, value: &str, px: f32, width: f32) -> String {
    let fits = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), FontId::proportional(px), ui.visuals().text_color())
            .size()
            .x
            <= width
    };
    if fits(value) {
        return value.to_owned();
    }
    let chars: Vec<char> = value.chars().collect();
    let (mut low, mut high) = (0, chars.len());
    while low < high {
        let middle = (low + high + 1) / 2;
        let candidate = format!("{}…", chars[..middle].iter().collect::<String>());
        if fits(&candidate) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    if fits("…") {
        format!("{}…", chars[..low].iter().collect::<String>())
    } else {
        String::new()
    }
}
fn button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).size(13.0)).corner_radius(5))
}
fn primary(ui: &mut egui::Ui, text: &str, palette: Palette) -> egui::Response {
    ui.scope(|ui| {
        let dark = ui.visuals().dark_mode;
        let visuals = ui.visuals_mut();
        visuals.widgets.inactive.bg_fill = palette.accent;
        visuals.widgets.hovered.bg_fill = palette.accent.gamma_multiply(if dark { 1.1 } else { 0.9 });
        visuals.widgets.active.bg_fill = palette.accent.gamma_multiply(if dark { 1.15 } else { 0.85 });
        ui.add(
            egui::Button::new(
                RichText::new(text)
                    .size(13.0)
                    .strong()
                    .color(palette.accent_text),
            )
            .corner_radius(5),
        )
    }).inner
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Select(usize),
    Enter(usize),
    Back,
    GoTo(usize),
    Open(usize),
    Copy(usize),
    Locate(usize),
}

impl App {
    fn apply_action(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Select(i) => self.selected = Some(i),
            Action::Enter(i) => {
                if let Some(result) = &self.result {
                    if result.nodes[i].directory {
                        let mut node = i;
                        let mut trail = vec![node];
                        while node != 0 {
                            node = result.nodes[node].parent;
                            trail.push(node);
                        }
                        trail.reverse();
                        self.trail = trail;
                        self.selected = None;
                        self.search.clear();
                    } else {
                        self.selected = Some(i);
                    }
                }
            }
            Action::Back => {
                if self.trail.len() > 1 {
                    self.trail.pop();
                    self.selected = None;
                    self.search.clear();
                }
            }
            Action::GoTo(level) => {
                self.trail.truncate(level + 1);
                self.selected = None;
                self.search.clear();
            }
            Action::Locate(i) => {
                if let Some(result) = &self.result {
                    let mut parent = result.nodes[i].parent;
                    let mut trail = vec![parent];
                    while parent != 0 {
                        parent = result.nodes[parent].parent;
                        trail.push(parent);
                    }
                    trail.reverse();
                    self.trail = trail;
                    self.view = View::Directory;
                    self.search.clear();
                    self.selected = Some(i);
                    ctx.data_mut(|data| data.insert_temp(egui::Id::new("locate_row"), i));
                }
            }
            Action::Copy(i) => {
                if let Some(r) = &self.result {
                    ctx.copy_text(r.path(i).display().to_string());
                }
            }
            Action::Open(i) => {
                if let Some(r) = &self.result {
                    #[cfg(windows)]
                    {
                        let node = &r.nodes[i];
                        let path = r.path(i);
                        let mut command = std::process::Command::new("explorer.exe");
                        if node.directory {
                            command.arg(&path);
                        } else {
                            let mut arg = std::ffi::OsString::from("/select,");
                            arg.push(&path);
                            command.arg(arg);
                        }
                        if let Err(e) = command.spawn() {
                            self.message = Some(format!("无法打开资源管理器：{e}"));
                        }
                    }
                    #[cfg(not(windows))]
                    {
                        let _ = (r, i);
                        self.message = Some("此操作需要 Windows 资源管理器。".into());
                    }
                }
            }
        }
    }
    fn refresh_list(&mut self) {
        let Some(result) = &self.result else {
            return;
        };
        let current = *self.trail.last().unwrap();
        if self.map.current != Some(current) {
            self.map.top = largest_map_children(result, current, MAP_TOP_LIMIT);
            self.map.nested = self.map.top.iter().map(|&i| {
                if result.nodes[i].directory {
                    largest_map_children(result, i, MAP_NESTED_LIMIT)
                } else {
                    Vec::new()
                }
            }).collect();
            self.map.current = Some(current);
        }
        let key = (current, self.search.to_lowercase(), self.sort_name);
        if self.cached_key.as_ref() == Some(&key) {
            return;
        }
        if self.cached_key.as_ref().is_some_and(|(directory, _, _)| *directory == current) {
            self.cached_children.clear();
        } else {
            self.cached_children = Vec::new();
        }
        self.cached_children.extend(
            result.nodes[current]
                .children
                .clone()
                .filter(|&i| key.1.is_empty() || result.nodes[i].name().to_lowercase().contains(&key.1)),
        );
        if self.sort_name {
            self.cached_children.sort_by_cached_key(|&i| result.nodes[i].name().to_lowercase());
        } else {
            self.cached_children.sort_unstable_by(|&a, &b| {
                result.nodes[b]
                    .size
                    .cmp(&result.nodes[a].size)
                    .then_with(|| result.nodes[a].name().cmp(&result.nodes[b].name()))
            });
        }
        self.cached_key = Some(key);
    }
}

fn largest_map_children(result: &ScanResult, parent: usize, limit: usize) -> Vec<usize> {
    use std::{cmp::Reverse, collections::BinaryHeap};
    let mut largest = BinaryHeap::with_capacity(limit);
    for i in result.nodes[parent].children.clone() {
        let bytes = result.nodes[i].size;
        if bytes == 0 { continue; }
        let item = Reverse((bytes, i));
        if largest.len() < limit {
            largest.push(item);
        } else if largest.peek().is_some_and(|smallest| item < *smallest) {
            *largest.peek_mut().unwrap() = item;
        }
    }
    largest.into_sorted_vec().into_iter().map(|Reverse((_, i))| i).collect()
}

fn map_color(bytes: u64, palette: Palette) -> Color32 {
    // Logarithmic size bands, from small green blocks to large red blocks.
    const EDGES: [u64; 7] = [4_000_000, 14_000_000, 52_000_000, 193_000_000,
        720_000_000, 2_700_000_000, 10_000_000_000];
    palette.chart[EDGES.partition_point(|&edge| bytes >= edge)]
}

// Keep the unshown bytes as a real, proportionally sized aggregate.
fn map_items(result: &ScanResult, parent: usize, top: &[usize], items: &mut Vec<MapItem>) {
    items.clear();
    let mut shown = 0u64;
    for (rank, &i) in top.iter().enumerate() {
        let bytes = result.nodes[i].size;
        shown = shown.saturating_add(bytes);
        items.push((Some(i), bytes, rank));
    }
    let other = result.nodes[parent].size.saturating_sub(shown);
    if other > 0 { items.push((None, other, usize::MAX)); }
    items.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
}

fn map_tile(
    ui: &mut egui::Ui,
    result: &ScanResult,
    current: usize,
    parent: usize,
    selected: Option<usize>,
    index: Option<usize>,
    bytes: u64,
    tile: Rect,
    color: Color32,
    expanded: bool,
    palette: Palette,
) -> Option<Action> {
    let response = ui.interact(tile, ui.id().with(("map_tile", parent, index)),
        if index.is_some() { Sense::click() } else { Sense::hover() });
    let fill = if response.hovered() {
        color.gamma_multiply(if ui.visuals().dark_mode { 1.12 } else { 0.94 })
    } else { color };
    let painted = tile.shrink(1.0);
    ui.painter().rect_filled(painted, 3, fill);
    if index.is_some() && index == selected {
        ui.painter().rect_stroke(painted, 3, Stroke::new(2.0_f32, palette.accent), egui::StrokeKind::Inside);
    }
    let name = index.map(|i| result.nodes[i].name()).unwrap_or_else(|| "其他项目".into());
    if tile.width() > 64.0 && tile.height() > 27.0 {
        let painter = ui.painter().with_clip_rect(painted.shrink(3.0));
        let name_width = if expanded && tile.width() > 190.0 { tile.width() - 100.0 } else { tile.width() - 16.0 };
        painter.text(tile.left_top() + Vec2::new(8.0, 6.0), Align2::LEFT_TOP,
            fit_text(ui, &name, 13.0, name_width), FontId::proportional(13.0), palette.chart_text);
        if expanded && tile.width() > 190.0 {
            painter.text(tile.right_top() + Vec2::new(-8.0, 6.0), Align2::RIGHT_TOP,
                size(bytes), FontId::proportional(12.0), palette.chart_text);
        } else if !expanded && tile.height() > 47.0 {
            painter.text(tile.left_bottom() + Vec2::new(8.0, -7.0), Align2::LEFT_BOTTOM,
                fit_text(ui, &size(bytes), 12.0, tile.width() - 16.0), FontId::proportional(12.0), palette.chart_text);
        }
    }
    let mut action = None;
    if let Some(i) = index {
        let node = &result.nodes[i];
        if response.clicked() {
            action = Some(if node.directory { Action::Enter(i) } else { Action::Select(i) });
        }
        response.context_menu(|ui| {
            if node.directory && ui.button("进入文件夹").clicked() {
                action = Some(Action::Enter(i));
                ui.close_menu();
            }
            if ui.button("定位到所在目录").clicked() { action = Some(Action::Locate(i)); ui.close_menu(); }
            if ui.button("复制路径").clicked() { action = Some(Action::Copy(i)); ui.close_menu(); }
            if ui.button("在资源管理器中查看").clicked() { action = Some(Action::Open(i)); ui.close_menu(); }
        });
        response.on_hover_ui(|ui| {
            ui.label(result.path(i).display().to_string());
            ui.label(format!("{} · 当前目录的 {:.1}%", size(bytes), percent(bytes, result.nodes[current].size)));
            ui.label(if node.directory { "点击进入文件夹 · 右键更多操作" } else { "点击选中 · 右键更多操作" });
        });
    } else {
        response.on_hover_ui(|ui| {
            ui.label(format!("其他项目 · {}", size(bytes)));
            ui.label("请进入对应目录，在目录列表中查看");
        });
    }
    action
}

// The map describes the full current directory, independently of list filtering.
fn distribution(
    ui: &mut egui::Ui,
    result: &ScanResult,
    current: usize,
    selected: Option<usize>,
    map: &mut MapCache,
    palette: Palette,
) -> Option<Action> {
    let total = result.nodes[current].size;
    let (status, status_response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    status_response.on_hover_text("当前目录最多显示 64 个项目；展开的文件夹最多显示 32 个子项目。其余有大小的项目合并，极小的块无法点选，零大小项目请在目录浏览中查看。");
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ui.available_height().max(1.0)), Sense::hover());
    ui.painter().rect_filled(rect, 4, palette.chart_bg);
    if total == 0 {
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, "此目录没有可统计的文件大小",
            FontId::proportional(13.0), palette.muted);
        ui.painter().text(status.left_center(), Align2::LEFT_CENTER, "零大小项目可在目录浏览中查看",
            FontId::proportional(11.0), palette.muted);
        return None;
    }
    map_items(result, current, &map.top, &mut map.items);
    map.tiles.clear();
    crate::treemap::squarify(&map.items, rect, &mut map.tiles);
    let mut action = None;
    let mut tiny = 0usize;
    for &(index, bytes, rank, tile) in &map.tiles {
        if tile.width() < 3.0 || tile.height() < 3.0 { tiny += 1; continue; }
        let expanded = index.is_some_and(|i| result.nodes[i].directory)
            && !map.nested[rank].is_empty() && tile.width() >= 145.0 && tile.height() >= 100.0;
        let color = if index.is_some() { map_color(bytes, palette) } else { palette.chart_other };
        if let Some(a) = map_tile(ui, result, current, current, selected, index, bytes, tile, color, expanded, palette) {
            action = Some(a);
        }
        if expanded {
            let parent = index.unwrap();
            let inner = Rect::from_min_max(tile.min + Vec2::new(4.0, 27.0), tile.max - Vec2::new(4.0, 4.0));
            map_items(result, parent, &map.nested[rank], &mut map.items);
            map.nested_tiles.clear();
            crate::treemap::squarify(&map.items, inner, &mut map.nested_tiles);
            for &(child, child_bytes, _, child_tile) in &map.nested_tiles {
                if child_tile.width() < 3.0 || child_tile.height() < 3.0 { tiny += 1; continue; }
                let child_color = if child.is_some() { map_color(child_bytes, palette) } else { palette.chart_other };
                if let Some(a) = map_tile(ui, result, current, parent, selected, child, child_bytes,
                    child_tile, child_color, false, palette) {
                    action = Some(a);
                }
            }
        }
    }
    let note = if tiny > 0 {
        format!("{tiny} 个极小块不可点 · 其他合并 · 零大小见列表")
    } else {
        "小文件夹整体显示 · 其他合并 · 零大小见列表".to_owned()
    };
    ui.painter().text(status.left_center(), Align2::LEFT_CENTER,
        fit_text(ui, &note, 11.0, status.width()), FontId::proportional(11.0), palette.muted);
    action
}

// The list owns the remaining viewport; it is never nested in another vertical scroller.
const ITEM_ROW_HEIGHT: f32 = 30.0;
fn item_list(
    ui: &mut egui::Ui,
    result: &ScanResult,
    current: usize,
    children: &[usize],
    selected: Option<usize>,
    palette: Palette,
) -> (Option<Action>, usize) {
    let mut action = None;
    let mut painted = 0;
    let total = result.nodes[current].size;
    let height = ui.available_height().max(1.0);
    let locate = ui.ctx().data_mut(|data| data.remove_temp::<usize>(egui::Id::new("locate_row")));
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt(("items", current))
        .auto_shrink([false, false])
        .max_height(height);
    if let Some(row) = locate.and_then(|i| children.iter().position(|&child| child == i)) {
        scroll = scroll.vertical_scroll_offset(row as f32 * (ITEM_ROW_HEIGHT + ui.spacing().item_spacing.y));
    }
    scroll
        .show_rows(ui, ITEM_ROW_HEIGHT, children.len(), |ui, rows| {
            for row in rows {
                let i = children[row];
                let node = &result.nodes[i];
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), ITEM_ROW_HEIGHT), Sense::click());
                painted += 1;
                let bg = if selected == Some(i) {
                    palette.selected
                } else if response.hovered() {
                    palette.hover
                } else {
                    palette.surface
                };
                ui.painter().rect_filled(rect, 0, bg);
                let painter = ui.painter().with_clip_rect(rect);
                let y = rect.center().y;
                // Simple drawn icons avoid missing glyphs on Windows installations.
                let icon = Rect::from_min_size(
                    Pos2::new(rect.left() + 9.0, y - 7.0),
                    Vec2::new(16.0, 13.0),
                );
                if node.directory {
                    painter.rect_filled(icon, 2, palette.folder);
                    painter.rect_filled(
                        Rect::from_min_size(icon.min + Vec2::new(1.0, -3.0), Vec2::new(7.0, 4.0)),
                        1,
                        palette.folder,
                    );
                } else {
                    painter.rect_stroke(
                        icon.shrink2(Vec2::new(2.0, 0.0)),
                        1,
                        Stroke::new(1.2_f32, palette.muted),
                        egui::StrokeKind::Inside,
                    );
                }
                let wide = rect.width() >= 500.0;
                let show_percent = rect.width() >= 350.0;
                let right_columns = if wide { 285.0 } else if show_percent { 160.0 } else { 100.0 };
                let name_right = (rect.right() - right_columns).max(rect.left() + 36.0);
                let name_painter = painter.with_clip_rect(Rect::from_min_max(
                    Pos2::new(rect.left() + 34.0, rect.top()),
                    Pos2::new(name_right, rect.bottom()),
                ));
                name_painter.text(
                    Pos2::new(rect.left() + 34.0, y),
                    Align2::LEFT_CENTER,
                    fit_text(ui, &node.name(), 14.0, name_right - rect.left() - 39.0),
                    FontId::proportional(14.0),
                    palette.text,
                );
                if wide {
                let bar_left = rect.right() - 270.0;
                let bar = Rect::from_min_size(Pos2::new(bar_left, y - 2.0), Vec2::new(105.0, 4.0));
                painter.rect_filled(bar, 2, palette.line);
                painter.rect_filled(
                    Rect::from_min_size(
                        bar.min,
                        Vec2::new(bar.width() * percent(node.size, total) / 100.0, 4.0),
                    ),
                    2,
                    palette.accent,
                );
                painter.text(
                    Pos2::new(rect.right() - 95.0, y),
                    Align2::RIGHT_CENTER,
                    size(node.size),
                    FontId::proportional(13.0),
                    palette.text,
                );
                painter.text(
                    Pos2::new(rect.right() - 19.0, y),
                    Align2::RIGHT_CENTER,
                    format!("{:.1}%", percent(node.size, total)),
                    FontId::proportional(13.0),
                    palette.muted,
                );
                } else if show_percent {
                    painter.text(Pos2::new(rect.right() - 75.0, y), Align2::RIGHT_CENTER,
                        size(node.size), FontId::proportional(13.0), palette.text);
                    painter.text(Pos2::new(rect.right() - 10.0, y), Align2::RIGHT_CENTER,
                        format!("{:.1}%", percent(node.size, total)), FontId::proportional(13.0), palette.muted);
                } else {
                    painter.text(Pos2::new(rect.right() - 10.0, y), Align2::RIGHT_CENTER,
                        size(node.size), FontId::proportional(13.0), palette.text);
                }
                if response.double_clicked() {
                    action = Some(Action::Enter(i));
                } else if response.clicked() {
                    action = Some(Action::Select(i));
                }
                response.context_menu(|ui| {
                    if node.directory && ui.button("进入文件夹").clicked() {
                        action = Some(Action::Enter(i));
                        ui.close_menu();
                    }
                    if ui.button("在资源管理器中查看").clicked() {
                        action = Some(Action::Open(i));
                        ui.close_menu();
                    }
                    if ui.button("复制路径").clicked() {
                        action = Some(Action::Copy(i));
                        ui.close_menu();
                    }
                });
                response.on_hover_ui(|ui| {
                    ui.label(format!(
                        "{}\n{}",
                        result.path(i).display(),
                        if node.directory {
                            "双击进入 · 右键更多操作"
                        } else {
                            "右键更多操作"
                        }
                    ));
                });
            }
        });
    (action, painted)
}

fn largest_list(ui: &mut egui::Ui, result: &ScanResult, selected: Option<usize>, palette: Palette) -> Option<Action> {
    let mut action = None;
    egui::ScrollArea::vertical().id_salt("largest_files").auto_shrink([false, false])
        .max_height(ui.available_height().max(1.0))
        .show_rows(ui, 40.0, result.largest_files.len(), |ui, rows| {
            for row in rows {
                let i = result.largest_files[row];
                let node = &result.nodes[i];
                let path = result.path(i);
                let parent = path.strip_prefix(&result.root).ok().and_then(|p| p.parent())
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(|p| p.display().to_string()).unwrap_or_else(|| "根目录".into());
                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
                let bg = if selected == Some(i) { palette.selected } else if response.hovered() { palette.hover } else { palette.surface };
                ui.painter().rect_filled(rect, 0, bg);
                let painter = ui.painter().with_clip_rect(rect);
                painter.text(Pos2::new(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER,
                    format!("{}", row + 1), FontId::proportional(12.0), palette.muted);
                let text_width = (rect.width() - 145.0).max(0.0);
                painter.text(Pos2::new(rect.left() + 36.0, rect.top() + 4.0), Align2::LEFT_TOP,
                    fit_text(ui, &node.name(), 14.0, text_width), FontId::proportional(14.0), palette.text);
                painter.text(Pos2::new(rect.left() + 36.0, rect.top() + 22.0), Align2::LEFT_TOP,
                    fit_text(ui, &parent, 11.0, text_width), FontId::proportional(11.0), palette.muted);
                painter.text(Pos2::new(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER,
                    size(node.size), FontId::proportional(13.0), palette.text);
                if response.double_clicked() { action = Some(Action::Locate(i)); }
                else if response.clicked() { action = Some(Action::Select(i)); }
                response.context_menu(|ui| {
                    if ui.button("定位到所在目录").clicked() { action = Some(Action::Locate(i)); ui.close_menu(); }
                    if ui.button("复制路径").clicked() { action = Some(Action::Copy(i)); ui.close_menu(); }
                    if ui.button("在资源管理器中查看").clicked() { action = Some(Action::Open(i)); ui.close_menu(); }
                });
                response.on_hover_text(format!("{}\n双击定位到所在目录", path.display()));
            }
        });
    action
}

impl App {
    fn draw(&mut self, ctx: &egui::Context) {
        self.events(ctx);
        let palette = self.theme.palette();
        let mut toggle_theme = false;
        let mut action = None;
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::new().fill(palette.surface).inner_margin(egui::Margin::symmetric(12, 8)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("空间视图").size(17.0).strong().color(palette.text));
                    ui.add_space(10.0);
                    if primary(ui, "选择文件夹", palette).clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.start(path, ctx.clone());
                        }
                    }
                    if self.last_path.is_some() && self.cancel.is_none()
                        && button(ui, "重新扫描").clicked() {
                        self.start(self.last_path.clone().unwrap(), ctx.clone());
                    }
                    if self.cancel.is_some() {
                        let stopping = self.stopping();
                        if !stopping && button(ui, "取消扫描").clicked() {
                            self.stop();
                        } else if stopping && self.pending.is_some() && button(ui, "取消新扫描").clicked() {
                            self.stop();
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if button(ui, match self.theme { Theme::Light => "深色模式", Theme::Dark => "浅色模式" })
                            .on_hover_text("切换界面主题（仅本次运行）").clicked() {
                            toggle_theme = true;
                        }
                    });
                });
            });
        if !ctx.wants_keyboard_input()
            && ctx.input(|i| {
                i.key_pressed(egui::Key::Backspace)
                    || (i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft))
            })
        {
            if self.view == View::LargestFiles { self.view = View::Directory; } else { action = Some(Action::Back); }
        }
        self.refresh_list();
        if self.cancel.is_none() {
            if let Some(result) = &self.result {
                let current = *self.trail.last().unwrap();
                egui::SidePanel::right("overview")
                    .default_width(270.0).min_width(220.0).max_width(330.0).resizable(true)
                    .frame(egui::Frame::new().fill(palette.bg).inner_margin(egui::Margin::symmetric(12, 10)))
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().id_salt("overview_content").auto_shrink([false, false]).show(ui, |ui| {
                            ui.label(RichText::new("扫描概览").size(14.0).strong().color(palette.text));
                            let root_path = result.root.display().to_string();
                            ui.label(RichText::new(fit_text(ui, &root_path, 12.0, ui.available_width())).size(12.0).color(palette.muted))
                                .on_hover_text(format!("扫描根目录\n{root_path}"));
                            ui.add_space(4.0);
                            ui.columns(2, |columns| {
                                txt(&mut columns[0], "逻辑大小", 12.0, palette.muted);
                                txt(&mut columns[0], size(result.nodes[0].size), 19.0, palette.text);
                                txt(&mut columns[1], "成功扫描文件", 12.0, palette.muted);
                                txt(&mut columns[1], result.nodes[0].files.to_string(), 19.0, palette.text);
                            });
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(format!("跳过链接/特殊目录 {}", result.skipped_links)).size(12.0).color(palette.muted))
                                    .on_hover_text("链接和特殊目录不会计入大小与文件数");
                                if ui.link(RichText::new(format!("读取错误 {}", result.skipped_errors)).size(12.0)
                                    .color(if result.skipped_errors > 0 { palette.warning } else { palette.muted })).clicked() {
                                    self.show_errors = true;
                                }
                            });
                            if result.skipped_links > 0 || result.skipped_errors > 0 {
                                txt(ui, "扫描结果不完整，跳过项未计入", 12.0, palette.warning);
                            } else {
                                txt(ui, "逻辑大小，不代表磁盘实际占用", 12.0, palette.muted);
                            }
                            if let Some(selected) = self.selected.filter(|i| self.view != View::LargestFiles || result.largest_files.contains(i)) {
                                ui.separator();
                                txt(ui, "已选择", 12.0, palette.muted);
                                let node = &result.nodes[selected];
                                ui.label(RichText::new(fit_text(ui, &node.name(), 14.0, ui.available_width())).size(14.0).strong())
                                    .on_hover_text(result.path(selected).display().to_string());
                                txt(ui, size(node.size), 13.0, palette.muted);
                                ui.horizontal_wrapped(|ui| {
                                    if (self.view == View::LargestFiles || node.parent != current) && button(ui, "定位目录").on_hover_text("定位到所在目录").clicked() {
                                        action = Some(Action::Locate(selected));
                                    }
                                    if button(ui, "复制路径").clicked() { action = Some(Action::Copy(selected)); }
                                    if button(ui, "资源管理器").on_hover_text("在资源管理器中查看").clicked() { action = Some(Action::Open(selected)); }
                                });
                            }
                            ui.separator();
                            txt(ui, "当前目录", 12.0, palette.muted);
                            txt(ui, format!("{} · {} 个文件", size(result.nodes[current].size), result.nodes[current].files), 13.0, palette.text);
                        });
                    });
            }
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.surface).inner_margin(egui::Margin::symmetric(12, 10)))
            .show(ctx, |ui| {
                if self.cancel.is_some() {
                    ui.add_space(55.0);
                    let stopping = self.stopping();
                    txt(ui, if stopping { "正在停止扫描" } else { "正在分析文件夹" }, 22.0, palette.text);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        txt(ui, format!("已发现 {} 个项目", self.scanned), 14.0, palette.muted);
                    });
                    if stopping {
                        let detail = if self.pending.is_some() {
                            "正在等待当前扫描退出，随后开始新扫描。"
                        } else {
                            "正在等待当前文件系统操作和扫描线程退出。"
                        };
                        txt(ui, detail, 13.0, palette.muted);
                    }
                    if let Some(path) = &self.last_path {
                        txt(ui, path.display().to_string(), 13.0, palette.muted);
                    }
                    return;
                }
                if let Some(message) = &self.message {
                    txt(ui, message.clone(), 13.0, palette.warning);
                }
                let Some(result) = &self.result else {
                    ui.add_space(55.0);
                    txt(ui, "选择文件夹开始分析", 22.0, palette.text);
                    ui.add_space(8.0);
                    txt(ui, "使用上方按钮选择文件夹，查看目录大小、空间分布和大文件。", 14.0, palette.muted);
                    return;
                };
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.view == View::Treemap, "空间地图").clicked() {
                        self.view = View::Treemap;
                    }
                    if ui.selectable_label(self.view == View::Directory, "目录浏览").clicked() {
                        self.view = View::Directory;
                    }
                    if ui.selectable_label(self.view == View::LargestFiles, "全局大文件").clicked() {
                        self.view = View::LargestFiles;
                    }
                });
                ui.separator();
                if self.view == View::LargestFiles {
                    txt(ui, format!("全扫描范围 · 按大小降序 · 显示 {} / {} 个文件{}",
                        result.largest_files.len(), result.nodes[0].files,
                        if result.nodes[0].files > result.largest_files.len() as u64 { "（前 100 个）" } else { "" }), 12.0, palette.muted);
                    ui.add_space(4.0);
                    let (header, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
                    ui.painter().text(Pos2::new(header.left() + 36.0, header.center().y), Align2::LEFT_CENTER,
                        "文件名 / 所在目录", FontId::proportional(12.0), palette.muted);
                    ui.painter().text(Pos2::new(header.right() - 10.0, header.center().y), Align2::RIGHT_CENTER,
                        "大小", FontId::proportional(12.0), palette.muted);
                    if result.largest_files.is_empty() {
                        ui.add_space(20.0);
                        txt(ui, "没有扫描到文件", 14.0, palette.muted);
                    } else if let Some(a) = largest_list(ui, result, self.selected, palette) {
                        action = Some(a);
                    }
                    return;
                }
                let current = *self.trail.last().unwrap();
                let node = &result.nodes[current];
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.trail.len() > 1, egui::Button::new("← 上一级"))
                        .on_hover_text("返回上一级 (Backspace / Alt+←)").clicked() {
                        action = Some(Action::Back);
                    }
                    egui::ScrollArea::horizontal().id_salt("breadcrumbs").auto_shrink([false, true]).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (level, &i) in self.trail.iter().enumerate() {
                                if level > 0 { txt(ui, "/", 13.0, palette.muted); }
                                let name = if level == 0 {
                                    result.root.file_name().map(|name| name.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| result.root.display().to_string())
                                } else {
                                    result.nodes[i].name().into_owned()
                                };
                                if ui.link(RichText::new(name).size(13.0).color(if i == current { palette.text } else { palette.muted }))
                                    .on_hover_text(result.path(i).display().to_string()).clicked() {
                                    action = Some(Action::GoTo(level));
                                }
                            }
                        });
                    });
                });
                if self.view == View::Treemap {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("空间分布").size(15.0).strong().color(palette.text));
                        txt(ui, format!("{}  ·  {} 个文件", size(node.size), node.files), 12.0, palette.muted);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            txt(ui, "大", 11.0, palette.muted);
                            for color in palette.chart.into_iter().rev() {
                                let (swatch, response) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), Sense::hover());
                                ui.painter().rect_filled(swatch, 2, color);
                                response.on_hover_text("按项目逻辑大小着色：绿色较小，红色较大；灰色为合并项");
                            }
                            txt(ui, "小", 11.0, palette.muted);
                        });
                    });
                    txt(ui, "面积对应逻辑大小 · 单击文件夹进入 · 单击文件选择 · 右键更多操作", 12.0, palette.muted);
                    if let Some(a) = distribution(ui, result, current, self.selected, &mut self.map, palette) {
                        action = Some(a);
                    }
                    return;
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new("目录内容").size(14.0).strong().color(palette.text));
                    txt(ui, format!("{} / {} 项", self.cached_children.len(), node.children.len()), 12.0, palette.muted);
                    ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("搜索当前目录")
                        .desired_width((ui.available_width() - 85.0).clamp(55.0, 160.0)));
                    if button(ui, if self.sort_name { "名称 ↑" } else { "大小 ↓" }).on_hover_text("切换排序方式").clicked() {
                        self.sort_name = !self.sort_name;
                    }
                });
                let (header, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
                let painter = ui.painter();
                let font = FontId::proportional(12.0);
                painter.text(Pos2::new(header.left() + 34.0, header.center().y), Align2::LEFT_CENTER, "名称", font.clone(), palette.muted);
                if header.width() >= 500.0 {
                    painter.text(Pos2::new(header.right() - 270.0, header.center().y), Align2::LEFT_CENTER, "占用", font.clone(), palette.muted);
                    painter.text(Pos2::new(header.right() - 95.0, header.center().y), Align2::RIGHT_CENTER, "大小", font.clone(), palette.muted);
                    painter.text(Pos2::new(header.right() - 19.0, header.center().y), Align2::RIGHT_CENTER, "占比", font, palette.muted);
                } else if header.width() >= 350.0 {
                    painter.text(Pos2::new(header.right() - 75.0, header.center().y), Align2::RIGHT_CENTER, "大小", font.clone(), palette.muted);
                    painter.text(Pos2::new(header.right() - 10.0, header.center().y), Align2::RIGHT_CENTER, "占比", font, palette.muted);
                } else {
                    painter.text(Pos2::new(header.right() - 10.0, header.center().y), Align2::RIGHT_CENTER, "大小", font, palette.muted);
                }
                if self.cached_children.is_empty() {
                    ui.add_space(20.0);
                    txt(ui, if self.search.is_empty() { "此文件夹没有可显示的项目" } else { "没有匹配的项目，试试其他关键词" }, 14.0, palette.muted);
                } else {
                    let (a, _) = item_list(ui, result, current, &self.cached_children, self.selected, palette);
                    if a.is_some() { action = a; }
                }
            });
        if self.show_errors {
            if let Some(result) = &self.result {
                egui::Window::new("读取错误详情").open(&mut self.show_errors)
                    .default_width(600.0).resizable(true).show(ctx, |ui| {
                        txt(ui, format!("共 {} 个读取错误，显示 {} 条", result.skipped_errors, result.error_details.len()), 14.0, palette.text);
                        if result.skipped_errors > result.error_details.len() as u64 {
                            txt(ui, "详情已截断，仅保留前 50 条。", 13.0, palette.warning);
                        }
                        if result.error_details.is_empty() {
                            txt(ui, "没有读取错误。", 13.0, palette.muted);
                        }
                        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                            for detail in &result.error_details {
                                ui.label(detail);
                                ui.separator();
                            }
                        });
                    });
            }
        }
        if toggle_theme {
            self.theme.toggle(ctx);
            ctx.request_repaint();
        }
        if let Some(a) = action {
            self.apply_action(a, ctx);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Node;
    use std::path::PathBuf;

    fn node(name: &str, parent: usize, directory: bool, size: u64) -> Node {
        Node::new(std::ffi::OsStr::new(name).into(), parent, directory, size)
    }

    #[test]
    fn theme_toggle_updates_rendering_without_changing_navigation() {
        fn label_position(shape: &egui::Shape, text: &str) -> Option<Pos2> {
            match shape {
                egui::Shape::Text(label) if label.galley.job.text == text => Some(label.pos),
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| label_position(shape, text)),
                _ => None,
            }
        }
        fn has_fill(shape: &egui::Shape, color: Color32) -> bool {
            match shape {
                egui::Shape::Rect(rect) => rect.fill == color,
                egui::Shape::Vec(shapes) => shapes.iter().any(|shape| has_fill(shape, color)),
                _ => false,
            }
        }
        fn frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) -> egui::FullOutput {
            ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1050.0, 740.0))),
                events,
                ..Default::default()
            }, |ctx| app.draw(ctx))
        }
        fn click(ctx: &egui::Context, app: &mut App, output: &egui::FullOutput, label: &str) -> egui::FullOutput {
            let pos = output.shapes.iter().find_map(|s| label_position(&s.shape, label)).unwrap()
                + Vec2::new(4.0, 4.0);
            let _ = frame(ctx, app, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE,
            }]);
            frame(ctx, app, vec![egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE,
            }])
        }

        let mut root = node("", 0, true, 0);
        root.size = 10;
        root.children = 1..2;
        let mut app = App::default();
        app.result = Some(ScanResult {
            root: PathBuf::from("root"), nodes: vec![root, node("file", 0, false, 10)],
            skipped_links: 0, skipped_errors: 0, error_details: vec![], largest_files: vec![1],
        });
        app.selected = Some(1);
        app.search = "file".into();
        let ctx = egui::Context::default();
        configure(&ctx);
        assert_eq!(app.theme, Theme::Light);
        assert!(!ctx.style().visuals.dark_mode);
        let light = frame(&ctx, &mut app, vec![]);
        for color in [Theme::Light.palette().bg, Theme::Light.palette().surface, Theme::Light.palette().chart[0]] {
            assert!(light.shapes.iter().any(|s| has_fill(&s.shape, color)));
        }
        click(&ctx, &mut app, &light, "深色模式");
        assert_eq!(app.theme, Theme::Dark);
        assert!(ctx.style().visuals.dark_mode);
        let dark = frame(&ctx, &mut app, vec![]);
        for color in [Theme::Dark.palette().bg, Theme::Dark.palette().surface, Theme::Dark.palette().chart[0]] {
            assert!(dark.shapes.iter().any(|s| has_fill(&s.shape, color)));
        }
        assert_eq!(app.trail, vec![0]);
        assert_eq!(app.selected, Some(1));
        assert_eq!(app.search, "file");
        assert_eq!(app.cached_children, vec![1]);
        click(&ctx, &mut app, &dark, "浅色模式");
        assert_eq!(app.theme, Theme::Light);
        assert!(!ctx.style().visuals.dark_mode);
        let directory = frame(&ctx, &mut app, vec![]);
        click(&ctx, &mut app, &directory, "全局大文件");
        assert_eq!(app.view, View::LargestFiles);
        let ranking = frame(&ctx, &mut app, vec![]);
        let copied = click(&ctx, &mut app, &ranking, "复制路径");
        let expected = PathBuf::from("root").join("file").display().to_string();
        assert!(copied.platform_output.commands.iter().any(|command|
            matches!(command, egui::OutputCommand::CopyText(path) if path == &expected)));
        let ranking = frame(&ctx, &mut app, vec![]);
        click(&ctx, &mut app, &ranking, "目录浏览");
        assert_eq!(app.view, View::Directory);
        assert_eq!(app.selected, Some(1));
        assert_eq!(app.search, "file");

        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        app.cancel = Some(cancel.clone());
        app.scanned = 42;
        let scanning = frame(&ctx, &mut app, vec![]);
        click(&ctx, &mut app, &scanning, "深色模式");
        assert_eq!(app.theme, Theme::Dark);
        assert_eq!(app.scanned, 42);
        assert!(!cancel.load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(app.selected, Some(1));
    }

    #[test]
    fn theme_text_and_chart_have_readable_contrast() {
        fn luminance(color: Color32) -> f32 {
            let channel = |value: u8| {
                let value = f32::from(value) / 255.0;
                if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
        }
        fn contrast(a: Color32, b: Color32) -> f32 {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        }
        for theme in [Theme::Light, Theme::Dark] {
            let p = theme.palette();
            for background in [p.bg, p.surface, p.widget, p.hover, p.selected] {
                assert!(contrast(p.text, background) >= 4.5);
                assert!(contrast(p.muted, background) >= 4.5);
            }
            assert!(contrast(p.accent_text, p.accent) >= 4.5);
            assert!(contrast(p.warning, p.bg) >= 4.5);
            for color in p.chart.into_iter().chain([p.chart_other]) {
                assert!(contrast(p.chart_text, color) >= 4.5);
                let hovered = color.gamma_multiply(if theme == Theme::Dark { 1.12 } else { 0.94 });
                assert!(contrast(p.chart_text, hovered) >= 4.5);
            }
        }
    }
    #[test]
    fn list_uses_the_available_viewport() {
        let mut nodes = vec![node("", 0, true, 100)];
        for i in 0..40 {
            nodes.push(node(&format!("item{i}"), 0, false, 1));
        }
        nodes[0].children = 1..nodes.len();
        let result = ScanResult {
            root: PathBuf::from("root"),
            nodes,
            skipped_links: 0,
            skipped_errors: 0,
            error_details: vec![], largest_files: vec![],
        };
        let ctx = egui::Context::default();
        let children: Vec<_> = result.nodes[0].children.clone().collect();
        let mut painted = 0;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (_, count) = item_list(ui, &result, 0, &children, None, Theme::Light.palette());
                    painted = count;
                });
            },
        );
        assert!(painted >= 8, "only {painted} rows were visible");
        assert!(painted < 40, "list should only paint visible rows");
    }

    #[test]
    fn navigation_search_and_chart_stay_consistent() {
        let mut root = node("", 0, true, 0);
        root.size = 30;
        root.children = 1..3;
        let mut folder = node("folder", 0, true, 0);
        folder.size = 20;
        let file = node("file", 0, false, 10);
        let mut app = App::default();
        app.result = Some(ScanResult {
            root: PathBuf::from("root"),
            nodes: vec![root, folder, file],
            skipped_links: 0,
            skipped_errors: 0,
            error_details: vec![], largest_files: vec![],
        });
        let ctx = egui::Context::default();
        app.refresh_list();
        assert_eq!(app.cached_children, vec![1, 2]);
        app.search = "file".into();
        app.refresh_list();
        assert_eq!(app.cached_children, vec![2]);
        assert_eq!(app.map.top, vec![1, 2]);
        app.apply_action(Action::Enter(1), &ctx);
        app.refresh_list();
        assert_eq!(app.trail, vec![0, 1]);
        assert!(app.search.is_empty());
        assert!(app.cached_children.is_empty());
        app.apply_action(Action::Back, &ctx);
        app.refresh_list();
        assert_eq!(app.trail, vec![0]);
        assert_eq!(app.cached_children, vec![1, 2]);
    }

    #[test]
    fn map_keeps_remainder_and_zero_size_items_available_in_list() {
        let mut root = node("", 0, true, 0);
        let mut nodes = vec![node("", 0, true, 0)];
        for i in 0..100 {
            nodes.push(node(&format!("item-{i}"), 0, false, i));
        }
        root.size = (0..100).sum();
        root.children = 1..nodes.len();
        nodes[0] = root;
        let mut app = App::default();
        app.result = Some(ScanResult {
            root: PathBuf::from("root"), nodes, skipped_links: 0, skipped_errors: 0,
            error_details: vec![], largest_files: vec![],
        });
        app.refresh_list();
        let result = app.result.as_ref().unwrap();
        assert_eq!(app.cached_children.len(), 100);
        let mut items = Vec::new();
        map_items(result, 0, &app.map.top, &mut items);
        let remainder = items.iter().find(|item| item.0.is_none()).unwrap().1;
        assert_eq!(remainder, (0..36).sum());
        assert_eq!(items.iter().map(|item| item.1).sum::<u64>(), result.nodes[0].size);
        let mut tiles = Vec::new();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 100.0));
        crate::treemap::squarify(&items, rect, &mut tiles);
        let other = tiles.iter().find(|tile| tile.0.is_none()).unwrap();
        assert!((other.3.area() / rect.area() - remainder as f32 / result.nodes[0].size as f32).abs() < 0.0001);
    }

    #[test]
    fn nested_map_clicks_preserve_directory_trail_at_both_window_sizes() {
        fn frame(ctx: &egui::Context, app: &mut App, screen: Vec2, events: Vec<egui::Event>) -> egui::FullOutput {
            ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)), events, ..Default::default()
            }, |ctx| app.draw(ctx))
        }
        fn click(ctx: &egui::Context, app: &mut App, screen: Vec2, pos: Pos2) {
            let _ = frame(ctx, app, screen, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE,
            }]);
            let _ = frame(ctx, app, screen, vec![egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE,
            }]);
        }
        fn text_position(shape: &egui::Shape, target: &str) -> Option<Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.job.text == target => Some(text.pos + text.galley.rect.center().to_vec2()),
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| text_position(shape, target)),
                _ => None,
            }
        }
        fn label(output: &egui::FullOutput, target: &str) -> Pos2 {
            output.shapes.iter().find_map(|shape| text_position(&shape.shape, target)).unwrap_or_else(|| panic!("missing label {target}"))
        }
        for screen in [Vec2::new(1050.0, 740.0), Vec2::new(700.0, 480.0)] {
            let mut root = node("", 0, true, 0);
            root.size = 100;
            root.children = 1..2;
            let mut parent = node("parent", 0, true, 0);
            parent.size = 100;
            parent.children = 2..4;
            let mut nested = node("nested", 1, true, 0);
            nested.size = 60;
            nested.children = 4..5;
            let mut app = App::default();
            app.result = Some(ScanResult {
                root: PathBuf::from("root"),
                nodes: vec![root, parent, nested, node("file", 1, false, 40), node("deep", 2, false, 60)],
                skipped_links: 0, skipped_errors: 0, error_details: vec![], largest_files: vec![],
            });
            let ctx = egui::Context::default();
            configure(&ctx);
            let _ = frame(&ctx, &mut app, screen, vec![]);
            assert!(app.map.tiles[0].3.height() > screen.y * 0.5, "map too small at {screen:?}");
            let file = app.map.nested_tiles.iter().find(|tile| tile.0 == Some(3)).unwrap().3.center();
            click(&ctx, &mut app, screen, file);
            assert_eq!(app.selected, Some(3));
            assert_eq!(app.trail, vec![0]);
            let output = frame(&ctx, &mut app, screen, vec![]);
            click(&ctx, &mut app, screen, label(&output, "定位目录"));
            assert_eq!(app.view, View::Directory);
            assert_eq!(app.trail, vec![0, 1]);
            assert_eq!(app.selected, Some(3));
            app.view = View::Treemap;
            app.apply_action(Action::GoTo(0), &ctx);
            let _ = frame(&ctx, &mut app, screen, vec![]);
            app.search = "not a map filter".into();
            let nested = app.map.nested_tiles.iter().find(|tile| tile.0 == Some(2)).unwrap().3.center();
            click(&ctx, &mut app, screen, nested);
            assert_eq!(app.trail, vec![0, 1, 2]);
            assert!(app.search.is_empty());
            let output = frame(&ctx, &mut app, screen, vec![]);
            click(&ctx, &mut app, screen, label(&output, "← 上一级"));
            assert_eq!(app.trail, vec![0, 1]);
            let _ = frame(&ctx, &mut app, screen, vec![]);
            let nested = app.map.tiles.iter().find(|tile| tile.0 == Some(2)).unwrap().3;
            click(&ctx, &mut app, screen, Pos2::new(nested.center().x, nested.top() + 12.0));
            assert_eq!(app.trail, vec![0, 1, 2]);
            let output = frame(&ctx, &mut app, screen, vec![]);
            click(&ctx, &mut app, screen, label(&output, "parent"));
            assert_eq!(app.trail, vec![0, 1]);
        }
    }

    #[test]
    fn map_reports_tiny_tiles_and_zero_size_directories() {
        fn contains(shape: &egui::Shape, value: &str) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.job.text.contains(value),
                egui::Shape::Vec(shapes) => shapes.iter().any(|shape| contains(shape, value)),
                _ => false,
            }
        }
        let mut root = node("", 0, true, 0);
        root.size = 1_000_000_001;
        root.children = 1..3;
        let mut app = App::default();
        app.result = Some(ScanResult {
            root: PathBuf::from("root"), nodes: vec![root, node("large", 0, false, 1_000_000_000), node("tiny", 0, false, 1)],
            skipped_links: 0, skipped_errors: 0, error_details: vec![], largest_files: vec![],
        });
        let ctx = egui::Context::default();
        configure(&ctx);
        let frame = |app: &mut App| ctx.run(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1050.0, 740.0))),
            ..Default::default()
        }, |ctx| app.draw(ctx));
        let output = frame(&mut app);
        assert!(output.shapes.iter().any(|shape| contains(&shape.shape, "1 个极小块不可点")));
        let mut empty = node("", 0, true, 0);
        empty.children = 1..2;
        app.result = Some(ScanResult {
            root: PathBuf::from("empty"), nodes: vec![empty, node("zero", 0, false, 0)],
            skipped_links: 0, skipped_errors: 0, error_details: vec![], largest_files: vec![],
        });
        app.map = MapCache::default();
        app.cached_key = None;
        let output = frame(&mut app);
        assert!(output.shapes.iter().any(|shape| contains(&shape.shape, "此目录没有可统计的文件大小")));
        assert_eq!(app.cached_children.len(), 1);
    }

    #[test]
    fn cancellation_remains_visible_until_worker_exits() {
        fn contains_text(shape: &egui::Shape, text: &str) -> bool {
            match shape {
                egui::Shape::Text(label) => label.galley.job.text == text,
                egui::Shape::Vec(shapes) => shapes.iter().any(|shape| contains_text(shape, text)),
                _ => false,
            }
        }
        let (release, blocked) = std::sync::mpsc::channel();
        let mut app = App::default();
        app.cancel = Some(Arc::new(std::sync::atomic::AtomicBool::new(false)));
        app.worker = Some(std::thread::spawn(move || {
            blocked.recv().unwrap();
            Ok(None)
        }));
        app.stop();
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| app.draw(ctx));
        assert!(output.shapes.iter().any(|s| contains_text(&s.shape, "正在停止扫描")));
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !app.worker.as_ref().unwrap().is_finished() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let output = ctx.run(egui::RawInput::default(), |ctx| app.draw(ctx));
        assert!(output.shapes.iter().any(|s| contains_text(&s.shape, "扫描已取消")));
    }

    #[test]
    fn global_ranking_locates_file_and_error_dialog_reports_truncation() {
        fn contains(shape: &egui::Shape, text: &str) -> bool {
            match shape {
                egui::Shape::Text(label) => label.galley.job.text.contains(text),
                egui::Shape::Vec(shapes) => shapes.iter().any(|shape| contains(shape, text)),
                _ => false,
            }
        }
        fn label_position(shape: &egui::Shape, text: &str) -> Option<Pos2> {
            match shape {
                egui::Shape::Text(label) if label.galley.job.text == text => Some(label.pos),
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| label_position(shape, text)),
                _ => None,
            }
        }
        let mut root = node("", 0, true, 0);
        root.children = 1..2;
        root.size = 42;
        root.files = 1;
        let mut folder = node("folder", 0, true, 0);
        folder.children = 2..3;
        folder.size = 42;
        folder.files = 1;
        let mut app = App::default();
        app.result = Some(ScanResult {
            root: PathBuf::from("root"), nodes: vec![root, folder, node("large.bin", 1, false, 42)],
            skipped_links: 1, skipped_errors: 51,
            error_details: vec!["root/denied: permission denied".into(); 50], largest_files: vec![2],
        });
        app.view = View::LargestFiles;
        app.show_errors = true;
        let ctx = egui::Context::default();
        let input = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1050.0, 740.0))),
            ..Default::default()
        };
        // egui uses the first frame to size a newly opened window.
        let _ = ctx.run(input(), |ctx| app.draw(ctx));
        let output = ctx.run(input(), |ctx| app.draw(ctx));
        for text in ["扫描结果不完整", "全扫描范围", "large.bin", "共 51 个读取错误", "详情已截断", "permission denied"] {
            assert!(output.shapes.iter().any(|shape| contains(&shape.shape, text)), "missing {text}");
        }
        app.show_errors = false;
        app.selected = Some(2);
        app.search = "unrelated".into();
        let output = ctx.run(input(), |ctx| app.draw(ctx));
        let pos = output.shapes.iter().find_map(|shape| label_position(&shape.shape, "定位目录"))
            .unwrap() + Vec2::new(4.0, 4.0);
        let mut press = input();
        press.events = vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
            pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE,
        }];
        let _ = ctx.run(press, |ctx| app.draw(ctx));
        let mut release = input();
        release.events = vec![egui::Event::PointerButton {
            pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE,
        }];
        let _ = ctx.run(release, |ctx| app.draw(ctx));
        app.refresh_list();
        assert_eq!(app.view, View::Directory);
        assert_eq!(app.trail, vec![0, 1]);
        assert_eq!(app.selected, Some(2));
        assert!(app.search.is_empty());
        assert_eq!(app.cached_children, vec![2]);
    }

    #[test]
    fn full_window_renders_multiple_rows() {
        fn count_rows(shape: &egui::Shape) -> usize {
            match shape {
                egui::Shape::Text(text) => {
                    usize::from(text.galley.job.text.starts_with("fixture-"))
                }
                egui::Shape::Vec(shapes) => shapes.iter().map(count_rows).sum(),
                _ => 0,
            }
        }
        fn contains_percentage(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.job.text == "2.5%",
                egui::Shape::Vec(shapes) => shapes.iter().any(contains_percentage),
                _ => false,
            }
        }
        fn contains_last_row(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.job.text == "fixture-39",
                egui::Shape::Vec(shapes) => shapes.iter().any(contains_last_row),
                _ => false,
            }
        }
        let mut root = node("", 0, true, 0);
        let mut nodes = vec![];
        for i in 0..40 {
            nodes.push(node(&format!("fixture-{i:02}"), 0, false, 1));
        }
        root.children = 1..nodes.len() + 1;
        root.size = 40;
        nodes.insert(0, root);
        let mut app = App::default();
        app.view = View::Directory;
        app.result = Some(ScanResult {
            root: PathBuf::from("root"),
            nodes,
            skipped_links: 0,
            skipped_errors: 0,
            error_details: vec![], largest_files: vec![],
        });
        for (screen, minimum) in [(Vec2::new(1050.0, 740.0), 12), (Vec2::new(700.0, 480.0), 6)] {
            let ctx = egui::Context::default();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)),
                    ..Default::default()
                },
                |ctx| app.draw(ctx),
            );
            let rows: usize = output.shapes.iter().map(|s| count_rows(&s.shape)).sum();
            assert!(rows >= minimum, "window {screen:?} rendered only {rows} fixture labels");
            assert!(output.shapes.iter().any(|shape| contains_percentage(&shape.shape)),
                "window {screen:?} did not display row percentages");
            app.apply_action(Action::Locate(40), &ctx);
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)),
                    ..Default::default()
                },
                |ctx| app.draw(ctx),
            );
            assert!(output.shapes.iter().any(|shape| contains_last_row(&shape.shape)),
                "window {screen:?} did not scroll to the located file");
        }
    }
}
