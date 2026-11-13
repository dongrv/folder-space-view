use crate::{percent, size, App, ScanResult};
use eframe::egui::{
    self, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, RichText,
    Sense, Stroke, Vec2,
};
use std::sync::Arc;

const BG: Color32 = Color32::from_rgb(16, 20, 28);
const SURFACE: Color32 = Color32::from_rgb(24, 30, 40);
const LINE: Color32 = Color32::from_rgb(43, 52, 65);
const TEXT: Color32 = Color32::from_rgb(233, 239, 244);
const MUTED: Color32 = Color32::from_rgb(145, 158, 174);
const TEAL: Color32 = Color32::from_rgb(99, 201, 183);
const COLORS: [Color32; 8] = [
    Color32::from_rgb(96, 194, 178),
    Color32::from_rgb(100, 156, 193),
    Color32::from_rgb(147, 158, 194),
    Color32::from_rgb(193, 166, 122),
    Color32::from_rgb(118, 174, 157),
    Color32::from_rgb(155, 140, 175),
    Color32::from_rgb(168, 150, 136),
    Color32::from_rgb(107, 145, 159),
];

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
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = SURFACE;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.widgets.noninteractive.bg_fill = SURFACE;
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(37, 46, 56);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(52, 65, 75);
    style.visuals.selection.bg_fill = Color32::from_rgb(45, 105, 98);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.spacing.item_spacing = Vec2::new(8.0, 7.0);
    ctx.set_style(style);
}
fn txt(ui: &mut egui::Ui, value: impl Into<String>, px: f32, color: Color32) {
    ui.label(RichText::new(value.into()).size(px).color(color));
}
fn fit_text(ui: &egui::Ui, value: &str, px: f32, width: f32) -> String {
    let fits = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), FontId::proportional(px), TEXT)
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
fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(text)
                .size(13.0)
                .strong()
                .color(Color32::from_rgb(15, 39, 36)),
        )
        .fill(TEAL)
        .corner_radius(5),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Select(usize),
    Enter(usize),
    Back,
    GoTo(usize),
    Open(usize),
    Copy(usize),
}

impl App {
    fn apply_action(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Select(i) => self.selected = Some(i),
            Action::Enter(i) => {
                if self.result.as_ref().is_some_and(|r| r.nodes[i].directory) {
                    self.trail.push(i);
                    self.selected = None;
                    self.search.clear();
                } else {
                    self.selected = Some(i);
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
            Action::Copy(i) => {
                if let Some(r) = &self.result {
                    ctx.copy_text(r.nodes[i].path.display().to_string());
                }
            }
            Action::Open(i) => {
                if let Some(r) = &self.result {
                    #[cfg(windows)]
                    {
                        let node = &r.nodes[i];
                        let mut command = std::process::Command::new("explorer.exe");
                        if node.directory {
                            command.arg(&node.path);
                        } else {
                            let mut arg = std::ffi::OsString::from("/select,");
                            arg.push(&node.path);
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
        if self.cached_chart_current != Some(current) {
            self.cached_chart = result.nodes[current].children.clone();
            self.cached_chart
                .sort_unstable_by_key(|&i| std::cmp::Reverse(result.nodes[i].size));
            self.cached_chart.truncate(32);
            self.cached_chart_current = Some(current);
        }
        let key = (current, self.search.to_lowercase(), self.sort_name);
        if self.cached_key.as_ref() == Some(&key) {
            return;
        }
        let mut children: Vec<_> = result.nodes[current]
            .children
            .iter()
            .copied()
            .filter(|&i| key.1.is_empty() || result.nodes[i].name().to_lowercase().contains(&key.1))
            .collect();
        if self.sort_name {
            children.sort_by_cached_key(|&i| result.nodes[i].name().to_lowercase());
        } else {
            children.sort_by(|&a, &b| {
                result.nodes[b]
                    .size
                    .cmp(&result.nodes[a].size)
                    .then_with(|| result.nodes[a].name().cmp(&result.nodes[b].name()))
            });
        }
        self.cached_key = Some(key);
        self.cached_children = children;
    }
}

// Split rectangles by their size totals, preserving each item's share of the area.
fn partition_map(
    items: &[(Option<usize>, u64, usize)],
    rect: Rect,
    out: &mut Vec<(Option<usize>, u64, usize, Rect)>,
) {
    if items.is_empty() {
        return;
    }
    if items.len() == 1 {
        out.push((items[0].0, items[0].1, items[0].2, rect));
        return;
    }
    let total: f64 = items.iter().map(|item| item.1 as f64).sum();
    let mut first = 0.0;
    let mut split = 0;
    for item in &items[..items.len() - 1] {
        first += item.1 as f64;
        split += 1;
        if first >= total / 2.0 {
            break;
        }
    }
    let fraction = (first / total) as f32;
    let (a, b) = if rect.width() >= rect.height() {
        let x = rect.left() + rect.width() * fraction;
        (
            Rect::from_min_max(rect.min, Pos2::new(x, rect.bottom())),
            Rect::from_min_max(Pos2::new(x, rect.top()), rect.max),
        )
    } else {
        let y = rect.top() + rect.height() * fraction;
        (
            Rect::from_min_max(rect.min, Pos2::new(rect.right(), y)),
            Rect::from_min_max(Pos2::new(rect.left(), y), rect.max),
        )
    };
    partition_map(&items[..split], a, out);
    partition_map(&items[split..], b, out);
}

// The map always describes the full directory, even when the list is filtered.
fn distribution(
    ui: &mut egui::Ui,
    result: &ScanResult,
    current: usize,
    top: &[usize],
) -> Option<Action> {
    let node = &result.nodes[current];
    let mut action = None;
    ui.horizontal(|ui| {
        txt(ui, "空间分布", 16.0, TEXT);
        txt(ui, "矩形面积代表大小", 12.0, MUTED);
    });
    ui.add_space(3.0);
    let height = (ui.available_height() * 0.28).clamp(100.0, 160.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    ui.painter()
        .rect_filled(rect, 5, Color32::from_rgb(34, 42, 53));
    if node.size == 0 {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "此目录没有可统计的文件大小",
            FontId::proportional(13.0),
            MUTED,
        );
        return None;
    }
    let mut items: Vec<_> = top
        .iter()
        .enumerate()
        .filter_map(|(rank, &i)| {
            let bytes = result.nodes[i].size;
            (bytes > 0).then_some((Some(i), bytes, rank))
        })
        .collect();
    let shown = items
        .iter()
        .fold(0u64, |sum, item| sum.saturating_add(item.1));
    let other = node.size.saturating_sub(shown);
    if other > 0 {
        items.push((None, other, COLORS.len()));
    }
    let mut tiles = Vec::with_capacity(items.len());
    partition_map(&items, rect, &mut tiles);
    for (index, bytes, rank, tile) in tiles {
        if tile.width() < 2.0 || tile.height() < 2.0 {
            continue;
        }
        let response = ui.interact(
            tile,
            ui.id().with(("tile", current, index)),
            if index.is_some() {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let color = if index.is_some() {
            COLORS[rank % COLORS.len()]
        } else {
            Color32::from_rgb(75, 87, 98)
        };
        ui.painter().rect_filled(
            tile.shrink(1.0),
            3,
            if response.hovered() {
                color.gamma_multiply(1.12)
            } else {
                color.gamma_multiply(0.72)
            },
        );
        let name = index
            .map(|i| result.nodes[i].name())
            .unwrap_or_else(|| "其他项目".into());
        if tile.width() > 70.0 && tile.height() > 30.0 {
            let painter = ui.painter().with_clip_rect(tile.shrink(5.0));
            painter.text(
                tile.left_top() + Vec2::new(9.0, 7.0),
                Align2::LEFT_TOP,
                fit_text(ui, &name, 13.0, tile.width() - 18.0),
                FontId::proportional(13.0),
                TEXT,
            );
            if tile.height() > 46.0 {
                let details = format!("{}  ·  {:.1}%", size(bytes), percent(bytes, node.size));
                let details = if ui
                    .painter()
                    .layout_no_wrap(details.clone(), FontId::proportional(12.0), TEXT)
                    .size()
                    .x
                    <= tile.width() - 18.0
                {
                    details
                } else {
                    size(bytes)
                };
                painter.text(
                    tile.left_bottom() + Vec2::new(9.0, -7.0),
                    Align2::LEFT_BOTTOM,
                    fit_text(ui, &details, 12.0, tile.width() - 18.0),
                    FontId::proportional(12.0),
                    TEXT,
                );
            }
        }
        if let Some(i) = index {
            let child = &result.nodes[i];
            if response.clicked() {
                action = Some(if child.directory {
                    Action::Enter(i)
                } else {
                    Action::Select(i)
                });
            }
            response.on_hover_text(format!(
                "{}\n{}  ·  {:.1}%{}",
                name,
                size(bytes),
                percent(bytes, node.size),
                if child.directory {
                    "\n点击进入"
                } else {
                    "\n点击选中"
                }
            ));
        } else {
            response.on_hover_text(format!(
                "其他项目\n{}  ·  {:.1}%\n请在下方列表查看",
                size(bytes),
                percent(bytes, node.size)
            ));
        }
    }
    ui.add_space(3.0);
    txt(ui, "显示较大的 32 项，其余合并 · 点击矩形查看", 11.0, MUTED);
    action
}

// The list owns the remaining viewport; it is never nested in another vertical scroller.
fn item_list(
    ui: &mut egui::Ui,
    result: &ScanResult,
    current: usize,
    children: &[usize],
    selected: Option<usize>,
) -> (Option<Action>, usize) {
    let mut action = None;
    let mut painted = 0;
    let total = result.nodes[current].size;
    let height = ui.available_height().max(1.0);
    egui::ScrollArea::vertical()
        .id_salt(("items", current))
        .auto_shrink([false, false])
        .max_height(height)
        .show_rows(ui, 40.0, children.len(), |ui, rows| {
            for row in rows {
                let i = children[row];
                let node = &result.nodes[i];
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
                painted += 1;
                let bg = if selected == Some(i) {
                    Color32::from_rgb(39, 67, 65)
                } else if response.hovered() {
                    Color32::from_rgb(32, 41, 51)
                } else {
                    SURFACE
                };
                ui.painter().rect_filled(rect, 4, bg);
                let painter = ui.painter().with_clip_rect(rect);
                let y = rect.center().y;
                // Simple drawn icons avoid missing glyphs on Windows installations.
                let icon = Rect::from_min_size(
                    Pos2::new(rect.left() + 15.0, y - 7.0),
                    Vec2::new(16.0, 13.0),
                );
                if node.directory {
                    painter.rect_filled(icon, 2, Color32::from_rgb(111, 167, 187));
                    painter.rect_filled(
                        Rect::from_min_size(icon.min + Vec2::new(1.0, -3.0), Vec2::new(7.0, 4.0)),
                        1,
                        Color32::from_rgb(111, 167, 187),
                    );
                } else {
                    painter.rect_stroke(
                        icon.shrink2(Vec2::new(2.0, 0.0)),
                        1,
                        Stroke::new(1.2_f32, MUTED),
                        egui::StrokeKind::Inside,
                    );
                }
                let name_right = (rect.right() - 285.0).max(rect.left() + 110.0);
                let name_painter = painter.with_clip_rect(Rect::from_min_max(
                    Pos2::new(rect.left() + 40.0, rect.top()),
                    Pos2::new(name_right, rect.bottom()),
                ));
                name_painter.text(
                    Pos2::new(rect.left() + 40.0, y),
                    Align2::LEFT_CENTER,
                    fit_text(ui, &node.name(), 14.0, name_right - rect.left() - 45.0),
                    FontId::proportional(14.0),
                    TEXT,
                );
                let bar_left = rect.right() - 270.0;
                let bar = Rect::from_min_size(Pos2::new(bar_left, y - 2.0), Vec2::new(105.0, 4.0));
                painter.rect_filled(bar, 2, LINE);
                painter.rect_filled(
                    Rect::from_min_size(
                        bar.min,
                        Vec2::new(bar.width() * percent(node.size, total) / 100.0, 4.0),
                    ),
                    2,
                    TEAL,
                );
                painter.text(
                    Pos2::new(rect.right() - 95.0, y),
                    Align2::RIGHT_CENTER,
                    size(node.size),
                    FontId::proportional(13.0),
                    TEXT,
                );
                painter.text(
                    Pos2::new(rect.right() - 19.0, y),
                    Align2::RIGHT_CENTER,
                    format!("{:.1}%", percent(node.size, total)),
                    FontId::proportional(13.0),
                    MUTED,
                );
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
                response.on_hover_text(format!(
                    "{}\n{}",
                    node.path.display(),
                    if node.directory {
                        "双击进入 · 右键更多操作"
                    } else {
                        "右键更多操作"
                    }
                ));
            }
        });
    (action, painted)
}

impl App {
    fn draw(&mut self, ctx: &egui::Context) {
        self.events();
        let mut action = None;
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(egui::Margin::symmetric(20, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    txt(ui, "▰", 22.0, TEAL);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        txt(ui, "空间视图", 19.0, TEXT);
                        txt(ui, "SPACE INSIGHT", 10.0, MUTED);
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary(ui, "选择文件夹").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.start(path, ctx.clone());
                            }
                        }
                        if self.last_path.is_some()
                            && self.cancel.is_none()
                            && button(ui, "重新扫描").clicked()
                        {
                            self.start(self.last_path.clone().unwrap(), ctx.clone());
                        }
                    });
                });
            });
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(egui::Margin::symmetric(20, 7)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    txt(ui, "文件逻辑大小 · 1024 进制", 11.0, MUTED);
                    if let Some(result) = &self.result {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let response = ui.label(
                                RichText::new(format!(
                                    "跳过 {} 个链接/特殊目录 · {} 个读取错误",
                                    result.skipped_links, result.skipped_errors
                                ))
                                .size(11.0)
                                .color(
                                    if result.skipped_errors > 0 {
                                        Color32::from_rgb(218, 175, 112)
                                    } else {
                                        MUTED
                                    },
                                ),
                            );
                            if let Some(error) = &result.first_error {
                                response.on_hover_text(format!("首个读取错误：{error}"));
                            }
                        });
                    }
                });
            });
        if !ctx.wants_keyboard_input()
            && ctx.input(|i| {
                i.key_pressed(egui::Key::Backspace)
                    || (i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft))
            })
        {
            action = Some(Action::Back);
        }
        self.refresh_list();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(20, 14)),
            )
            .show(ctx, |ui| {
                if self.cancel.is_some() {
                    ui.add_space(75.0);
                    txt(ui, "正在分析文件夹", 25.0, TEXT);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        txt(ui, format!("已发现 {} 个项目", self.scanned), 14.0, MUTED);
                        if button(ui, "取消扫描").clicked() {
                            self.stop();
                            self.message = Some("扫描已取消".into());
                        }
                    });
                    if let Some(path) = &self.last_path {
                        txt(ui, path.display().to_string(), 13.0, MUTED);
                    }
                    return;
                }
                if let Some(message) = &self.message {
                    txt(ui, message.clone(), 13.0, Color32::from_rgb(218, 175, 112));
                }
                let Some(result) = &self.result else {
                    ui.add_space(80.0);
                    txt(ui, "了解空间都去了哪里", 27.0, TEXT);
                    ui.add_space(8.0);
                    txt(
                        ui,
                        "选择一个文件夹，查看大小分布并逐层找到占用空间的文件。",
                        14.0,
                        MUTED,
                    );
                    ui.add_space(14.0);
                    if primary(ui, "选择文件夹开始分析").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.start(path, ctx.clone());
                        }
                    }
                    return;
                };
                let current = *self.trail.last().unwrap();
                let node = &result.nodes[current];
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(self.trail.len() > 1, egui::Button::new("← 上一级"))
                        .on_hover_text("返回上一级 (Backspace / Alt+←)")
                        .clicked()
                    {
                        action = Some(Action::Back);
                    }
                    egui::ScrollArea::horizontal()
                        .id_salt("breadcrumbs")
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (level, &i) in self.trail.iter().enumerate() {
                                    if level > 0 {
                                        txt(ui, "/", 13.0, MUTED);
                                    }
                                    let name = if level == 0 {
                                        result.nodes[i].path.display().to_string()
                                    } else {
                                        result.nodes[i].name()
                                    };
                                    if ui
                                        .link(
                                            RichText::new(name).size(13.0).color(if i == current {
                                                TEXT
                                            } else {
                                                MUTED
                                            }),
                                        )
                                        .clicked()
                                    {
                                        action = Some(Action::GoTo(level));
                                    }
                                }
                            });
                        });
                });
                ui.add_space(9.0);
                ui.horizontal_wrapped(|ui| {
                    txt(ui, size(node.size), 27.0, TEXT);
                    txt(
                        ui,
                        format!(
                            "  文件 {}  ·  当前目录 {} 项",
                            node.files,
                            node.children.len()
                        ),
                        13.0,
                        MUTED,
                    );
                });
                ui.add_space(9.0);
                if let Some(a) = distribution(ui, result, current, &self.cached_chart) {
                    action = Some(a);
                }
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    txt(ui, "目录内容", 17.0, TEXT);
                    txt(
                        ui,
                        format!(
                            "{} / {} 项",
                            self.cached_children.len(),
                            node.children.len()
                        ),
                        12.0,
                        MUTED,
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if button(
                            ui,
                            if self.sort_name {
                                "名称 ↑"
                            } else {
                                "大小 ↓"
                            },
                        )
                        .on_hover_text("切换排序方式")
                        .clicked()
                        {
                            self.sort_name = !self.sort_name;
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("搜索当前目录")
                                .desired_width(150.0),
                        );
                        if let Some(selected) = self.selected {
                            if button(ui, "在资源管理器中查看").clicked() {
                                action = Some(Action::Open(selected));
                            }
                        }
                    });
                });
                let (header, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 19.0), Sense::hover());
                let painter = ui.painter();
                let font = FontId::proportional(12.0);
                painter.text(
                    Pos2::new(header.left() + 40.0, header.center().y),
                    Align2::LEFT_CENTER,
                    "名称",
                    font.clone(),
                    MUTED,
                );
                painter.text(
                    Pos2::new(header.right() - 270.0, header.center().y),
                    Align2::LEFT_CENTER,
                    "占用",
                    font.clone(),
                    MUTED,
                );
                painter.text(
                    Pos2::new(header.right() - 95.0, header.center().y),
                    Align2::RIGHT_CENTER,
                    "大小",
                    font.clone(),
                    MUTED,
                );
                painter.text(
                    Pos2::new(header.right() - 19.0, header.center().y),
                    Align2::RIGHT_CENTER,
                    "占比",
                    font,
                    MUTED,
                );
                if self.cached_children.is_empty() {
                    ui.add_space(20.0);
                    txt(
                        ui,
                        if self.search.is_empty() {
                            "此文件夹没有可显示的项目"
                        } else {
                            "没有匹配的项目，试试其他关键词"
                        },
                        14.0,
                        MUTED,
                    );
                } else {
                    let (a, _) =
                        item_list(ui, result, current, &self.cached_children, self.selected);
                    if a.is_some() {
                        action = a;
                    }
                }
            });
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
    #[test]
    fn map_preserves_proportions() {
        let mut tiles = Vec::new();
        partition_map(
            &[(Some(1), 6, 0), (Some(2), 3, 1), (None, 1, 2)],
            Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 100.0)),
            &mut tiles,
        );
        for (_, bytes, _, rect) in tiles {
            let share = rect.width() * rect.height() / 20_000.0;
            assert!((share - bytes as f32 / 10.0).abs() < 0.0001);
        }
    }
    #[test]
    fn list_uses_the_available_viewport() {
        let mut nodes = vec![Node::new(PathBuf::from("root"), true, 100)];
        for i in 0..40 {
            nodes.push(Node::new(PathBuf::from(format!("item{i}")), false, 1));
            nodes[0].children.push(i + 1);
        }
        let result = ScanResult {
            nodes,
            skipped_links: 0,
            skipped_errors: 0,
            first_error: None,
        };
        let ctx = egui::Context::default();
        let mut painted = 0;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (_, count) = item_list(ui, &result, 0, &result.nodes[0].children, None);
                    painted = count;
                });
            },
        );
        assert!(painted >= 8, "only {painted} rows were visible");
        assert!(painted < 40, "list should only paint visible rows");
    }

    #[test]
    fn navigation_search_and_chart_stay_consistent() {
        let mut root = Node::new(PathBuf::from("root"), true, 0);
        root.size = 30;
        root.children = vec![1, 2];
        let mut folder = Node::new(PathBuf::from("root/folder"), true, 0);
        folder.size = 20;
        let file = Node::new(PathBuf::from("root/file"), false, 10);
        let mut app = App::default();
        app.result = Some(ScanResult {
            nodes: vec![root, folder, file],
            skipped_links: 0,
            skipped_errors: 0,
            first_error: None,
        });
        let ctx = egui::Context::default();
        app.refresh_list();
        assert_eq!(app.cached_children, vec![1, 2]);
        app.search = "file".into();
        app.refresh_list();
        assert_eq!(app.cached_children, vec![2]);
        assert_eq!(app.cached_chart, vec![1, 2]);
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
    fn full_window_renders_multiple_rows() {
        fn count_rows(shape: &egui::Shape) -> usize {
            match shape {
                egui::Shape::Text(text) => {
                    usize::from(text.pos.y > 360.0 && text.galley.job.text.starts_with("fixture-"))
                }
                egui::Shape::Vec(shapes) => shapes.iter().map(count_rows).sum(),
                _ => 0,
            }
        }
        let mut root = Node::new(PathBuf::from("root"), true, 0);
        let mut nodes = vec![];
        for i in 0..40 {
            root.children.push(i + 1);
            nodes.push(Node::new(
                PathBuf::from(format!("fixture-{i:02}")),
                false,
                1,
            ));
        }
        root.size = 40;
        nodes.insert(0, root);
        let mut app = App::default();
        app.result = Some(ScanResult {
            nodes,
            skipped_links: 0,
            skipped_errors: 0,
            first_error: None,
        });
        let ctx = egui::Context::default();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1050.0, 740.0))),
                ..Default::default()
            },
            |ctx| app.draw(ctx),
        );
        let rows: usize = output.shapes.iter().map(|s| count_rows(&s.shape)).sum();
        assert!(rows >= 5, "full window rendered only {rows} fixture labels");
    }
}
