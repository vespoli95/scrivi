use std::collections::HashMap;
use std::time::{Duration, Instant};

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    pos2, vec2, Color32, CornerRadius, CursorIcon, FontId, Id, Key, KeyboardShortcut, Modifiers,
    Pos2, Rect, Response, RichText, ScrollArea, Sense, Stroke, TextEdit, Ui, Vec2,
};

use crate::icons;
use crate::layout::{
    compute_category_drop_index, compute_category_grid, compute_heading_layout,
    compute_todo_drop_index, CELL_MIN_HEIGHT, CELL_MIN_WIDTH, GRID_GAP,
};
use crate::options::WindowPlatform;
use crate::store::{MemoryDb, Todo, CATEGORY_EMOJIS};
use crate::theme::{
    apply_egui_theme, category_color, palette, APP_THEMES, CATEGORY_COLORS, DEFAULT_THEME,
};
use crate::updates::{UpdateController, UpdateEvent};

#[derive(Clone, Debug)]
pub struct RenderedTitleLine {
    pub text: String,
    pub text_rect: Rect,
    pub strike_segment: Option<(Pos2, Pos2)>,
}

#[derive(Clone, Debug)]
pub struct TodoTitleMetrics {
    pub rect: Rect,
    pub required_height: f32,
    pub lines: Vec<RenderedTitleLine>,
}

#[derive(Clone, Debug)]
pub struct TodoRowRects {
    pub row_rect: Rect,
    pub handle_rect: Rect,
    pub checkbox_rect: Rect,
    pub title_rect: Rect,
    pub edit_rect: Rect,
    pub delete_rect: Rect,
}

#[derive(Clone, Debug)]
pub struct CategoryPanelRects {
    pub card_rect: Rect,
    pub handle_rect: Rect,
    pub emoji_rect: Rect,
    pub label_rect: Rect,
    pub heading_avail_rect: Rect,
    pub color_rect: Rect,
    pub delete_rect: Rect,
    pub todo_input_rect: Rect,
    pub todo_add_rect: Rect,
    pub scroll_rect: Rect,
    pub content_height: f32,
    pub todo_rows: Vec<(i64, TodoRowRects)>,
}

#[derive(Clone, Debug)]
pub struct UiSnapshot {
    pub pin_rect: Option<Rect>,
    pub settings_rect: Option<Rect>,
    pub new_cat_emoji_rect: Option<Rect>,
    pub new_cat_input_rect: Option<Rect>,
    pub new_cat_add_rect: Option<Rect>,
    pub empty_categories_rect: Option<Rect>,
    pub outer_scroll_rect: Rect,
    pub outer_content_height: f32,
    pub category_panels: HashMap<i64, CategoryPanelRects>,
    pub title_metrics: HashMap<i64, TodoTitleMetrics>,
    pub modal_rect: Option<Rect>,
    pub modal_title: Option<String>,
    pub modal_message: Option<String>,
    pub modal_buttons: Vec<(String, Rect)>,
    pub modal_input_rect: Option<Rect>,
    pub modal_emoji_rects: Vec<(String, Rect)>,
    pub modal_theme_combo_rect: Option<Rect>,
    pub modal_theme_items: Vec<(String, Rect)>,
    pub modal_startup_check_rect: Option<Rect>,
    pub modal_startup_disabled: bool,
    pub modal_startup_checked: bool,
}

impl Default for UiSnapshot {
    fn default() -> Self {
        Self {
            pin_rect: None,
            settings_rect: None,
            new_cat_emoji_rect: None,
            new_cat_input_rect: None,
            new_cat_add_rect: None,
            empty_categories_rect: None,
            outer_scroll_rect: Rect::NOTHING,
            outer_content_height: 0.0,
            category_panels: HashMap::new(),
            title_metrics: HashMap::new(),
            modal_rect: None,
            modal_title: None,
            modal_message: None,
            modal_buttons: Vec::new(),
            modal_input_rect: None,
            modal_emoji_rects: Vec::new(),
            modal_theme_combo_rect: None,
            modal_theme_items: Vec::new(),
            modal_startup_check_rect: None,
            modal_startup_disabled: false,
            modal_startup_checked: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragPayload {
    Category(i64),
    Todo(i64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EmojiTarget {
    NewCategory,
    Category(i64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ConfirmKind {
    DeleteCategory(i64),
    InstallUpdate {
        version: String,
        download_url: String,
    },
}

#[derive(Clone, Debug)]
enum Modal {
    Error {
        message: String,
        close_app_on_ok: bool,
    },
    Confirm {
        title: String,
        message: String,
        kind: ConfirmKind,
    },
    EmojiPicker {
        selected: String,
        target: EmojiTarget,
    },
    ColorPicker {
        category_id: i64,
        selected: String,
    },
    EditTodo {
        todo_id: i64,
        draft: String,
        request_focus: bool,
    },
    Settings {
        startup_checked: bool,
        startup_disabled: bool,
        theme_popup_open: bool,
    },
    DownloadingUpdate,
}

#[derive(Clone, Copy, Debug)]
enum ButtonImportance {
    Low,
    Medium,
    High,
}

pub struct ScriviApp {
    db: Option<MemoryDb>,
    platform: Box<dyn WindowPlatform>,
    pinned: bool,
    new_category_name: String,
    new_category_icon: String,
    focus_new_category: bool,
    todo_drafts: HashMap<i64, String>,
    focus_todo_entry: Option<i64>,
    outer_scroll_offset: f32,
    outer_content_height: f32,
    outer_scroll_rect: Rect,
    inner_scroll_offsets: HashMap<i64, f32>,
    inner_content_heights: HashMap<i64, f32>,
    inner_scroll_rects: HashMap<i64, Rect>,
    grid_columns: usize,
    category_rects: HashMap<i64, Rect>,
    todo_row_rects: HashMap<i64, Vec<(i64, Rect)>>,
    active_drag: Option<DragPayload>,
    hovered_category: Option<i64>,
    last_drag_pos: Option<Pos2>,
    last_auto_scroll: Option<Instant>,
    modals: Vec<(u64, Modal)>,
    next_modal_id: u64,
    updates: Option<UpdateController>,
    theme_applied: String,
    should_close: bool,
    last_cursor_icon: CursorIcon,
    snapshot: UiSnapshot,
}

impl ScriviApp {
    pub fn new(
        db: MemoryDb,
        platform: Box<dyn WindowPlatform>,
        updates: Option<UpdateController>,
    ) -> Self {
        let theme = db.app_theme().to_string();
        Self {
            db: Some(db),
            platform,
            pinned: false,
            new_category_name: String::new(),
            new_category_icon: String::new(),
            focus_new_category: false,
            todo_drafts: HashMap::new(),
            focus_todo_entry: None,
            outer_scroll_offset: 0.0,
            outer_content_height: 0.0,
            outer_scroll_rect: Rect::NOTHING,
            inner_scroll_offsets: HashMap::new(),
            inner_content_heights: HashMap::new(),
            inner_scroll_rects: HashMap::new(),
            grid_columns: 2,
            category_rects: HashMap::new(),
            todo_row_rects: HashMap::new(),
            active_drag: None,
            hovered_category: None,
            last_drag_pos: None,
            last_auto_scroll: None,
            modals: Vec::new(),
            next_modal_id: 1,
            updates,
            theme_applied: theme,
            should_close: false,
            last_cursor_icon: CursorIcon::Default,
            snapshot: UiSnapshot::default(),
        }
    }

    pub fn startup_error(message: impl Into<String>) -> Self {
        #[cfg(not(windows))]
        let platform: Box<dyn WindowPlatform> = Box::new(crate::options::DefaultPlatform);
        #[cfg(windows)]
        let platform: Box<dyn WindowPlatform> = Box::new(crate::options::WindowsPlatform::new(0));

        Self {
            db: None,
            platform,
            pinned: false,
            new_category_name: String::new(),
            new_category_icon: String::new(),
            focus_new_category: false,
            todo_drafts: HashMap::new(),
            focus_todo_entry: None,
            outer_scroll_offset: 0.0,
            outer_content_height: 0.0,
            outer_scroll_rect: Rect::NOTHING,
            inner_scroll_offsets: HashMap::new(),
            inner_content_heights: HashMap::new(),
            inner_scroll_rects: HashMap::new(),
            grid_columns: 2,
            category_rects: HashMap::new(),
            todo_row_rects: HashMap::new(),
            active_drag: None,
            hovered_category: None,
            last_drag_pos: None,
            last_auto_scroll: None,
            modals: vec![(
                1,
                Modal::Error {
                    message: message.into(),
                    close_app_on_ok: true,
                },
            )],
            next_modal_id: 2,
            updates: None,
            theme_applied: DEFAULT_THEME.to_string(),
            should_close: false,
            last_cursor_icon: CursorIcon::Default,
            snapshot: UiSnapshot::default(),
        }
    }

    pub fn db(&self) -> Option<&MemoryDb> {
        self.db.as_ref()
    }

    pub fn db_mut(&mut self) -> Option<&mut MemoryDb> {
        self.db.as_mut()
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    pub fn outer_scroll_offset(&self) -> f32 {
        self.outer_scroll_offset
    }

    pub fn inner_scroll_offset(&self, category_id: i64) -> f32 {
        self.inner_scroll_offsets
            .get(&category_id)
            .copied()
            .unwrap_or(0.0)
    }

    pub fn set_inner_scroll_offset_for_test(&mut self, category_id: i64, offset: f32) {
        let max_off = (self
            .inner_content_heights
            .get(&category_id)
            .copied()
            .unwrap_or(0.0)
            - self
                .inner_scroll_rects
                .get(&category_id)
                .map(|r| r.height())
                .unwrap_or(0.0))
        .max(0.0);
        self.inner_scroll_offsets
            .insert(category_id, offset.clamp(0.0, max_off));
    }

    pub fn set_outer_scroll_offset_for_test(&mut self, offset: f32) {
        let max_off = (self.outer_content_height - self.outer_scroll_rect.height()).max(0.0);
        self.outer_scroll_offset = offset.clamp(0.0, max_off);
    }

    pub fn snapshot(&self) -> &UiSnapshot {
        &self.snapshot
    }

    pub fn last_cursor_icon(&self) -> CursorIcon {
        self.last_cursor_icon
    }

    pub fn is_closing(&self) -> bool {
        self.should_close
    }

    pub fn new_category_name(&self) -> &str {
        &self.new_category_name
    }

    pub fn todo_draft(&self, category_id: i64) -> &str {
        self.todo_drafts
            .get(&category_id)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn active_modal_title(&self) -> Option<&str> {
        self.snapshot.modal_title.as_deref()
    }

    pub fn scroll_near(&mut self, point: Pos2) {
        if self.outer_scroll_rect.contains(point) {
            let max_outer = (self.outer_content_height - self.outer_scroll_rect.height()).max(0.0);
            let before = self.outer_scroll_offset;
            if point.y < self.outer_scroll_rect.top() + 24.0 {
                self.outer_scroll_offset = (self.outer_scroll_offset - 12.0).clamp(0.0, max_outer);
            } else if point.y > self.outer_scroll_rect.bottom() - 24.0 {
                self.outer_scroll_offset = (self.outer_scroll_offset + 12.0).clamp(0.0, max_outer);
            }
            if (self.outer_scroll_offset - before).abs() > f32::EPSILON {
                return;
            }
        }
        if let Some(cat_id) = self.category_at_point(point) {
            if let Some(&inner_rect) = self.inner_scroll_rects.get(&cat_id) {
                if inner_rect.contains(point) {
                    let max_inner = (self
                        .inner_content_heights
                        .get(&cat_id)
                        .copied()
                        .unwrap_or(0.0)
                        - inner_rect.height())
                    .max(0.0);
                    let offset = self.inner_scroll_offsets.entry(cat_id).or_insert(0.0);
                    if point.y < inner_rect.top() + 24.0 {
                        *offset = (*offset - 12.0).clamp(0.0, max_inner);
                    } else if point.y > inner_rect.bottom() - 24.0 {
                        *offset = (*offset + 12.0).clamp(0.0, max_inner);
                    }
                }
            }
        }
    }

    fn current_theme_name(&self) -> &str {
        self.db
            .as_ref()
            .map(MemoryDb::app_theme)
            .unwrap_or(DEFAULT_THEME)
    }

    fn push_modal(&mut self, modal: Modal) {
        let id = self.next_modal_id;
        self.next_modal_id += 1;
        self.modals.push((id, modal));
    }

    fn push_error(&mut self, message: impl Into<String>) {
        self.push_modal(Modal::Error {
            message: message.into(),
            close_app_on_ok: false,
        });
    }

    fn poll_updates(&mut self, ctx: &egui::Context) {
        let mut events = Vec::new();
        if let Some(ctrl) = &self.updates {
            while let Some(ev) = ctrl.try_recv() {
                events.push(ev);
            }
        }
        for ev in events {
            match ev {
                UpdateEvent::Available(info) => {
                    let suppressed = self
                        .updates
                        .as_ref()
                        .and_then(|c| c.declined_version.as_ref())
                        == Some(&info.version);
                    let already_prompted = self.modals.iter().any(|(_, m)| {
                        matches!(
                            m,
                            Modal::Confirm {
                                kind: ConfirmKind::InstallUpdate { .. },
                                ..
                            } | Modal::DownloadingUpdate
                        )
                    });
                    if !suppressed && !already_prompted {
                        self.push_modal(Modal::Confirm {
                            title: "Update available".into(),
                            message: format!(
                                "scrivi {} is available (installed: {}).\nDownload and run the unsigned installer now?\nScrivi will restart. Your todos will be kept.",
                                info.version,
                                crate::updates::APP_VERSION
                            ),
                            kind: ConfirmKind::InstallUpdate {
                                version: info.version,
                                download_url: info.download_url,
                            },
                        });
                    }
                }
                UpdateEvent::InstallFinished(res) => {
                    self.modals
                        .retain(|(_, m)| !matches!(m, Modal::DownloadingUpdate));
                    match res {
                        Ok(()) => {
                            self.should_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        Err(err) => {
                            self.push_error(err);
                        }
                    }
                }
            }
        }
    }

    fn category_at_point(&self, point: Pos2) -> Option<i64> {
        if !self.outer_scroll_rect.contains(point) {
            return None;
        }
        let db = self.db.as_ref()?;
        for group in db.list_categories() {
            if let Some(&rect) = self.category_rects.get(&group.id) {
                if rect.contains(point) {
                    return Some(group.id);
                }
            }
        }
        None
    }

    fn handle_drag_state(&mut self, ctx: &egui::Context) {
        let Some(drag) = self.active_drag else {
            return;
        };

        let pointer_down = ctx.input(|i| i.pointer.primary_down());
        let pointer_pos = ctx
            .input(|i| i.pointer.interact_pos())
            .or(self.last_drag_pos);

        if pointer_down {
            if let Some(pos) = pointer_pos {
                self.last_drag_pos = Some(pos);
                self.hovered_category = self.category_at_point(pos);
                let now = Instant::now();
                let should_tick = self
                    .last_auto_scroll
                    .is_none_or(|prev| now.duration_since(prev) >= Duration::from_millis(60));
                if should_tick {
                    self.scroll_near(pos);
                    self.last_auto_scroll = Some(now);
                }
                ctx.request_repaint_after(Duration::from_millis(60));
            }
            return;
        }

        // Pointer released: finalize drop
        self.active_drag = None;
        self.hovered_category = None;
        self.last_auto_scroll = None;
        let drop_point = pointer_pos.or(self.last_drag_pos);
        self.last_drag_pos = None;

        let Some(point) = drop_point else {
            return;
        };

        match drag {
            DragPayload::Category(id) => self.drop_category(id, point),
            DragPayload::Todo(id) => self.drop_todo(id, point),
        }
    }

    fn drop_category(&mut self, id: i64, point: Pos2) {
        let Some(target_id) = self.category_at_point(point) else {
            return;
        };
        if target_id == id {
            return;
        }
        let Some(&target_rect) = self.category_rects.get(&target_id) else {
            return;
        };
        let Some(db) = self.db.as_mut() else {
            return;
        };
        let ordered_ids: Vec<i64> = db.list_categories().into_iter().map(|c| c.id).collect();
        let index = compute_category_drop_index(
            &ordered_ids,
            id,
            target_id,
            target_rect,
            point,
            self.grid_columns,
        );
        if let Err(err) = db.move_category(id, index as isize) {
            self.push_error(err.to_string());
        }
    }

    fn drop_todo(&mut self, id: i64, point: Pos2) {
        let Some(target_cat_id) = self.category_at_point(point) else {
            return;
        };
        let row_rects = self
            .todo_row_rects
            .get(&target_cat_id)
            .cloned()
            .unwrap_or_default();
        let index = compute_todo_drop_index(&row_rects, id, point);
        let Some(db) = self.db.as_mut() else {
            return;
        };
        if let Err(err) = db.move_todo(id, target_cat_id, index as isize) {
            self.push_error(err.to_string());
        }
    }

    fn try_add_category(&mut self) {
        let Some(db) = self.db.as_mut() else {
            return;
        };
        match db.add_category_with_icon(&self.new_category_name, &self.new_category_icon) {
            Ok(_) => {
                self.new_category_name.clear();
                self.new_category_icon.clear();
                self.focus_new_category = true;
            }
            Err(err) => {
                self.push_error(err.to_string());
            }
        }
    }

    fn open_settings_modal(&mut self) {
        let (startup_checked, startup_disabled, read_err) = if self.platform.supports_startup() {
            match self.platform.startup_enabled() {
                Ok(enabled) => (enabled, false, None),
                Err(err) => (false, true, Some(err)),
            }
        } else {
            (false, false, None)
        };
        self.push_modal(Modal::Settings {
            startup_checked,
            startup_disabled,
            theme_popup_open: false,
        });
        if let Some(err) = read_err {
            self.push_error(err);
        }
    }

    pub fn render(&mut self, ctx: &egui::Context) {
        self.poll_updates(ctx);

        let theme_name = self.current_theme_name().to_string();
        if self.theme_applied != theme_name {
            apply_egui_theme(ctx, &theme_name);
            self.theme_applied = theme_name.clone();
        }

        // Handle drag update/drop before resetting per-frame geometry or at end?
        // Using previous frame's rects lets drop hit-testing work accurately even on release frame!
        self.handle_drag_state(ctx);

        self.snapshot = UiSnapshot::default();
        self.category_rects.clear();
        self.todo_row_rects.clear();
        self.inner_scroll_rects.clear();

        let p = palette(&theme_name);
        let has_modal = !self.modals.is_empty();

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.background).inner_margin(12))
            .show(ctx, |ui| {
                let panel_inner_rect = ui.max_rect();
                ui.add_enabled_ui(!has_modal, |ui| {
                    self.render_header(ui, &theme_name);
                    ui.add_space(10.0);
                    self.render_category_grid(ui, panel_inner_rect, &theme_name);
                });
            });

        self.render_modals(ctx, &theme_name);

        if self.should_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        self.last_cursor_icon = ctx.output(|o| o.cursor_icon);
    }

    fn render_header(&mut self, ui: &mut Ui, theme_name: &str) {
        let p = palette(theme_name);
        ui.vertical(|ui| {
            // Top bar: title + Pin/Settings
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("scrivi")
                        .size(20.0)
                        .strong()
                        .italics()
                        .color(p.foreground),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let settings_resp = icon_text_button(
                        ui,
                        "Settings",
                        ButtonImportance::Medium,
                        theme_name,
                        icons::paint_settings,
                    );
                    self.snapshot.settings_rect = Some(settings_resp.rect);
                    if settings_resp.clicked() {
                        self.open_settings_modal();
                    }

                    if self.platform.supports_pin() {
                        ui.add_space(6.0);
                        let pin_label = if self.pinned { "Unpin" } else { "Pin" };
                        let importance = if self.pinned {
                            ButtonImportance::High
                        } else {
                            ButtonImportance::Medium
                        };
                        let pin_resp = icon_text_button(
                            ui,
                            pin_label,
                            importance,
                            theme_name,
                            icons::paint_pin,
                        );
                        self.snapshot.pin_rect = Some(pin_resp.rect);
                        if pin_resp.clicked() {
                            let next = !self.pinned;
                            match self.platform.set_pinned(ui.ctx(), next) {
                                Ok(()) => self.pinned = next,
                                Err(err) => self.push_error(err),
                            }
                        }
                    }
                });
            });

            ui.add_space(8.0);

            // Add category row
            ui.horizontal(|ui| {
                let emoji_resp =
                    emoji_button(ui, &self.new_category_icon, false, vec2(40.0, 40.0), theme_name);
                self.snapshot.new_cat_emoji_rect = Some(emoji_resp.rect);
                if emoji_resp.clicked() {
                    self.push_modal(Modal::EmojiPicker {
                        selected: self.new_category_icon.clone(),
                        target: EmojiTarget::NewCategory,
                    });
                }

                let add_width = icon_text_button_width(ui, "Add category").max(128.0);
                let spacing = ui.spacing().item_spacing.x;
                let entry_margin = egui::Margin::symmetric(10, 10);
                let entry_outer_width = (ui.available_width() - add_width - spacing)
                    .max(entry_margin.sum().x + 24.0);
                let entry_inner_width = (entry_outer_width - entry_margin.sum().x).max(0.0);

                let input_id = Id::new("new_category_name_input");
                let output = TextEdit::singleline(&mut self.new_category_name)
                    .id(input_id)
                    .hint_text("Category name (Life, Money, Work...)")
                    .desired_width(entry_inner_width)
                    .margin(entry_margin)
                    .show(ui);
                self.snapshot.new_cat_input_rect = Some(output.response.rect);

                if self.focus_new_category {
                    output.response.request_focus();
                    self.focus_new_category = false;
                }

                let submitted = output.response.lost_focus()
                    && ui.input(|i| i.key_pressed(Key::Enter));

                let add_resp = icon_text_button_sized(
                    ui,
                    "Add category",
                    vec2(add_width, 40.0),
                    ButtonImportance::High,
                    theme_name,
                    icons::paint_plus,
                );
                self.snapshot.new_cat_add_rect = Some(add_resp.rect);

                if submitted || add_resp.clicked() {
                    self.try_add_category();
                }
            });
        });
    }

    fn render_category_grid(&mut self, ui: &mut Ui, panel_inner_rect: Rect, theme_name: &str) {
        let p = palette(theme_name);
        let Some(db) = self.db.as_ref() else {
            return;
        };
        let groups = db.list_categories();
        let raw_avail = ui.available_rect_before_wrap();
        let avail_rect = Rect::from_min_max(
            pos2(panel_inner_rect.left(), raw_avail.top()),
            pos2(
                panel_inner_rect.right(),
                panel_inner_rect.bottom().max(raw_avail.top()),
            ),
        );
        self.outer_scroll_rect = avail_rect;
        self.snapshot.outer_scroll_rect = avail_rect;

        // Clean stale drafts & scroll offsets for deleted categories
        self.todo_drafts
            .retain(|id, _| groups.iter().any(|g| g.id == *id));
        self.inner_scroll_offsets
            .retain(|id, _| groups.iter().any(|g| g.id == *id));

        if groups.is_empty() {
            let mut scroll_ui = ui.new_child(egui::UiBuilder::new().max_rect(avail_rect));
            scroll_ui.centered_and_justified(|ui| {
                let resp = ui.label(
                    RichText::new("Create a category to get started.").color(p.muted),
                );
                self.snapshot.empty_categories_rect = Some(resp.rect);
            });
            self.outer_content_height = avail_rect.height();
            self.snapshot.outer_content_height = avail_rect.height();
            return;
        }

        let mut pending_actions: Vec<UiAction> = Vec::new();
        let mut grid_columns = self.grid_columns;
        let mut grid_content_height = 0.0_f32;

        let scroll_out = ScrollArea::vertical()
            .id_salt("category_grid_scroll")
            .max_width(avail_rect.width())
            .auto_shrink([false, false])
            .vertical_scroll_offset(self.outer_scroll_offset)
            .show(ui, |ui| {
                let grid_width = ui.available_width().min(avail_rect.width()).max(0.0);
                let grid = compute_category_grid(
                    groups.len(),
                    vec2(grid_width, avail_rect.height()),
                    vec2(CELL_MIN_WIDTH, CELL_MIN_HEIGHT),
                    GRID_GAP,
                );
                grid_columns = grid.columns;
                grid_content_height = grid.content_height;

                let (canvas_rect, _) = ui.allocate_exact_size(
                    vec2(grid_width, grid.content_height),
                    Sense::hover(),
                );
                for (idx, group) in groups.iter().enumerate() {
                    let card_rect = grid.rects[idx].translate(canvas_rect.min.to_vec2());
                    self.category_rects.insert(group.id, card_rect);
                    let panel_rects = ui.scope_builder(
                        egui::UiBuilder::new().max_rect(card_rect),
                        |ui| {
                            self.render_category_card(
                                ui,
                                card_rect,
                                group,
                                theme_name,
                                &mut pending_actions,
                            )
                        },
                    );
                    self.snapshot
                        .category_panels
                        .insert(group.id, panel_rects.inner);
                }
            });

        self.grid_columns = grid_columns;
        self.outer_content_height = grid_content_height;
        self.snapshot.outer_content_height = grid_content_height;
        self.outer_scroll_rect = scroll_out.inner_rect;
        self.snapshot.outer_scroll_rect = scroll_out.inner_rect;

        let max_outer =
            (scroll_out.content_size.y - scroll_out.inner_rect.height()).max(0.0);
        self.outer_scroll_offset = scroll_out.state.offset.y.clamp(0.0, max_outer);

        for action in pending_actions {
            self.apply_action(action);
        }
    }

    fn render_category_card(
        &mut self,
        ui: &mut Ui,
        card_rect: Rect,
        group: &crate::store::Category,
        theme_name: &str,
        actions: &mut Vec<UiAction>,
    ) -> CategoryPanelRects {
        let p = palette(theme_name);
        let fill = category_color(&group.color, theme_name);
        let border_stroke = if self.hovered_category == Some(group.id) {
            Stroke::new(2.0_f32, p.primary)
        } else {
            Stroke::new(1.0_f32, p.border)
        };
        ui.painter()
            .rect(card_rect, CornerRadius::same(10), fill, border_stroke, egui::StrokeKind::Inside);

        let inner_rect = card_rect.shrink(12.0);
        let mut card_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner_rect));
        card_ui.set_clip_rect(ui.clip_rect().intersect(card_rect));

        // Title bar
        let header_height = 40.0;
        let header_rect = Rect::from_min_size(
            inner_rect.min,
            vec2(inner_rect.width(), header_height),
        );
        let btn_side = 28.0;
        let btn_gap = 4.0;
        let controls_w = btn_side * 2.0 + btn_gap;

        let handle_rect = Rect::from_min_size(
            pos2(header_rect.left(), header_rect.center().y - btn_side * 0.5),
            vec2(btn_side, btn_side),
        );
        let handle_resp = card_ui
            .interact(handle_rect, Id::new(("cat_drag", group.id)), Sense::drag())
            .on_hover_cursor(CursorIcon::PointingHand);
        let handle_color = if handle_resp.hovered() || handle_resp.dragged() {
            p.foreground
        } else {
            p.muted
        };
        icons::paint_drag_handle(&card_ui, handle_rect, handle_color);
        if handle_resp.drag_started() {
            self.active_drag = Some(DragPayload::Category(group.id));
            self.last_drag_pos = handle_resp.interact_pointer_pos();
        }

        let delete_rect = Rect::from_min_size(
            pos2(
                header_rect.right() - btn_side,
                header_rect.center().y - btn_side * 0.5,
            ),
            vec2(btn_side, btn_side),
        );
        let color_rect = Rect::from_min_size(
            pos2(delete_rect.left() - btn_gap - btn_side, delete_rect.top()),
            vec2(btn_side, btn_side),
        );
        let color_resp = icon_button_in_rect(
            &mut card_ui,
            color_rect,
            Id::new(("cat_color", group.id)),
            ButtonImportance::Low,
            theme_name,
            icons::paint_palette,
        );
        if color_resp.clicked() {
            actions.push(UiAction::OpenColorPicker {
                category_id: group.id,
                selected: group.color.clone(),
            });
        }

        let delete_resp = icon_button_in_rect(
            &mut card_ui,
            delete_rect,
            Id::new(("cat_delete", group.id)),
            ButtonImportance::Low,
            theme_name,
            icons::paint_trash,
        );
        if delete_resp.clicked() {
            actions.push(UiAction::ConfirmDeleteCategory {
                id: group.id,
                name: group.name.clone(),
            });
        }

        // Centered heading between balanced left/right controls
        let heading_left = header_rect.left() + controls_w + 6.0;
        let heading_right = (header_rect.right() - controls_w - 6.0).max(heading_left);
        let heading_avail = Rect::from_min_max(
            pos2(heading_left, header_rect.top()),
            pos2(heading_right, header_rect.bottom()),
        );
        let heading_font = FontId::proportional(16.0);
        let name_galley = card_ui.fonts(|f| {
            f.layout_no_wrap(group.name.clone(), heading_font.clone(), p.foreground)
        });
        let heading = compute_heading_layout(
            heading_avail,
            vec2(40.0, 40.0),
            name_galley.size().x + 6.0,
            6.0,
        );
        let emoji_resp = emoji_button_in_rect(
            &mut card_ui,
            heading.icon_rect,
            Id::new(("cat_emoji", group.id)),
            &group.icon,
            false,
            theme_name,
        );
        if emoji_resp.clicked() {
            actions.push(UiAction::OpenCategoryEmojiPicker {
                category_id: group.id,
                selected: group.icon.clone(),
            });
        }

        // Render bold category label with ellipsis truncation inside heading.label_rect
        if heading.label_rect.width() > 2.0 {
            let mut label_job = LayoutJob::single_section(
                group.name.clone(),
                TextFormat {
                    font_id: heading_font,
                    color: p.foreground,
                    ..Default::default()
                },
            );
            label_job.wrap = TextWrapping {
                max_width: heading.label_rect.width(),
                max_rows: 1,
                break_anywhere: true,
                overflow_character: Some('…'),
            };
            let truncated = card_ui.fonts(|f| f.layout_job(label_job));
            let text_pos = pos2(
                heading.label_rect.left(),
                heading.label_rect.center().y - truncated.size().y * 0.5,
            );
            card_ui.painter().galley(text_pos, truncated, p.foreground);
        }

        // Advance cursor below header
        card_ui.allocate_rect(header_rect, Sense::hover());
        card_ui.add_space(6.0);

        // Separator line
        let (sep_rect, _) = card_ui.allocate_exact_size(vec2(inner_rect.width(), 1.0), Sense::hover());
        card_ui.painter().line_segment(
            [
                pos2(sep_rect.left(), sep_rect.center().y),
                pos2(sep_rect.right(), sep_rect.center().y),
            ],
            Stroke::new(1.0_f32, p.border),
        );
        card_ui.add_space(8.0);

        // Multiline todo input row
        let draft = self.todo_drafts.entry(group.id).or_default();
        let focus_this = self.focus_todo_entry == Some(group.id);
        if focus_this {
            self.focus_todo_entry = None;
        }

        let add_side = 36.0;
        let spacing = card_ui.spacing().item_spacing.x;
        let entry_outer_width = (inner_rect.width() - add_side - spacing).max(24.0);
        let row_top = card_ui.cursor().top();
        let entry_max_rect = Rect::from_min_max(
            pos2(inner_rect.left(), row_top),
            pos2(
                inner_rect.left() + entry_outer_width,
                inner_rect.bottom().max(row_top),
            ),
        );
        let mut entry_ui = card_ui.new_child(
            egui::UiBuilder::new()
                .max_rect(entry_max_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        entry_ui.set_clip_rect(card_ui.clip_rect().intersect(Rect::from_min_max(
            pos2(inner_rect.left() - 1.0, row_top - 1.0),
            pos2(entry_max_rect.right() + 1.0, inner_rect.bottom().max(row_top)),
        )));
        let entry_id = Id::new(("todo_entry", group.id));
        let (entry_resp, enter_submit) = render_multiline_submit_entry(
            &mut entry_ui,
            entry_id,
            draft,
            "What needs doing?",
            entry_outer_width,
            2,
            focus_this,
        );
        let todo_input_rect = entry_resp.rect;

        let row_height = entry_ui.min_rect().height().max(add_side);
        let add_rect = Rect::from_min_size(
            pos2(inner_rect.right() - add_side, row_top),
            vec2(add_side, row_height),
        );
        let add_resp = icon_button_in_rect(
            &mut card_ui,
            add_rect,
            Id::new(("todo_add_btn", group.id)),
            ButtonImportance::High,
            theme_name,
            icons::paint_plus,
        );
        let todo_add_rect = add_resp.rect;

        if enter_submit || add_resp.clicked() {
            actions.push(UiAction::AddTodo {
                category_id: group.id,
            });
        }

        card_ui.advance_cursor_after_rect(Rect::from_min_size(
            pos2(inner_rect.left(), row_top),
            vec2(inner_rect.width(), row_height),
        ));

        card_ui.add_space(8.0);

        // Scrollable todo list
        let raw_scroll = card_ui.available_rect_before_wrap();
        let scroll_avail = Rect::from_min_max(
            pos2(inner_rect.left(), raw_scroll.top()),
            pos2(inner_rect.right(), inner_rect.bottom().max(raw_scroll.top())),
        );
        self.inner_scroll_rects.insert(group.id, scroll_avail);

        let items = self
            .db
            .as_ref()
            .map(|db| db.list_category_todos(group.id))
            .unwrap_or_default();

        let prev_inner_offset = self
            .inner_scroll_offsets
            .get(&group.id)
            .copied()
            .unwrap_or(0.0);

        let mut row_rects_for_drop = Vec::new();
        let mut todo_rows_snapshot = Vec::new();

        let inner_scroll_out = ScrollArea::vertical()
            .id_salt(("cat_todos_scroll", group.id))
            .max_width(scroll_avail.width())
            .auto_shrink([false, false])
            .vertical_scroll_offset(prev_inner_offset)
            .show(&mut card_ui, |ui| {
                let list_width = ui.available_width().min(scroll_avail.width()).max(1.0);
                if items.is_empty() {
                    ui.label(
                        RichText::new("No todos yet. Add one above.").color(p.muted),
                    );
                    return;
                }
                for (idx, item) in items.iter().enumerate() {
                    if idx > 0 {
                        ui.add_space(4.0);
                    }
                    let row_rects = self.render_todo_row(
                        ui,
                        list_width,
                        item,
                        theme_name,
                        actions,
                    );
                    row_rects_for_drop.push((item.id, row_rects.row_rect));
                    todo_rows_snapshot.push((item.id, row_rects));
                }
            });

        let max_inner =
            (inner_scroll_out.content_size.y - inner_scroll_out.inner_rect.height()).max(0.0);
        self.inner_content_heights
            .insert(group.id, inner_scroll_out.content_size.y);
        self.inner_scroll_offsets.insert(
            group.id,
            inner_scroll_out.state.offset.y.clamp(0.0, max_inner),
        );
        self.todo_row_rects.insert(group.id, row_rects_for_drop);

        CategoryPanelRects {
            card_rect,
            handle_rect,
            emoji_rect: emoji_resp.rect,
            label_rect: heading.label_rect,
            heading_avail_rect: heading_avail,
            color_rect,
            delete_rect,
            todo_input_rect,
            todo_add_rect,
            scroll_rect: scroll_avail,
            content_height: inner_scroll_out.content_size.y,
            todo_rows: todo_rows_snapshot,
        }
    }

    fn render_todo_row(
        &mut self,
        ui: &mut Ui,
        row_width: f32,
        item: &Todo,
        theme_name: &str,
        actions: &mut Vec<UiAction>,
    ) -> TodoRowRects {
        let p = palette(theme_name);
        let handle_side = 22.0;
        let check_side = 20.0;
        let btn_side = 26.0;
        let gap = 6.0;
        let left_width = handle_side + gap + check_side;
        let right_width = btn_side + 4.0 + btn_side;
        let title_width = (row_width - left_width - right_width - 2.0 * gap).max(1.0);
        let inner_pad_x = 4.0;
        let inner_pad_y = 4.0;
        let wrap_width = (title_width - inner_pad_x * 2.0).max(1.0);

        let ink = if item.done { p.muted } else { p.foreground };
        let font_id = FontId::proportional(14.0);
        let galley = layout_todo_galley(ui, &item.title, wrap_width, font_id, ink);
        let required_title_height = galley.size().y + inner_pad_y * 2.0;
        let row_height = handle_side
            .max(check_side)
            .max(btn_side)
            .max(required_title_height);

        let (row_rect, _) = ui.allocate_exact_size(vec2(row_width, row_height), Sense::hover());

        let handle_rect = Rect::from_min_size(
            pos2(row_rect.left(), row_rect.center().y - handle_side * 0.5),
            vec2(handle_side, handle_side),
        );
        let handle_resp = ui
            .interact(handle_rect, Id::new(("todo_drag", item.id)), Sense::drag())
            .on_hover_cursor(CursorIcon::PointingHand);
        let handle_color = if handle_resp.hovered() || handle_resp.dragged() {
            p.foreground
        } else {
            p.muted
        };
        icons::paint_drag_handle(ui, handle_rect, handle_color);
        if handle_resp.drag_started() {
            self.active_drag = Some(DragPayload::Todo(item.id));
            self.last_drag_pos = handle_resp.interact_pointer_pos();
        }

        let checkbox_rect = Rect::from_min_size(
            pos2(
                handle_rect.right() + gap,
                row_rect.center().y - check_side * 0.5,
            ),
            vec2(check_side, check_side),
        );
        let check_resp = todo_checkbox(ui, checkbox_rect, Id::new(("todo_check", item.id)), item.done, theme_name);
        if check_resp.clicked() {
            actions.push(UiAction::SetTodoDone {
                id: item.id,
                done: !item.done,
            });
        }

        let title_rect = Rect::from_min_size(
            pos2(checkbox_rect.right() + gap, row_rect.top()),
            vec2(title_width, row_height),
        );
        let title_resp = ui
            .interact(title_rect, Id::new(("todo_title", item.id)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let metrics = paint_todo_title(
            ui,
            title_rect,
            inner_pad_x,
            inner_pad_y,
            galley,
            required_title_height,
            item.done,
            ink,
        );
        self.snapshot.title_metrics.insert(item.id, metrics);
        if title_resp.clicked() {
            actions.push(UiAction::SetTodoDone {
                id: item.id,
                done: !item.done,
            });
        }

        let delete_rect = Rect::from_min_size(
            pos2(
                row_rect.right() - btn_side,
                row_rect.center().y - btn_side * 0.5,
            ),
            vec2(btn_side, btn_side),
        );
        let edit_rect = Rect::from_min_size(
            pos2(delete_rect.left() - 4.0 - btn_side, delete_rect.top()),
            vec2(btn_side, btn_side),
        );

        let edit_resp = icon_button_in_rect(
            ui,
            edit_rect,
            Id::new(("todo_edit", item.id)),
            ButtonImportance::Low,
            theme_name,
            icons::paint_edit,
        );
        if edit_resp.clicked() {
            actions.push(UiAction::OpenEditTodo { todo: item.clone() });
        }

        let delete_resp = icon_button_in_rect(
            ui,
            delete_rect,
            Id::new(("todo_del", item.id)),
            ButtonImportance::Low,
            theme_name,
            icons::paint_trash,
        );
        if delete_resp.clicked() {
            actions.push(UiAction::DeleteTodo { id: item.id });
        }

        TodoRowRects {
            row_rect,
            handle_rect,
            checkbox_rect,
            title_rect,
            edit_rect,
            delete_rect,
        }
    }

    fn apply_action(&mut self, action: UiAction) {
        match action {
            UiAction::AddTodo { category_id } => {
                let text = self.todo_drafts.get(&category_id).cloned().unwrap_or_default();
                let Some(db) = self.db.as_mut() else {
                    return;
                };
                match db.add_in_category(&text, category_id) {
                    Ok(_) => {
                        self.todo_drafts.insert(category_id, String::new());
                        self.focus_todo_entry = Some(category_id);
                    }
                    Err(err) => self.push_error(err.to_string()),
                }
            }
            UiAction::SetTodoDone { id, done } => {
                let Some(db) = self.db.as_mut() else {
                    return;
                };
                if let Err(err) = db.set_done(id, done) {
                    self.push_error(err.to_string());
                }
            }
            UiAction::DeleteTodo { id } => {
                let Some(db) = self.db.as_mut() else {
                    return;
                };
                if let Err(err) = db.delete(id) {
                    self.push_error(err.to_string());
                }
            }
            UiAction::OpenEditTodo { todo } => {
                self.push_modal(Modal::EditTodo {
                    todo_id: todo.id,
                    draft: todo.title,
                    request_focus: true,
                });
            }
            UiAction::OpenCategoryEmojiPicker {
                category_id,
                selected,
            } => {
                self.push_modal(Modal::EmojiPicker {
                    selected,
                    target: EmojiTarget::Category(category_id),
                });
            }
            UiAction::OpenColorPicker {
                category_id,
                selected,
            } => {
                self.push_modal(Modal::ColorPicker {
                    category_id,
                    selected,
                });
            }
            UiAction::ConfirmDeleteCategory { id, name } => {
                self.push_modal(Modal::Confirm {
                    title: "Delete category?".into(),
                    message: format!(
                        "Delete {name} and all its todos? This cannot be undone."
                    ),
                    kind: ConfirmKind::DeleteCategory(id),
                });
            }
        }
    }

    fn render_modals(&mut self, ctx: &egui::Context, theme_name: &str) {
        if self.modals.is_empty() {
            return;
        }
        let p = palette(theme_name);
        let modal_count = self.modals.len();
        let mut next_action: Option<ModalResult> = None;

        for idx in 0..modal_count {
            let is_top = idx + 1 == modal_count;
            let (modal_id, mut modal) = self.modals[idx].clone();
            let backdrop = if idx == 0 {
                Color32::from_black_alpha(128)
            } else {
                Color32::from_black_alpha(48)
            };

            egui::Modal::new(Id::new(("scrivi_modal", modal_id)))
                .backdrop_color(backdrop)
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    ui.add_enabled_ui(is_top, |ui| {
                        let frame = egui::Frame::new()
                            .fill(p.overlay)
                            .stroke(Stroke::new(1.0_f32, p.border))
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(16)
                            .shadow(egui::epaint::Shadow {
                                offset: [0, 8],
                                blur: 20,
                                spread: 0,
                                color: Color32::from_black_alpha(96),
                            });

                        let frame_out = frame.show(ui, |ui| {
                            let max_modal_width = (ctx.screen_rect().width() - 56.0).max(200.0);
                            ui.set_max_width(max_modal_width);
                            self.render_single_modal(
                                ui,
                                &mut modal,
                                is_top,
                                max_modal_width,
                                theme_name,
                                &mut next_action,
                            )
                        });
                        if is_top {
                            self.snapshot.modal_rect = Some(frame_out.response.rect);
                        }
                    });
                });
            self.modals[idx].1 = modal;
        }

        if let Some(action) = next_action {
            self.apply_modal_result(ctx, action);
        }
    }

    fn render_single_modal(
        &mut self,
        ui: &mut Ui,
        modal: &mut Modal,
        is_top: bool,
        max_modal_width: f32,
        theme_name: &str,
        out: &mut Option<ModalResult>,
    ) {
        let p = palette(theme_name);
        match modal {
            Modal::Error {
                message,
                close_app_on_ok,
            } => {
                ui.set_min_width(340.0_f32.min(max_modal_width));
                ui.set_max_width(440.0_f32.min(max_modal_width));
                if is_top {
                    self.snapshot.modal_title = Some("Error".into());
                    self.snapshot.modal_message = Some(message.clone());
                }
                ui.horizontal(|ui| {
                    let (icon_rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                    icons::paint_error_badge(ui, icon_rect, Color32::from_rgb(225, 85, 95));
                    ui.label(RichText::new("Error").size(16.0).strong().color(p.foreground));
                });
                ui.add_space(8.0);
                ui.add(
                    egui::Label::new(RichText::new(message.as_str()).color(p.foreground)).wrap(),
                );
                ui.add_space(14.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ok = text_button(ui, "OK", ButtonImportance::Medium, theme_name);
                    if is_top {
                        self.snapshot.modal_buttons.push(("OK".into(), ok.rect));
                    }
                    if ok.clicked() {
                        *out = Some(ModalResult::CloseTop {
                            close_app: *close_app_on_ok,
                        });
                    }
                });
            }
            Modal::Confirm {
                title,
                message,
                kind,
            } => {
                ui.set_min_width(340.0_f32.min(max_modal_width));
                ui.set_max_width(440.0_f32.min(max_modal_width));
                if is_top {
                    self.snapshot.modal_title = Some(title.clone());
                    self.snapshot.modal_message = Some(message.clone());
                }
                ui.label(RichText::new(title.as_str()).size(16.0).strong().color(p.foreground));
                ui.add_space(8.0);
                ui.add(
                    egui::Label::new(RichText::new(message.as_str()).color(p.foreground)).wrap(),
                );
                ui.add_space(14.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let yes = text_button(ui, "Yes", ButtonImportance::High, theme_name);
                    ui.add_space(6.0);
                    let no = text_button(ui, "No", ButtonImportance::Medium, theme_name);
                    if is_top {
                        self.snapshot.modal_buttons.push(("Yes".into(), yes.rect));
                        self.snapshot.modal_buttons.push(("No".into(), no.rect));
                    }
                    if yes.clicked() {
                        *out = Some(ModalResult::ConfirmYes(kind.clone()));
                    } else if no.clicked() {
                        *out = Some(ModalResult::ConfirmNo(kind.clone()));
                    }
                });
            }
            Modal::EmojiPicker { selected, target } => {
                if is_top {
                    self.snapshot.modal_title = Some("Category emoji".into());
                }
                ui.label(
                    RichText::new("Category emoji")
                        .size(16.0)
                        .strong()
                        .color(p.foreground),
                );
                ui.add_space(10.0);

                let cols = 6usize;
                for row in CATEGORY_EMOJIS.chunks(cols) {
                    ui.horizontal(|ui| {
                        for &emoji in row {
                            let resp = emoji_button(
                                ui,
                                emoji,
                                selected == emoji,
                                vec2(48.0, 48.0),
                                theme_name,
                            );
                            if is_top {
                                self.snapshot
                                    .modal_emoji_rects
                                    .push((emoji.to_string(), resp.rect));
                            }
                            if resp.clicked() {
                                *out = Some(ModalResult::PickEmoji {
                                    target: target.clone(),
                                    emoji: emoji.to_string(),
                                });
                            }
                        }
                    });
                    ui.add_space(4.0);
                }

                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let cancel = text_button(ui, "Cancel", ButtonImportance::Medium, theme_name);
                    ui.add_space(6.0);
                    let no_icon = text_button(ui, "No icon", ButtonImportance::Medium, theme_name);
                    if is_top {
                        self.snapshot
                            .modal_buttons
                            .push(("Cancel".into(), cancel.rect));
                        self.snapshot
                            .modal_buttons
                            .push(("No icon".into(), no_icon.rect));
                    }
                    if cancel.clicked() {
                        *out = Some(ModalResult::CloseTop { close_app: false });
                    } else if no_icon.clicked() {
                        *out = Some(ModalResult::PickEmoji {
                            target: target.clone(),
                            emoji: String::new(),
                        });
                    }
                });
            }
            Modal::ColorPicker {
                category_id,
                selected,
            } => {
                ui.set_min_width(260.0_f32.min(max_modal_width));
                if is_top {
                    self.snapshot.modal_title = Some("Category color".into());
                }
                ui.label(
                    RichText::new("Category color")
                        .size(16.0)
                        .strong()
                        .color(p.foreground),
                );
                ui.add_space(10.0);

                for row in CATEGORY_COLORS.chunks(2) {
                    ui.horizontal(|ui| {
                        for &name in row {
                            let value = if name == "Default" { "" } else { name };
                            let importance = if selected == value {
                                ButtonImportance::High
                            } else {
                                ButtonImportance::Medium
                            };
                            let resp =
                                text_button_sized(ui, name, vec2(120.0, 34.0), importance, theme_name);
                            if is_top {
                                self.snapshot
                                    .modal_buttons
                                    .push((name.to_string(), resp.rect));
                            }
                            if resp.clicked() {
                                *out = Some(ModalResult::PickColor {
                                    category_id: *category_id,
                                    color: value.to_string(),
                                });
                            }
                        }
                    });
                    ui.add_space(4.0);
                }

                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let cancel = text_button(ui, "Cancel", ButtonImportance::Medium, theme_name);
                    if is_top {
                        self.snapshot
                            .modal_buttons
                            .push(("Cancel".into(), cancel.rect));
                    }
                    if cancel.clicked() {
                        *out = Some(ModalResult::CloseTop { close_app: false });
                    }
                });
            }
            Modal::EditTodo {
                todo_id,
                draft,
                request_focus,
            } => {
                let modal_width = max_modal_width.clamp(240.0, 568.0);
                ui.set_max_width(modal_width);
                ui.set_min_size(vec2(modal_width, 288.0));
                if is_top {
                    self.snapshot.modal_title = Some("Edit todo".into());
                }
                ui.label(
                    RichText::new("Edit todo")
                        .size(16.0)
                        .strong()
                        .color(p.foreground),
                );
                ui.add_space(10.0);

                let focus_now = *request_focus;
                *request_focus = false;
                let (entry_resp, enter_submit) = render_multiline_submit_entry(
                    ui,
                    Id::new(("edit_todo_input", *todo_id)),
                    draft,
                    "What needs doing?",
                    modal_width,
                    6,
                    focus_now,
                );
                if is_top {
                    self.snapshot.modal_input_rect = Some(entry_resp.rect);
                }

                ui.add_space(14.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let save = text_button(ui, "Save", ButtonImportance::High, theme_name);
                    ui.add_space(6.0);
                    let cancel = text_button(ui, "Cancel", ButtonImportance::Medium, theme_name);
                    if is_top {
                        self.snapshot.modal_buttons.push(("Save".into(), save.rect));
                        self.snapshot
                            .modal_buttons
                            .push(("Cancel".into(), cancel.rect));
                    }
                    if (is_top && enter_submit) || save.clicked() {
                        *out = Some(ModalResult::SaveTodoEdit {
                            todo_id: *todo_id,
                            title: draft.clone(),
                        });
                    } else if cancel.clicked() {
                        *out = Some(ModalResult::CloseTop { close_app: false });
                    }
                });
            }
            Modal::Settings {
                startup_checked,
                startup_disabled,
                theme_popup_open,
            } => {
                ui.set_min_width(320.0_f32.min(max_modal_width));
                if is_top {
                    self.snapshot.modal_title = Some("Settings".into());
                }
                ui.label(
                    RichText::new("Settings")
                        .size(16.0)
                        .strong()
                        .color(p.foreground),
                );
                ui.add_space(10.0);
                ui.label(RichText::new("App theme").color(p.foreground));
                ui.add_space(4.0);

                let current_theme = self.current_theme_name().to_string();
                let combo_resp = text_button_sized(
                    ui,
                    &format!("{current_theme}  ▾"),
                    vec2(220.0, 32.0),
                    ButtonImportance::Medium,
                    theme_name,
                );
                if is_top {
                    self.snapshot.modal_theme_combo_rect = Some(combo_resp.rect);
                }
                if combo_resp.clicked() {
                    *theme_popup_open = !*theme_popup_open;
                }

                if *theme_popup_open {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .fill(p.surface)
                        .stroke(Stroke::new(1.0_f32, p.border))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(6)
                        .show(ui, |ui| {
                            for &opt in APP_THEMES {
                                let imp = if opt == current_theme {
                                    ButtonImportance::High
                                } else {
                                    ButtonImportance::Low
                                };
                                let r = text_button_sized(
                                    ui,
                                    opt,
                                    vec2(208.0, 28.0),
                                    imp,
                                    theme_name,
                                );
                                if is_top {
                                    self.snapshot
                                        .modal_theme_items
                                        .push((opt.to_string(), r.rect));
                                }
                                if r.clicked() {
                                    *theme_popup_open = false;
                                    *out = Some(ModalResult::SelectTheme(opt.to_string()));
                                }
                            }
                        });
                }

                if self.platform.supports_startup() {
                    ui.add_space(12.0);
                    let mut checked = *startup_checked;
                    let check_resp = ui.horizontal(|ui| {
                        let (box_rect, _) =
                            ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                        let mut full_rect = box_rect;
                        let resp = ui.add_enabled_ui(!*startup_disabled, |ui| {
                            let label_resp = ui.add(
                                egui::Label::new(
                                    RichText::new("Start Scrivi when I sign in").color(
                                        if *startup_disabled {
                                            p.muted
                                        } else {
                                            p.foreground
                                        },
                                    ),
                                )
                                .sense(Sense::click()),
                            );
                            full_rect = box_rect.union(label_resp.rect);
                            let check_id = Id::new("settings_startup_check");
                            let box_resp =
                                todo_checkbox_enabled(ui, box_rect, check_id, checked, !*startup_disabled, theme_name);
                            let clicked = box_resp.clicked() || label_resp.clicked();
                            if label_resp.hovered() && !*startup_disabled {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }
                            clicked
                        });
                        if resp.inner {
                            checked = !checked;
                        }
                        (full_rect, resp.inner)
                    });
                    if is_top {
                        self.snapshot.modal_startup_check_rect = Some(check_resp.inner.0);
                        self.snapshot.modal_startup_disabled = *startup_disabled;
                        self.snapshot.modal_startup_checked = *startup_checked;
                    }
                    if check_resp.inner.1 && !*startup_disabled {
                        *out = Some(ModalResult::ToggleStartup(checked));
                    }
                }

                ui.add_space(14.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close = text_button(ui, "Close", ButtonImportance::Medium, theme_name);
                    if is_top {
                        self.snapshot
                            .modal_buttons
                            .push(("Close".into(), close.rect));
                    }
                    if close.clicked() {
                        *out = Some(ModalResult::CloseTop { close_app: false });
                    }
                });
            }
            Modal::DownloadingUpdate => {
                ui.set_min_width(300.0_f32.min(max_modal_width));
                if is_top {
                    self.snapshot.modal_title = Some("Downloading update".into());
                }
                ui.label(
                    RichText::new("Downloading update")
                        .size(16.0)
                        .strong()
                        .color(p.foreground),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(RichText::new("Downloading and verifying installer...").color(p.foreground));
                });
            }
        }
    }

    fn apply_modal_result(&mut self, ctx: &egui::Context, res: ModalResult) {
        match res {
            ModalResult::CloseTop { close_app } => {
                self.modals.pop();
                if close_app {
                    self.should_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            ModalResult::ConfirmYes(kind) => {
                self.modals.pop();
                match kind {
                    ConfirmKind::DeleteCategory(id) => {
                        if let Some(db) = self.db.as_mut() {
                            if let Err(err) = db.delete_category(id) {
                                self.push_error(err.to_string());
                            }
                        }
                    }
                    ConfirmKind::InstallUpdate { download_url, .. } => {
                        self.push_modal(Modal::DownloadingUpdate);
                        if let Some(ctrl) = &self.updates {
                            ctrl.start_install(ctx.clone(), download_url);
                        }
                    }
                }
            }
            ModalResult::ConfirmNo(kind) => {
                self.modals.pop();
                if let ConfirmKind::InstallUpdate { version, .. } = kind {
                    if let Some(ctrl) = &mut self.updates {
                        ctrl.declined_version = Some(version);
                    }
                }
            }
            ModalResult::PickEmoji { target, emoji } => {
                self.modals.pop();
                match target {
                    EmojiTarget::NewCategory => {
                        self.new_category_icon = emoji;
                    }
                    EmojiTarget::Category(category_id) => {
                        if let Some(db) = self.db.as_mut() {
                            if let Err(err) = db.set_category_icon(category_id, &emoji) {
                                self.push_error(err.to_string());
                            }
                        }
                    }
                }
            }
            ModalResult::PickColor { category_id, color } => {
                self.modals.pop();
                if let Some(db) = self.db.as_mut() {
                    if let Err(err) = db.set_category_color(category_id, &color) {
                        self.push_error(err.to_string());
                    }
                }
            }
            ModalResult::SaveTodoEdit { todo_id, title } => {
                if let Some(db) = self.db.as_mut() {
                    match db.set_title(todo_id, &title) {
                        Ok(()) => {
                            self.modals.pop();
                        }
                        Err(err) => {
                            self.push_error(err.to_string());
                        }
                    }
                }
            }
            ModalResult::SelectTheme(name) => {
                if let Some(db) = self.db.as_mut() {
                    match db.set_app_theme(&name) {
                        Ok(()) => {
                            apply_egui_theme(ctx, &name);
                            self.theme_applied = name;
                        }
                        Err(err) => {
                            self.push_error(err.to_string());
                        }
                    }
                }
            }
            ModalResult::ToggleStartup(next) => match self.platform.set_startup(next) {
                Ok(()) => {
                    for (_, m) in self.modals.iter_mut().rev() {
                        if let Modal::Settings {
                            startup_checked, ..
                        } = m
                        {
                            *startup_checked = next;
                            break;
                        }
                    }
                }
                Err(err) => {
                    self.push_error(err);
                }
            },
        }
    }
}

impl eframe::App for ScriviApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(ctrl) = &self.updates {
            ctrl.cancel();
        }
        if let Some(db) = self.db.as_mut() {
            if let Err(err) = db.close() {
                eprintln!("close todo database: {err}");
            }
        }
    }
}

enum UiAction {
    AddTodo { category_id: i64 },
    SetTodoDone { id: i64, done: bool },
    DeleteTodo { id: i64 },
    OpenEditTodo { todo: Todo },
    OpenCategoryEmojiPicker { category_id: i64, selected: String },
    OpenColorPicker { category_id: i64, selected: String },
    ConfirmDeleteCategory { id: i64, name: String },
}

enum ModalResult {
    CloseTop { close_app: bool },
    ConfirmYes(ConfirmKind),
    ConfirmNo(ConfirmKind),
    PickEmoji { target: EmojiTarget, emoji: String },
    PickColor { category_id: i64, color: String },
    SaveTodoEdit { todo_id: i64, title: String },
    SelectTheme(String),
    ToggleStartup(bool),
}

pub fn render_multiline_submit_entry(
    ui: &mut Ui,
    id: Id,
    text: &mut String,
    hint: &str,
    width: f32,
    desired_rows: usize,
    focus_now: bool,
) -> (Response, bool) {
    let shift_enter = KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter);
    let margin = egui::Margin::symmetric(4, 2);
    let output = TextEdit::multiline(text)
        .id(id)
        .hint_text(hint)
        .margin(margin)
        .desired_width((width - margin.sum().x).max(0.0))
        .desired_rows(desired_rows)
        .return_key(shift_enter)
        .show(ui);

    if focus_now {
        output.response.request_focus();
    }

    let enter_submit = output.response.has_focus()
        && ui.input_mut(|i| {
            let mut found = false;
            i.events.retain(|ev| {
                if let egui::Event::Key {
                    key: Key::Enter,
                    pressed: true,
                    modifiers,
                    ..
                } = ev
                {
                    if !modifiers.shift
                        && !modifiers.alt
                        && !modifiers.ctrl
                        && !modifiers.command
                    {
                        found = true;
                        return false;
                    }
                }
                true
            });
            found
        });

    (output.response, enter_submit)
}

pub fn layout_todo_galley(
    ui: &Ui,
    title: &str,
    wrap_width: f32,
    font_id: FontId,
    color: Color32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::single_section(
        title.to_string(),
        TextFormat {
            font_id,
            color,
            ..Default::default()
        },
    );
    job.wrap = TextWrapping {
        max_width: wrap_width.max(1.0),
        break_anywhere: false,
        overflow_character: None,
        ..Default::default()
    };
    ui.fonts(|f| f.layout_job(job))
}

#[allow(clippy::too_many_arguments)]
pub fn paint_todo_title(
    ui: &Ui,
    title_rect: Rect,
    inner_pad_x: f32,
    inner_pad_y: f32,
    galley: std::sync::Arc<egui::Galley>,
    required_height: f32,
    done: bool,
    ink: Color32,
) -> TodoTitleMetrics {
    let start_y = title_rect.top()
        + ((title_rect.height() - galley.size().y) * 0.5).max(inner_pad_y);
    let text_pos = pos2(title_rect.left() + inner_pad_x, start_y);

    let mut lines = Vec::with_capacity(galley.rows.len());
    for row in &galley.rows {
        let line_text: String = row.glyphs.iter().map(|g| g.chr).collect();
        let non_ws: Vec<_> = row
            .glyphs
            .iter()
            .filter(|g| !g.chr.is_whitespace())
            .collect();
        let row_rect_abs = row.rect.translate(text_pos.to_vec2());
        let strike_segment = if done && !non_ws.is_empty() {
            let x_min = text_pos.x + non_ws.first().map(|g| g.pos.x).unwrap_or(row.rect.min.x);
            let x_max = text_pos.x + non_ws.last().map(|g| g.max_x()).unwrap_or(row.rect.max.x);
            let y = text_pos.y + row.rect.center().y;
            Some((pos2(x_min, y), pos2(x_max, y)))
        } else {
            None
        };
        lines.push(RenderedTitleLine {
            text: line_text,
            text_rect: row_rect_abs,
            strike_segment,
        });
    }

    let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(title_rect));
    painter.galley(text_pos, galley, ink);

    if done {
        for line in &lines {
            if let Some((a, b)) = line.strike_segment {
                painter.line_segment([a, b], Stroke::new(1.2_f32, ink));
            }
        }
    }

    TodoTitleMetrics {
        rect: title_rect,
        required_height,
        lines,
    }
}

fn button_colors(
    importance: ButtonImportance,
    hovered: bool,
    disabled: bool,
    theme_name: &str,
) -> (Color32, Color32, Stroke) {
    let p = palette(theme_name);
    if disabled {
        return (p.button, p.muted, Stroke::new(1.0_f32, p.border));
    }
    match importance {
        ButtonImportance::High => {
            let bg = if hovered {
                Color32::from_rgb(
                    p.primary.r().saturating_add(12),
                    p.primary.g().saturating_add(12),
                    p.primary.b().saturating_add(12),
                )
            } else {
                p.primary
            };
            (bg, p.on_primary, Stroke::NONE)
        }
        ButtonImportance::Medium => {
            let bg = if hovered { p.surface } else { p.button };
            let stroke = if hovered {
                Stroke::new(1.0_f32, p.primary)
            } else {
                Stroke::new(1.0_f32, p.border)
            };
            (bg, p.foreground, stroke)
        }
        ButtonImportance::Low => {
            let bg = if hovered {
                Color32::from_rgba_unmultiplied(p.primary.r(), p.primary.g(), p.primary.b(), 36)
            } else {
                Color32::TRANSPARENT
            };
            let fg = if hovered { p.foreground } else { p.muted };
            (bg, fg, Stroke::NONE)
        }
    }
}

pub fn pointer_button(
    ui: &mut Ui,
    text: &str,
    enabled: bool,
    theme_name: &str,
) -> Response {
    ui.add_enabled_ui(enabled, |ui| {
        text_button_impl(ui, text, None, ButtonImportance::Medium, enabled, theme_name)
    })
    .inner
}

fn text_button(
    ui: &mut Ui,
    text: &str,
    importance: ButtonImportance,
    theme_name: &str,
) -> Response {
    text_button_impl(ui, text, None, importance, true, theme_name)
}

fn text_button_sized(
    ui: &mut Ui,
    text: &str,
    size: Vec2,
    importance: ButtonImportance,
    theme_name: &str,
) -> Response {
    text_button_impl(ui, text, Some(size), importance, true, theme_name)
}

fn text_button_impl(
    ui: &mut Ui,
    text: &str,
    explicit_size: Option<Vec2>,
    importance: ButtonImportance,
    enabled: bool,
    theme_name: &str,
) -> Response {
    let font_id = FontId::proportional(14.0);
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font_id, Color32::WHITE));
    let desired = explicit_size.unwrap_or_else(|| {
        vec2((galley.size().x + 24.0).max(64.0), (galley.size().y + 14.0).max(32.0))
    });
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(desired, sense);
    let resp = if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    };
    let (bg, fg, stroke) = button_colors(importance, resp.hovered(), !enabled, theme_name);
    ui.painter()
        .rect(rect, CornerRadius::same(6), bg, stroke, egui::StrokeKind::Middle);
    let colored_galley = ui.fonts(|f| {
        f.layout_no_wrap(text.to_string(), FontId::proportional(14.0), fg)
    });
    let text_pos = rect.center() - colored_galley.size() * 0.5;
    ui.painter().galley(text_pos, colored_galley, fg);
    resp
}

fn icon_text_button(
    ui: &mut Ui,
    text: &str,
    importance: ButtonImportance,
    theme_name: &str,
    paint_icon: impl FnOnce(&Ui, Rect, Color32),
) -> Response {
    icon_text_button_impl(ui, text, None, importance, theme_name, paint_icon)
}

fn icon_text_button_sized(
    ui: &mut Ui,
    text: &str,
    size: Vec2,
    importance: ButtonImportance,
    theme_name: &str,
    paint_icon: impl FnOnce(&Ui, Rect, Color32),
) -> Response {
    icon_text_button_impl(ui, text, Some(size), importance, theme_name, paint_icon)
}

fn icon_text_button_width(ui: &Ui, text: &str) -> f32 {
    let font_id = FontId::proportional(14.0);
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font_id, Color32::WHITE));
    icon_text_button_width_for_galley(&galley)
}

fn icon_text_button_width_for_galley(galley: &egui::Galley) -> f32 {
    let icon_side = 16.0;
    let gap = 6.0;
    (icon_side + gap + galley.size().x + 22.0).max(74.0)
}

fn icon_text_button_impl(
    ui: &mut Ui,
    text: &str,
    explicit_size: Option<Vec2>,
    importance: ButtonImportance,
    theme_name: &str,
    paint_icon: impl FnOnce(&Ui, Rect, Color32),
) -> Response {
    let font_id = FontId::proportional(14.0);
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font_id, Color32::WHITE));
    let icon_side = 16.0;
    let gap = 6.0;
    let desired = explicit_size.unwrap_or_else(|| {
        vec2(
            icon_text_button_width_for_galley(&galley),
            (galley.size().y + 14.0).max(32.0),
        )
    });
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let (bg, fg, stroke) = button_colors(importance, resp.hovered(), false, theme_name);
    ui.painter()
        .rect(rect, CornerRadius::same(6), bg, stroke, egui::StrokeKind::Middle);

    let total_w = icon_side + gap + galley.size().x;
    let start_x = rect.center().x - total_w * 0.5;
    let icon_rect = Rect::from_min_size(
        pos2(start_x, rect.center().y - icon_side * 0.5),
        vec2(icon_side, icon_side),
    );
    paint_icon(ui, icon_rect, fg);

    let colored_galley = ui.fonts(|f| {
        f.layout_no_wrap(text.to_string(), FontId::proportional(14.0), fg)
    });
    let text_pos = pos2(
        icon_rect.right() + gap,
        rect.center().y - colored_galley.size().y * 0.5,
    );
    ui.painter().galley(text_pos, colored_galley, fg);
    resp
}

fn icon_button_in_rect(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    importance: ButtonImportance,
    theme_name: &str,
    paint_icon: impl FnOnce(&Ui, Rect, Color32),
) -> Response {
    let resp = ui
        .interact(rect, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let (bg, fg, stroke) = button_colors(importance, resp.hovered(), false, theme_name);
    ui.painter()
        .rect(rect, CornerRadius::same(6), bg, stroke, egui::StrokeKind::Middle);
    let icon_side = rect.width().min(rect.height()) * 0.68;
    let icon_rect = Rect::from_center_size(rect.center(), vec2(icon_side, icon_side));
    paint_icon(ui, icon_rect, fg);
    resp
}

fn emoji_button(
    ui: &mut Ui,
    emoji: &str,
    selected: bool,
    size: Vec2,
    theme_name: &str,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    paint_emoji_button_visuals(ui, rect, resp.hovered(), emoji, selected, theme_name);
    resp
}

fn emoji_button_in_rect(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    emoji: &str,
    selected: bool,
    theme_name: &str,
) -> Response {
    let resp = ui
        .interact(rect, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    paint_emoji_button_visuals(ui, rect, resp.hovered(), emoji, selected, theme_name);
    resp
}

fn paint_emoji_button_visuals(
    ui: &Ui,
    rect: Rect,
    hovered: bool,
    emoji: &str,
    selected: bool,
    theme_name: &str,
) {
    let p = palette(theme_name);
    let importance = if selected {
        ButtonImportance::High
    } else {
        ButtonImportance::Medium
    };
    let (bg, fg, stroke) = button_colors(importance, hovered, false, theme_name);
    ui.painter()
        .rect(rect, CornerRadius::same(6), bg, stroke, egui::StrokeKind::Middle);
    let ink = if selected { p.on_primary } else { fg };
    icons::paint_emoji_or_plus(ui, rect, emoji, ink, 0.70);
}

pub fn todo_checkbox(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    checked: bool,
    theme_name: &str,
) -> Response {
    todo_checkbox_enabled(ui, rect, id, checked, true, theme_name)
}

pub fn todo_checkbox_enabled(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    checked: bool,
    enabled: bool,
    theme_name: &str,
) -> Response {
    let p = palette(theme_name);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let resp = ui.interact(rect, id, sense);
    let resp = if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    };
    let (bg, stroke) = if checked {
        (p.primary, Stroke::NONE)
    } else {
        let border_color = if resp.hovered() && enabled {
            p.primary
        } else {
            p.border
        };
        (p.input, Stroke::new(1.5_f32, border_color))
    };
    ui.painter()
        .rect(rect, CornerRadius::same(4), bg, stroke, egui::StrokeKind::Middle);
    if checked {
        icons::paint_check(ui, rect, p.on_primary);
    }
    resp
}
