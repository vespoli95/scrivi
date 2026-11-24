use std::cell::RefCell;
use std::rc::Rc;

use egui::{pos2, vec2, CursorIcon, Event, FontId, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, Vec2};
use scrivi::layout::{
    compute_category_grid, compute_heading_layout, compute_pinned_category_stack, GRID_GAP,
    PINNED_WINDOW_HEIGHT, PINNED_WINDOW_WIDTH,
};
use scrivi::options::WindowPlatform;
use scrivi::store::{MemoryDb, CATEGORY_EMOJIS};
use scrivi::theme::{palette, DEFAULT_THEME};
use scrivi::ui::{
    layout_todo_galley, paint_todo_title, pointer_button, render_multiline_submit_entry,
    todo_checkbox, ScriviApp,
};

struct TestHarness {
    ctx: egui::Context,
    size: Vec2,
    time: f64,
}

impl TestHarness {
    fn new(width: f32, height: f32) -> Self {
        Self {
            ctx: egui::Context::default(),
            size: vec2(width, height),
            time: 0.0,
        }
    }

    fn step(&mut self, app: &mut ScriviApp, events: Vec<Event>, modifiers: Modifiers) {
        self.time += 1.0 / 60.0;
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.time),
            modifiers,
            events,
            ..Default::default()
        };
        let _ = self.ctx.run(raw, |ctx| app.render(ctx));
    }

    fn pump(&mut self, app: &mut ScriviApp) {
        self.step(app, vec![], Modifiers::NONE);
        self.step(app, vec![], Modifiers::NONE);
    }

    fn resize(&mut self, app: &mut ScriviApp, width: f32, height: f32) {
        self.size = vec2(width, height);
        self.pump(app);
    }

    fn hover(&mut self, app: &mut ScriviApp, pos: Pos2) {
        self.step(app, vec![Event::PointerMoved(pos)], Modifiers::NONE);
    }

    fn click(&mut self, app: &mut ScriviApp, pos: Pos2) {
        self.step(
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
            Modifiers::NONE,
        );
        self.step(
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
            Modifiers::NONE,
        );
        self.pump(app);
    }

    fn replace_text(&mut self, app: &mut ScriviApp, rect: Rect, text: &str) {
        self.click(app, rect.center());
        self.step(
            app,
            vec![
                Event::Key {
                    key: Key::A,
                    physical_key: Some(Key::A),
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::Key {
                    key: Key::Backspace,
                    physical_key: Some(Key::Backspace),
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
            ],
            Modifiers::NONE,
        );
        if !text.is_empty() {
            let mut events = Vec::new();
            for (i, chunk) in text.split('\n').enumerate() {
                if i > 0 {
                    events.push(Event::Key {
                        key: Key::Enter,
                        physical_key: Some(Key::Enter),
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::SHIFT,
                    });
                }
                if !chunk.is_empty() {
                    events.push(Event::Text(chunk.to_string()));
                }
            }
            self.step(app, events, Modifiers::NONE);
        }
        self.step(app, vec![], Modifiers::NONE);
    }

    fn press_enter(&mut self, app: &mut ScriviApp) {
        self.step(
            app,
            vec![
                Event::Key {
                    key: Key::Enter,
                    physical_key: Some(Key::Enter),
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
                Event::Key {
                    key: Key::Enter,
                    physical_key: Some(Key::Enter),
                    pressed: false,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
            ],
            Modifiers::NONE,
        );
        self.step(app, vec![], Modifiers::NONE);
    }

    fn drag(&mut self, app: &mut ScriviApp, from: Pos2, to: Pos2) {
        self.step(
            app,
            vec![
                Event::PointerMoved(from),
                Event::PointerButton {
                    pos: from,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
            Modifiers::NONE,
        );
        self.step(
            app,
            vec![Event::PointerMoved(from + vec2(8.0, 2.0))],
            Modifiers::NONE,
        );
        self.step(app, vec![Event::PointerMoved(to)], Modifiers::NONE);
        self.step(
            app,
            vec![
                Event::PointerMoved(to),
                Event::PointerButton {
                    pos: to,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
            Modifiers::NONE,
        );
        self.step(app, vec![], Modifiers::NONE);
    }

    fn scroll(&mut self, app: &mut ScriviApp, pos: Pos2, delta: Vec2) {
        self.step(
            app,
            vec![
                Event::PointerMoved(pos),
                Event::MouseWheel {
                    unit: MouseWheelUnit::Point,
                    delta,
                    modifiers: Modifiers::NONE,
                },
            ],
            Modifiers::NONE,
        );
        self.step(app, vec![], Modifiers::NONE);
    }
}

#[derive(Default)]
struct MockPlatformState {
    pinned: bool,
    startup: bool,
    fail_pin: bool,
    fail_startup: bool,
    fail_read: bool,
}

struct MockPlatform {
    state: Rc<RefCell<MockPlatformState>>,
}

impl WindowPlatform for MockPlatform {
    fn supports_pin(&self) -> bool {
        true
    }

    fn set_pinned(&mut self, _ctx: &egui::Context, pinned: bool) -> Result<(), String> {
        let mut st = self.state.borrow_mut();
        if st.fail_pin {
            return Err("pin failed".into());
        }
        st.pinned = pinned;
        Ok(())
    }

    fn supports_startup(&self) -> bool {
        true
    }

    fn startup_enabled(&mut self) -> Result<bool, String> {
        let st = self.state.borrow();
        if st.fail_read {
            return Err("read failed".into());
        }
        Ok(st.startup)
    }

    fn set_startup(&mut self, enabled: bool) -> Result<(), String> {
        let mut st = self.state.borrow_mut();
        if st.fail_startup {
            return Err("startup failed".into());
        }
        st.startup = enabled;
        Ok(())
    }
}

fn new_app(platform: Box<dyn WindowPlatform>) -> ScriviApp {
    let db = MemoryDb::open_in_memory().unwrap();
    ScriviApp::new(db, platform, None)
}

fn pump_unpinned(h: &mut TestHarness, app: &mut ScriviApp) {
    h.pump(app);
    if app.is_pinned() {
        let pin_rect = app.snapshot().pin_rect.unwrap();
        h.click(app, pin_rect.center());
    }
}

fn find_modal_button(app: &ScriviApp, label: &str) -> Rect {
    app.snapshot()
        .modal_buttons
        .iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("missing modal button {label:?}: {:?}", app.snapshot().modal_buttons))
        .1
}

fn find_modal_emoji(app: &ScriviApp, emoji: &str) -> Rect {
    app.snapshot()
        .modal_emoji_rects
        .iter()
        .find(|(e, _)| e == emoji)
        .unwrap_or_else(|| panic!("missing modal emoji {emoji:?}"))
        .1
}

#[test]
fn test_multiline_entry_shift_enter_adds_newline() {
    let ctx = egui::Context::default();
    let mut text = "first".to_string();
    let id = egui::Id::new("test_multiline_entry");
    let mut time = 0.0;

    let mut run_frame =
        |text: &mut String, events: Vec<Event>, modifiers: Modifiers, focus: bool| -> Option<String> {
            time += 1.0 / 60.0;
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 200.0))),
                time: Some(time),
                modifiers,
                events,
                ..Default::default()
            };
            let mut submitted = None;
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (_, enter_submit) = render_multiline_submit_entry(
                        ui,
                        id,
                        text,
                        "hint",
                        300.0,
                        2,
                        focus,
                    );
                    if enter_submit {
                        submitted = Some(text.clone());
                    }
                });
            });
            submitted
        };

    run_frame(&mut text, vec![], Modifiers::NONE, true);
    run_frame(&mut text, vec![], Modifiers::NONE, false);

    // Shift+Enter + 's'
    let submitted = run_frame(
        &mut text,
        vec![
            Event::Key {
                key: Key::Enter,
                physical_key: Some(Key::Enter),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
            Event::Text("s".into()),
        ],
        Modifiers::SHIFT,
        false,
    );
    assert_eq!(text, "first\ns", "Shift+Enter did not insert a newline");
    assert!(submitted.is_none());

    // Plain Enter submits without adding newline
    let submitted = run_frame(
        &mut text,
        vec![Event::Key {
            key: Key::Enter,
            physical_key: Some(Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        Modifiers::NONE,
        false,
    );
    assert_eq!(submitted.as_deref(), Some("first\ns"));
}

#[test]
fn test_category_ui() {
    let mut app = new_app(Box::new(MockPlatform {
        state: Rc::new(RefCell::new(MockPlatformState::default())),
    }));
    let mut h = TestHarness::new(760.0, 600.0);
    pump_unpinned(&mut h, &mut app);

    assert!(app.db().unwrap().list_categories().is_empty());
    assert!(app.snapshot().empty_categories_rect.is_some());

    // Pick emoji + create "Life"
    let cat_input = app.snapshot().new_cat_input_rect.unwrap();
    h.replace_text(&mut app, cat_input, "Life");
    let cat_emoji = app.snapshot().new_cat_emoji_rect.unwrap();
    h.click(&mut app, cat_emoji.center());
    let home_emoji_rect = find_modal_emoji(&app, "🏠");
    assert_eq!(home_emoji_rect.size(), vec2(48.0, 48.0));
    assert!(CATEGORY_EMOJIS.len() > 24);
    h.click(&mut app, home_emoji_rect.center());

    let add_cat_rect = app.snapshot().new_cat_add_rect.unwrap();
    h.click(&mut app, add_cat_rect.center());

    let cats = app.db().unwrap().list_categories();
    assert_eq!(cats.len(), 1);
    assert_eq!(cats[0].icon, "🏠");
    let life_id = cats[0].id;

    // Draft survives adding another category via Enter
    let life_panel = app.snapshot().category_panels[&life_id].clone();
    h.replace_text(&mut app, life_panel.todo_input_rect, "Keep this draft");
    let cat_input = app.snapshot().new_cat_input_rect.unwrap();
    h.replace_text(&mut app, cat_input, "Work");
    h.press_enter(&mut app);

    let cats = app.db().unwrap().list_categories();
    assert_eq!(cats.len(), 2);
    assert_eq!(app.new_category_name(), "");
    assert_eq!(app.todo_draft(life_id), "Keep this draft");
    let work_id = cats[1].id;

    // Panel stretches to fill available window space (> 340 x 240)
    let life_panel = app.snapshot().category_panels[&life_id].clone();
    assert!(life_panel.card_rect.width() > 340.0);
    assert!(life_panel.card_rect.height() > 240.0);

    // Change emoji and keep draft
    h.replace_text(&mut app, life_panel.todo_input_rect, "Another draft");
    let life_panel = app.snapshot().category_panels[&life_id].clone();
    h.click(&mut app, life_panel.emoji_rect.center());
    let star_rect = find_modal_emoji(&app, "⭐");
    h.click(&mut app, star_rect.center());
    assert_eq!(app.db().unwrap().category(life_id).unwrap().icon, "⭐");
    assert_eq!(app.todo_draft(life_id), "Another draft");

    // Centered heading stays within available rect
    let life_panel = app.snapshot().category_panels[&life_id].clone();
    assert!(life_panel.label_rect.right() <= life_panel.heading_avail_rect.right() + 0.01);

    // Cancel emoji picker leaves icon unchanged
    h.click(&mut app, life_panel.emoji_rect.center());
    let cancel_rect = find_modal_button(&app, "Cancel");
    h.click(&mut app, cancel_rect.center());
    assert_eq!(app.db().unwrap().category(life_id).unwrap().icon, "⭐");

    // Remove icon via "No icon"
    let life_panel = app.snapshot().category_panels[&life_id].clone();
    h.click(&mut app, life_panel.emoji_rect.center());
    let no_icon_rect = find_modal_button(&app, "No icon");
    h.click(&mut app, no_icon_rect.center());
    assert_eq!(app.db().unwrap().category(life_id).unwrap().icon, "");

    // Re-add icon using '+'
    let life_panel = app.snapshot().category_panels[&life_id].clone();
    h.click(&mut app, life_panel.emoji_rect.center());
    let sprout_rect = find_modal_emoji(&app, "🌱");
    h.click(&mut app, sprout_rect.center());
    assert_eq!(app.db().unwrap().category(life_id).unwrap().icon, "🌱");

    // Category color picker: Rose then Default
    let life_panel = app.snapshot().category_panels[&life_id].clone();
    h.click(&mut app, life_panel.color_rect.center());
    let rose_rect = find_modal_button(&app, "Rose");
    h.click(&mut app, rose_rect.center());
    assert_eq!(app.db().unwrap().category(life_id).unwrap().color, "Rose");

    let life_panel = app.snapshot().category_panels[&life_id].clone();
    h.click(&mut app, life_panel.color_rect.center());
    let default_rect = find_modal_button(&app, "Default");
    h.click(&mut app, default_rect.center());
    assert_eq!(app.db().unwrap().category(life_id).unwrap().color, "");

    // Add todo in Work
    let work_panel = app.snapshot().category_panels[&work_id].clone();
    h.replace_text(&mut app, work_panel.todo_input_rect, "Finish report");
    let work_panel = app.snapshot().category_panels[&work_id].clone();
    h.click(&mut app, work_panel.todo_add_rect.center());

    let items = app.db().unwrap().list();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Finish report");
    assert_eq!(items[0].category_id, work_id);
    assert_eq!(app.todo_draft(work_id), "");
    let todo_id = items[0].id;

    // Complete by clicking title
    let work_panel = app.snapshot().category_panels[&work_id].clone();
    let title_rect = work_panel.todo_rows[0].1.title_rect;
    h.click(&mut app, title_rect.center());
    assert!(app.db().unwrap().todo(todo_id).unwrap().done);

    // Edit dialog size >= 600x320, blank save rejected, trigger failure preserves todo & draft
    let work_panel = app.snapshot().category_panels[&work_id].clone();
    let edit_rect = work_panel.todo_rows[0].1.edit_rect;
    h.click(&mut app, edit_rect.center());
    let modal_rect = app.snapshot().modal_rect.unwrap();
    assert!(
        modal_rect.width() >= 600.0 && modal_rect.height() >= 320.0,
        "edit dialog too small: {modal_rect:?}"
    );
    let edit_input = app.snapshot().modal_input_rect.unwrap();
    h.replace_text(&mut app, edit_input, " ");
    let save_rect = find_modal_button(&app, "Save");
    h.click(&mut app, save_rect.center());
    assert_eq!(app.db().unwrap().todo(todo_id).unwrap().title, "Finish report");
    assert_eq!(app.active_modal_title(), Some("Error"));
    let ok_rect = find_modal_button(&app, "OK");
    h.click(&mut app, ok_rect.center());

    let edit_input = app.snapshot().modal_input_rect.unwrap();
    h.replace_text(&mut app, edit_input, "Finish presentation");
    app.db()
        .unwrap()
        .exec_sql_for_test(
            "CREATE TRIGGER prevent_edit BEFORE UPDATE OF title ON todos
             BEGIN SELECT RAISE(ABORT, 'update blocked'); END;",
        )
        .unwrap();
    let save_rect = find_modal_button(&app, "Save");
    h.click(&mut app, save_rect.center());
    assert_eq!(app.db().unwrap().todo(todo_id).unwrap().title, "Finish report");
    assert_eq!(app.active_modal_title(), Some("Error"));
    let ok_rect = find_modal_button(&app, "OK");
    h.click(&mut app, ok_rect.center());
    app.db()
        .unwrap()
        .exec_sql_for_test("DROP TRIGGER prevent_edit;")
        .unwrap();

    // Enter saves edit preserving completion
    let edit_input = app.snapshot().modal_input_rect.unwrap();
    h.click(&mut app, edit_input.center());
    h.press_enter(&mut app);
    let saved = app.db().unwrap().todo(todo_id).unwrap();
    assert_eq!(saved.title, "Finish presentation");
    assert!(saved.done);

    // Cancel edit discards draft
    let work_panel = app.snapshot().category_panels[&work_id].clone();
    h.click(&mut app, work_panel.todo_rows[0].1.edit_rect.center());
    let edit_input = app.snapshot().modal_input_rect.unwrap();
    h.replace_text(&mut app, edit_input, "Canceled edit");
    let cancel_rect = find_modal_button(&app, "Cancel");
    h.click(&mut app, cancel_rect.center());
    assert_eq!(
        app.db().unwrap().todo(todo_id).unwrap().title,
        "Finish presentation"
    );

    // Delete Work: confirm No keeps it, confirm Yes deletes it and keeps Life's draft
    let work_panel = app.snapshot().category_panels[&work_id].clone();
    h.click(&mut app, work_panel.delete_rect.center());
    assert!(app
        .snapshot()
        .modal_message
        .as_deref()
        .unwrap()
        .contains("all its todos"));
    let no_rect = find_modal_button(&app, "No");
    h.click(&mut app, no_rect.center());
    assert_eq!(app.db().unwrap().list_categories().len(), 2);
    assert_eq!(app.db().unwrap().list().len(), 1);

    let work_panel = app.snapshot().category_panels[&work_id].clone();
    h.click(&mut app, work_panel.delete_rect.center());
    let yes_rect = find_modal_button(&app, "Yes");
    h.click(&mut app, yes_rect.center());
    assert_eq!(app.db().unwrap().list_categories().len(), 1);
    assert!(app.db().unwrap().list().is_empty());
    assert_eq!(app.todo_draft(life_id), "Another draft");
}

#[test]
fn test_window_options() {
    let state = Rc::new(RefCell::new(MockPlatformState {
        startup: true,
        ..Default::default()
    }));
    let mut app = new_app(Box::new(MockPlatform {
        state: state.clone(),
    }));
    let mut h = TestHarness::new(760.0, 600.0);
    h.pump(&mut app);

    // Starts pinned on platforms that support pinning
    assert!(app.is_pinned());
    assert!(state.borrow().pinned);

    // Failed unpin keeps it pinned and surfaces an error
    state.borrow_mut().fail_pin = true;
    let pin_rect = app.snapshot().pin_rect.unwrap();
    h.click(&mut app, pin_rect.center());
    assert!(app.is_pinned());
    assert!(state.borrow().pinned);
    assert_eq!(app.active_modal_title(), Some("Error"));
    let ok_rect = find_modal_button(&app, "OK");
    h.click(&mut app, ok_rect.center());

    // Unpin succeeds
    state.borrow_mut().fail_pin = false;
    let pin_rect = app.snapshot().pin_rect.unwrap();
    h.click(&mut app, pin_rect.center());
    assert!(!app.is_pinned());
    assert!(!state.borrow().pinned);

    // Pin -> error on unpin -> Unpin
    let pin_rect = app.snapshot().pin_rect.unwrap();
    h.click(&mut app, pin_rect.center());
    assert!(app.is_pinned());
    assert!(state.borrow().pinned);

    state.borrow_mut().fail_pin = true;
    let pin_rect = app.snapshot().pin_rect.unwrap();
    h.click(&mut app, pin_rect.center());
    assert!(app.is_pinned());
    assert!(state.borrow().pinned);
    assert_eq!(app.active_modal_title(), Some("Error"));
    let ok_rect = find_modal_button(&app, "OK");
    h.click(&mut app, ok_rect.center());

    state.borrow_mut().fail_pin = false;
    let pin_rect = app.snapshot().pin_rect.unwrap();
    h.click(&mut app, pin_rect.center());
    assert!(!app.is_pinned());
    assert!(!state.borrow().pinned);

    // Open Settings -> change theme to Ocean -> fail Rose -> revert
    let settings_rect = app.snapshot().settings_rect.unwrap();
    h.click(&mut app, settings_rect.center());
    assert_eq!(app.active_modal_title(), Some("Settings"));

    let combo_rect = app.snapshot().modal_theme_combo_rect.unwrap();
    h.click(&mut app, combo_rect.center());
    let ocean_rect = app
        .snapshot()
        .modal_theme_items
        .iter()
        .find(|(t, _)| t == "Ocean")
        .unwrap()
        .1;
    h.click(&mut app, ocean_rect.center());
    assert_eq!(app.db().unwrap().app_theme(), "Ocean");

    app.db()
        .unwrap()
        .exec_sql_for_test(
            "CREATE TRIGGER prevent_theme_update BEFORE UPDATE ON settings
             BEGIN SELECT RAISE(ABORT, 'theme blocked'); END;",
        )
        .unwrap();
    let combo_rect = app.snapshot().modal_theme_combo_rect.unwrap();
    h.click(&mut app, combo_rect.center());
    let rose_rect = app
        .snapshot()
        .modal_theme_items
        .iter()
        .find(|(t, _)| t == "Rose")
        .unwrap()
        .1;
    h.click(&mut app, rose_rect.center());
    assert_eq!(app.db().unwrap().app_theme(), "Ocean");
    assert_eq!(app.active_modal_title(), Some("Error"));
    let ok_rect = find_modal_button(&app, "OK");
    h.click(&mut app, ok_rect.center());

    // Startup checkbox
    assert!(app.snapshot().modal_startup_checked);
    let check_rect = app.snapshot().modal_startup_check_rect.unwrap();
    h.click(&mut app, check_rect.center());
    assert!(!state.borrow().startup);
    assert!(!app.snapshot().modal_startup_checked);

    state.borrow_mut().fail_startup = true;
    let check_rect = app.snapshot().modal_startup_check_rect.unwrap();
    h.click(&mut app, check_rect.center());
    assert!(!state.borrow().startup);
    assert!(!app.snapshot().modal_startup_checked);
    assert_eq!(app.active_modal_title(), Some("Error"));
    let ok_rect = find_modal_button(&app, "OK");
    h.click(&mut app, ok_rect.center());

    state.borrow_mut().fail_startup = false;
    let check_rect = app.snapshot().modal_startup_check_rect.unwrap();
    h.click(&mut app, check_rect.center());
    assert!(state.borrow().startup);
    assert!(app.snapshot().modal_startup_checked);

    let close_rect = find_modal_button(&app, "Close");
    h.click(&mut app, close_rect.center());

    // Startup read failure disables control & still allows pinning afterwards
    state.borrow_mut().fail_read = true;
    let settings_rect = app.snapshot().settings_rect.unwrap();
    h.click(&mut app, settings_rect.center());
    assert_eq!(app.active_modal_title(), Some("Error"));
    assert_eq!(app.snapshot().modal_message.as_deref(), Some("read failed"));
    let ok_rect = find_modal_button(&app, "OK");
    h.click(&mut app, ok_rect.center());

    assert!(app.snapshot().modal_startup_disabled);
    let close_rect = find_modal_button(&app, "Close");
    h.click(&mut app, close_rect.center());

    let pin_rect = app.snapshot().pin_rect.unwrap();
    h.click(&mut app, pin_rect.center());
    assert!(app.is_pinned());
}

#[test]
fn test_drag_categories_and_todos() {
    let mut db = MemoryDb::open_in_memory().unwrap();
    let life_id = db.add_category("Life").unwrap();
    let work_id = db.add_category("Work").unwrap();
    let home_id = db.add_category("Home").unwrap();
    for title in ["A", "B", "C"] {
        db.add_in_category(title, life_id).unwrap();
    }
    db.add_in_category("D", work_id).unwrap();

    let mut app = ScriviApp::new(
        db,
        Box::new(MockPlatform {
            state: Rc::new(RefCell::new(MockPlatformState::default())),
        }),
        None,
    );
    let mut h = TestHarness::new(1200.0, 600.0);
    pump_unpinned(&mut h, &mut app);

    // Reorder C above A inside Life
    let life = &app.snapshot().category_panels[&life_id];
    let from = life.todo_rows[2].1.handle_rect.center();
    let r0 = life.todo_rows[0].1.row_rect;
    let to = pos2(r0.center().x, r0.top() + r0.height() * 0.25);
    h.drag(&mut app, from, to);

    let got = app.db().unwrap().list_category_todos(life_id);
    assert_eq!(
        got.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
        vec!["C", "A", "B"]
    );

    // Move B from Life to top of Work
    let life = &app.snapshot().category_panels[&life_id];
    let work = &app.snapshot().category_panels[&work_id];
    let from = life.todo_rows[2].1.handle_rect.center();
    let w0 = work.todo_rows[0].1.row_rect;
    let to = pos2(w0.center().x, w0.top() + w0.height() * 0.25);
    h.drag(&mut app, from, to);

    let got = app.db().unwrap().list_category_todos(work_id);
    assert_eq!(
        got.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
        vec!["B", "D"]
    );

    // Drag Work card onto left half of Life card
    let life_card = app.snapshot().category_panels[&life_id].card_rect;
    let work_handle = app.snapshot().category_panels[&work_id].handle_rect.center();
    let to = pos2(
        life_card.left() + life_card.width() * 0.1,
        life_card.center().y,
    );
    h.drag(&mut app, work_handle, to);

    let cats = app.db().unwrap().list_categories();
    assert_eq!(cats[0].name, "Work");
    assert_eq!(cats[1].name, "Life");

    // Drag C from Life onto empty Home card
    let life = &app.snapshot().category_panels[&life_id];
    let home_card = app.snapshot().category_panels[&home_id].card_rect;
    let from = life.todo_rows[0].1.handle_rect.center();
    h.drag(&mut app, from, home_card.center());
    let home_todos = app.db().unwrap().list_category_todos(home_id);
    assert_eq!(home_todos.len(), 1);
    assert_eq!(home_todos[0].title, "C");

    // Single column vertical category drag using target's lower half
    h.resize(&mut app, 400.0, 900.0);
    let work_handle = app.snapshot().category_panels[&work_id].handle_rect.center();
    let life_card = app.snapshot().category_panels[&life_id].card_rect;
    let to = pos2(
        life_card.left() + life_card.width() * 0.2,
        life_card.top() + life_card.height() * 0.8,
    );
    h.drag(&mut app, work_handle, to);
    let cats = app.db().unwrap().list_categories();
    assert_eq!(cats[0].name, "Life");
    assert_eq!(cats[1].name, "Work");
}

#[test]
fn test_nested_scroll_propagates_at_edges() {
    let mut db = MemoryDb::open_in_memory().unwrap();
    let mut first_id = 0;
    for (i, name) in ["Life", "Work", "Home", "Other"].iter().enumerate() {
        let id = db.add_category(name).unwrap();
        if i == 0 {
            first_id = id;
        }
    }
    let mut app = ScriviApp::new(
        db,
        Box::new(MockPlatform {
            state: Rc::new(RefCell::new(MockPlatformState::default())),
        }),
        None,
    );
    let mut h = TestHarness::new(400.0, 400.0);
    pump_unpinned(&mut h, &mut app);

    let inner_rect = app.snapshot().category_panels[&first_id].scroll_rect;
    h.scroll(&mut app, inner_rect.center(), vec2(0.0, -80.0));
    assert!(
        app.outer_scroll_offset() > 0.0,
        "scrolling over an empty category did not scroll the category grid"
    );
    let offset = app.outer_scroll_offset();

    let cat_input = app.snapshot().new_cat_input_rect.unwrap();
    h.replace_text(&mut app, cat_input, "Another");
    let add_rect = app.snapshot().new_cat_add_rect.unwrap();
    h.click(&mut app, add_rect.center());

    assert!(
        (app.outer_scroll_offset() - offset).abs() < 0.01,
        "adding a category changed scroll position from {offset} to {}",
        app.outer_scroll_offset()
    );
}

#[test]
fn test_drag_auto_scroll_targets() {
    let mut db = MemoryDb::open_in_memory().unwrap();
    let life_id = db.add_category("Life").unwrap();
    for name in ["Work", "Home", "Other"] {
        db.add_category(name).unwrap();
    }
    for _ in 0..20 {
        db.add_in_category("Task", life_id).unwrap();
    }
    let mut app = ScriviApp::new(
        db,
        Box::new(MockPlatform {
            state: Rc::new(RefCell::new(MockPlatformState::default())),
        }),
        None,
    );
    let mut h = TestHarness::new(400.0, 400.0);
    pump_unpinned(&mut h, &mut app);

    let outer_rect = app.snapshot().outer_scroll_rect;
    app.scroll_near(pos2(outer_rect.left() + 20.0, outer_rect.bottom() - 5.0));
    assert!(
        app.outer_scroll_offset() > 0.0,
        "dragging at the grid edge did not scroll categories"
    );

    app.set_outer_scroll_offset_for_test(0.0);
    h.pump(&mut app);

    let inner_rect = app.snapshot().category_panels[&life_id].scroll_rect;
    app.scroll_near(pos2(inner_rect.left() + 20.0, inner_rect.bottom() - 5.0));
    assert!(
        app.inner_scroll_offset(life_id) > 0.0,
        "dragging at the category edge did not scroll todos"
    );
}

#[test]
fn test_wrapped_todos_resize_and_scroll() {
    let mut db = MemoryDb::open_in_memory().unwrap();
    let life_id = db.add_category("Life").unwrap();
    for title in [
        "A long task with several words that should wrap across multiple lines in a narrow category",
        "Another long task with several words to check that the rows do not overlap each other",
        "A third task that keeps the list taller than the available space",
        "And a fourth long task to make the inner list scroll",
    ] {
        db.add_in_category(title, life_id).unwrap();
    }
    db.add_category("Work").unwrap();
    db.add_category("Home").unwrap();

    let mut app = ScriviApp::new(
        db,
        Box::new(MockPlatform {
            state: Rc::new(RefCell::new(MockPlatformState::default())),
        }),
        None,
    );
    let mut h = TestHarness::new(400.0, 360.0);
    pump_unpinned(&mut h, &mut app);

    let verify = |app: &ScriviApp| {
        let panel = &app.snapshot().category_panels[&life_id];
        let mut bottom = f32::NEG_INFINITY;
        for &(todo_id, ref row) in &panel.todo_rows {
            let metrics = &app.snapshot().title_metrics[&todo_id];
            assert!(
                metrics.rect.height() + 0.01 >= metrics.required_height,
                "wrapped title is clipped: rect={:?} required={}",
                metrics.rect,
                metrics.required_height
            );
            if bottom.is_finite() {
                assert!(row.row_rect.top() + 0.01 >= bottom, "wrapped rows overlap");
            }
            bottom = row.row_rect.bottom();
        }
        assert!(
            bottom - panel.scroll_rect.top() <= panel.content_height + 1.0,
            "scroll content excludes wrapped rows"
        );
    };

    verify(&app);

    let inner_rect = app.snapshot().category_panels[&life_id].scroll_rect;
    h.scroll(&mut app, inner_rect.center(), vec2(0.0, -80.0));
    assert!(
        app.inner_scroll_offset(life_id) > 0.0,
        "long category did not scroll its own items"
    );

    app.set_inner_scroll_offset_for_test(life_id, 10_000.0);
    h.pump(&mut app);
    let inner_rect = app.snapshot().category_panels[&life_id].scroll_rect;
    h.scroll(&mut app, inner_rect.center(), vec2(0.0, -80.0));
    assert!(
        app.outer_scroll_offset() > 0.0,
        "scrolling at the end of a category did not continue scrolling the grid"
    );

    app.set_outer_scroll_offset_for_test(0.0);
    h.resize(&mut app, 760.0, 360.0);
    verify(&app);
    h.resize(&mut app, 400.0, 360.0);
    verify(&app);
}

#[test]
fn test_todo_strikethrough_follows_rendered_text() {
    let ctx = egui::Context::default();
    let p = palette(DEFAULT_THEME);
    for title in [
        "Buy milk",
        "A very long completed todo that wraps",
        "Résumé 日本語",
    ] {
        for width in [200.0_f32, 80.0] {
            let mut metrics_out = None;
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0))),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let galley = layout_todo_galley(
                        ui,
                        title,
                        width,
                        FontId::proportional(14.0),
                        p.muted,
                    );
                    let required = galley.size().y + 8.0;
                    let rect = Rect::from_min_size(Pos2::ZERO, vec2(width + 8.0, required));
                    let m = paint_todo_title(ui, rect, 4.0, 4.0, galley, required, true, p.muted);
                    metrics_out = Some(m);
                });
            });
            let metrics = metrics_out.unwrap();
            let mut rendered = String::new();
            for line in &metrics.lines {
                rendered.push_str(&line.text);
                if line.text.chars().any(|c| !c.is_whitespace()) {
                    let (a, b) = line
                        .strike_segment
                        .expect("non-empty line should have strikethrough");
                    assert!(a.x >= line.text_rect.left() - 0.5);
                    assert!(b.x <= line.text_rect.right() + 0.5);
                    assert!(b.x > a.x);
                }
            }
            let rendered_compact: String = rendered.chars().filter(|c| !c.is_whitespace()).collect();
            let title_compact: String = title.chars().filter(|c| !c.is_whitespace()).collect();
            assert_eq!(rendered_compact, title_compact, "wrapped text lost content");
        }
    }
}

#[test]
fn test_category_rows_fill_available_width() {
    for width in [300.0_f32, 340.0, 760.0, 1200.0] {
        let grid = compute_category_grid(5, vec2(width, 600.0), vec2(340.0, 320.0), GRID_GAP);
        let rects = &grid.rects;
        let mut start = 0;
        while start < rects.len() {
            let mut end = start;
            while end + 1 < rects.len() && (rects[end + 1].top() - rects[start].top()).abs() < 0.01
            {
                end += 1;
            }
            assert!((rects[start].left() - 0.0).abs() < 0.01);
            assert!(
                (width - rects[end].right()).abs() < 0.01,
                "row does not fill width {width}"
            );
            for i in (start + 1)..=end {
                let gap = rects[i].left() - rects[i - 1].right();
                assert!((gap - GRID_GAP).abs() < 0.01);
                assert!((rects[i].width() - rects[start].width()).abs() < 0.01);
            }
            start = end + 1;
        }
        let last = rects.last().unwrap();
        assert!(last.height() >= 320.0);
        assert!(grid.content_height.max(600.0) >= last.bottom() - 0.01);
    }

    let single = compute_category_grid(1, vec2(760.0, 600.0), vec2(340.0, 320.0), GRID_GAP);
    assert_eq!(single.rects[0].size(), vec2(760.0, 600.0));
}

#[test]
fn test_category_heading_fits_and_truncates() {
    let ctx = egui::Context::default();
    let _ = ctx.run(RawInput::default(), |_| {});
    let text_width = ctx.fonts(|f| {
        f.layout_no_wrap("Life".into(), FontId::proportional(16.0), egui::Color32::WHITE)
            .size()
            .x
    }) + 6.0;

    let heading = compute_heading_layout(
        Rect::from_min_size(Pos2::ZERO, vec2(220.0, 40.0)),
        vec2(40.0, 40.0),
        text_width,
        6.0,
    );
    assert!(heading.label_rect.width() >= text_width - 0.01);
    let left_margin = heading.icon_rect.left();
    let right_margin = 220.0 - heading.label_rect.right();
    assert!((left_margin - right_margin).abs() < 0.01);

    let narrow = compute_heading_layout(
        Rect::from_min_size(Pos2::ZERO, vec2(60.0, 40.0)),
        vec2(40.0, 40.0),
        text_width,
        6.0,
    );
    assert!(narrow.label_rect.right() <= 60.01);
}

#[test]
fn test_category_grid_resizes_in_scroll() {
    let mut db = MemoryDb::open_in_memory().unwrap();
    for name in ["Life", "Work", "Home", "Other"] {
        db.add_category_with_icon(name, "").unwrap();
    }
    let groups = db.list_categories();
    let last_id = groups.last().unwrap().id;

    let mut app = ScriviApp::new(
        db,
        Box::new(MockPlatform {
            state: Rc::new(RefCell::new(MockPlatformState::default())),
        }),
        None,
    );
    let mut h = TestHarness::new(1200.0, 600.0);

    for width in [1200.0_f32, 400.0, 760.0, 1200.0] {
        if width == 1200.0 && app.is_pinned() {
            pump_unpinned(&mut h, &mut app);
        }
        h.resize(&mut app, width, 600.0);
        let snap = app.snapshot();
        let last_card = snap.category_panels[&last_id].card_rect;
        let outer = snap.outer_scroll_rect;
        assert!(
            last_card.bottom() - outer.top() <= snap.outer_content_height + 0.01,
            "last category outside scroll content after resizing to {width}"
        );
        assert!(
            (last_card.right() - outer.right()).abs() < 0.01,
            "last row does not fill grid after resizing to {width}"
        );
    }
}

#[test]
fn test_button_and_title_cursors() {
    let ctx = egui::Context::default();
    let mut time = 0.0;

    let mut run_widgets = |pointer: Option<Pos2>| -> (CursorIcon, Rect, Rect, Rect) {
        time += 1.0 / 60.0;
        let mut btn_rect = Rect::NOTHING;
        let mut dis_rect = Rect::NOTHING;
        let mut check_rect = Rect::NOTHING;
        let events = pointer.map(|p| vec![Event::PointerMoved(p)]).unwrap_or_default();
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 200.0))),
            time: Some(time),
            events,
            ..Default::default()
        };
        let out = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                btn_rect = pointer_button(ui, "Add", true, DEFAULT_THEME).rect;
                dis_rect = pointer_button(ui, "Disabled", false, DEFAULT_THEME).rect;
                let (cr, _) = ui.allocate_exact_size(vec2(20.0, 20.0), egui::Sense::hover());
                check_rect = cr;
                todo_checkbox(ui, cr, egui::Id::new("c"), false, DEFAULT_THEME);
            });
        });
        (out.platform_output.cursor_icon, btn_rect, dis_rect, check_rect)
    };

    let (_, btn_rect, dis_rect, check_rect) = run_widgets(None);
    assert_eq!(run_widgets(Some(btn_rect.center())).0, CursorIcon::PointingHand);
    assert_eq!(run_widgets(Some(dis_rect.center())).0, CursorIcon::Default);
    assert_eq!(run_widgets(Some(check_rect.center())).0, CursorIcon::PointingHand);

    // Clickable todo title cursor
    let mut db = MemoryDb::open_in_memory().unwrap();
    let cat_id = db.add_category("Life").unwrap();
    db.add_in_category("Task", cat_id).unwrap();
    let mut app = ScriviApp::new(
        db,
        Box::new(MockPlatform {
            state: Rc::new(RefCell::new(MockPlatformState::default())),
        }),
        None,
    );
    let mut h = TestHarness::new(760.0, 600.0);
    pump_unpinned(&mut h, &mut app);
    let title_rect = app.snapshot().category_panels[&cat_id].todo_rows[0]
        .1
        .title_rect;
    h.hover(&mut app, title_rect.center());
    assert_eq!(app.last_cursor_icon(), CursorIcon::PointingHand);
}

#[test]
fn test_layout_stays_within_window_right_edge() {
    let widths = [300.0_f32, 400.0, 760.0, 1200.0];
    let assert_within_window = |app: &ScriviApp, width: f32, label: &str| {
        let max_right = width - 12.0;
        let snap = app.snapshot();
        let settings_rect = snap.settings_rect.expect("settings button missing");
        let add_cat_rect = snap.new_cat_add_rect.expect("add category button missing");

        assert!(
            settings_rect.right() <= max_right + 0.01,
            "{label} @ {width}: settings_rect {} exceeds {max_right}",
            settings_rect.right()
        );
        assert!(
            add_cat_rect.right() <= max_right + 0.01,
            "{label} @ {width}: new_cat_add_rect {} exceeds {max_right}",
            add_cat_rect.right()
        );
        assert!(
            (add_cat_rect.right() - settings_rect.right()).abs() < 0.01,
            "{label} @ {width}: new_cat_add_rect {} != settings_rect {}",
            add_cat_rect.right(),
            settings_rect.right()
        );
        assert!(
            snap.outer_scroll_rect.right() <= max_right + 0.01,
            "{label} @ {width}: outer_scroll_rect {} exceeds {max_right}",
            snap.outer_scroll_rect.right()
        );

        for (&cat_id, panel) in &snap.category_panels {
            assert!(
                panel.card_rect.right() <= max_right + 0.01,
                "{label} @ {width} cat {cat_id}: card_rect {} exceeds {max_right}",
                panel.card_rect.right()
            );
            let inner_right = panel.card_rect.right() - 12.0;
            assert!(
                panel.delete_rect.right() <= inner_right + 0.01,
                "{label} @ {width} cat {cat_id}: delete_rect {} exceeds {inner_right}",
                panel.delete_rect.right()
            );
            assert!(
                panel.color_rect.right() < panel.delete_rect.left(),
                "{label} @ {width} cat {cat_id}: color_rect {} overlaps delete_rect {}",
                panel.color_rect.right(),
                panel.delete_rect.left()
            );
            assert!(
                panel.todo_input_rect.right() < panel.todo_add_rect.left(),
                "{label} @ {width} cat {cat_id}: todo_input_rect {} overlaps todo_add_rect {}",
                panel.todo_input_rect.right(),
                panel.todo_add_rect.left()
            );
            assert!(
                panel.todo_add_rect.right() <= inner_right + 0.01,
                "{label} @ {width} cat {cat_id}: todo_add_rect {} exceeds {inner_right}",
                panel.todo_add_rect.right()
            );
            assert!(
                panel.scroll_rect.right() <= inner_right + 0.01,
                "{label} @ {width} cat {cat_id}: scroll_rect {} exceeds {inner_right}",
                panel.scroll_rect.right()
            );

            for &(todo_id, ref row) in &panel.todo_rows {
                assert!(
                    row.row_rect.right() <= inner_right + 0.01,
                    "{label} @ {width} cat {cat_id} todo {todo_id}: row_rect {} exceeds {inner_right}",
                    row.row_rect.right()
                );
                assert!(
                    row.delete_rect.right() <= inner_right + 0.01,
                    "{label} @ {width} cat {cat_id} todo {todo_id}: delete_rect {} exceeds {inner_right}",
                    row.delete_rect.right()
                );
                assert!(
                    row.edit_rect.right() < row.delete_rect.left(),
                    "{label} @ {width} cat {cat_id} todo {todo_id}: edit_rect {} overlaps delete_rect {}",
                    row.edit_rect.right(),
                    row.delete_rect.left()
                );
                assert!(
                    row.title_rect.right() < row.edit_rect.left(),
                    "{label} @ {width} cat {cat_id} todo {todo_id}: title_rect {} overlaps edit_rect {}",
                    row.title_rect.right(),
                    row.edit_rect.left()
                );
            }
        }
    };

    for count in [1usize, 2, 5] {
        let mut db = MemoryDb::open_in_memory().unwrap();
        let mut first_id = 0;
        for i in 0..count {
            let id = db.add_category(&format!("Category {}", i + 1)).unwrap();
            if i == 0 {
                first_id = id;
            }
            db.add_in_category(
                "A long task description with many words that wraps across multiple lines inside the card",
                id,
            )
            .unwrap();
            db.add_in_category(
                "SupercalifragilisticexpialidociousLongUnbrokenWordThatShouldStayClippedInsideTitleRect",
                id,
            )
            .unwrap();
        }

        let mut app = ScriviApp::new(
            db,
            Box::new(MockPlatform {
                state: Rc::new(RefCell::new(MockPlatformState::default())),
            }),
            None,
        );
        let mut h = TestHarness::new(760.0, 600.0);
        pump_unpinned(&mut h, &mut app);

        let draft_input = app.snapshot().category_panels[&first_id].todo_input_rect;
        h.replace_text(
            &mut app,
            draft_input,
            "Line one\nLine two with trailing spaces     ",
        );

        for width in widths {
            h.resize(&mut app, width, 600.0);
            assert_within_window(&app, width, &format!("{count} categories"));
        }
    }

    // EditTodo modal fits inside both 760x600 and narrow 400x600 viewports
    let mut db = MemoryDb::open_in_memory().unwrap();
    let cat_id = db.add_category("Life").unwrap();
    db.add_in_category("Task to edit", cat_id).unwrap();
    let mut app = ScriviApp::new(
        db,
        Box::new(MockPlatform {
            state: Rc::new(RefCell::new(MockPlatformState::default())),
        }),
        None,
    );
    let mut h = TestHarness::new(760.0, 600.0);
    pump_unpinned(&mut h, &mut app);

    let edit_rect = app.snapshot().category_panels[&cat_id].todo_rows[0]
        .1
        .edit_rect;
    h.click(&mut app, edit_rect.center());
    for width in [760.0_f32, 400.0] {
        h.resize(&mut app, width, 600.0);
        let modal_rect = app.snapshot().modal_rect.expect("EditTodo modal missing");
        assert!(
            modal_rect.left() >= -0.01 && modal_rect.right() <= width + 0.01,
            "EditTodo modal @ {width} overflows viewport: {modal_rect:?}"
        );
    }
}

#[test]
fn test_pinned_category_stack_layout() {
    for (count, avail, min_h) in [
        (1usize, vec2(360.0, 300.0), 240.0),
        (4usize, vec2(360.0, 300.0), 240.0),
        (4usize, vec2(740.0, 520.0), 240.0),
    ] {
        let stack = compute_pinned_category_stack(count, avail, min_h, GRID_GAP);
        assert_eq!(stack.columns, 1);
        assert_eq!(stack.rows, count.max(1));
        assert_eq!(stack.rects.len(), count);
        let expected_h = avail.y.max(min_h);
        assert_eq!(stack.rects[0], Rect::from_min_size(Pos2::ZERO, vec2(avail.x, expected_h)));
        if count > 1 {
            assert_eq!(
                stack.rects[1],
                Rect::from_min_size(pos2(0.0, expected_h + GRID_GAP), vec2(avail.x, expected_h))
            );
        }
        let expected_content = (count as f32) * expected_h + (count as f32 - 1.0) * GRID_GAP;
        assert!((stack.content_height - expected_content).abs() < 0.01);
    }
}

#[test]
fn test_pinned_mode_collapses_app_and_shows_single_scrollable_category() {
    let mut db = MemoryDb::open_in_memory().unwrap();
    let life_id = db.add_category("Life").unwrap();
    let work_id = db.add_category("Work").unwrap();
    let _home_id = db.add_category("Home").unwrap();
    let _other_id = db.add_category("Other").unwrap();
    let task_id = db.add_in_category("Pinned task", life_id).unwrap();
    db.add_in_category("Second cat task", work_id).unwrap();

    let state = Rc::new(RefCell::new(MockPlatformState::default()));
    let mut app = ScriviApp::new(
        db,
        Box::new(MockPlatform {
            state: state.clone(),
        }),
        None,
    );
    let mut h = TestHarness::new(PINNED_WINDOW_WIDTH, PINNED_WINDOW_HEIGHT);
    h.pump(&mut app);

    // Starts pinned and collapsed with only the first category visible
    assert!(app.is_pinned());
    assert!(state.borrow().pinned);
    assert_eq!(app.outer_scroll_offset(), 0.0);

    let snap = app.snapshot();
    assert!(snap.pin_rect.is_some());
    assert!(snap.settings_rect.is_none());
    assert!(snap.new_cat_emoji_rect.is_none());
    assert!(snap.new_cat_input_rect.is_none());
    assert!(snap.new_cat_add_rect.is_none());

    let outer = snap.outer_scroll_rect;
    let life_panel = &snap.category_panels[&life_id];
    let work_panel = &snap.category_panels[&work_id];
    assert!(
        (life_panel.card_rect.top() - outer.top()).abs() < 1.0,
        "first category does not start at top of pinned scroll area"
    );
    assert!(
        (life_panel.card_rect.bottom() - outer.bottom()).abs() < 1.0,
        "first category does not fill pinned scroll area height"
    );
    assert!(
        life_panel.card_rect.right() <= PINNED_WINDOW_WIDTH - 8.0 + 0.01,
        "first category exceeds pinned window right margin"
    );
    assert!(
        work_panel.card_rect.top() >= outer.bottom() + GRID_GAP - 1.0,
        "second category should start below the visible pinned viewport"
    );

    // Can still interact with todos in the first category while pinned
    let todo_title = life_panel.todo_rows[0].1.title_rect;
    h.click(&mut app, todo_title.center());
    assert!(app.db().unwrap().todo(task_id).unwrap().done);

    // Scrolling down reveals the next category
    let scroll_point = app.snapshot().category_panels[&life_id].scroll_rect.center();
    for _ in 0..8 {
        h.scroll(&mut app, scroll_point, vec2(0.0, -80.0));
    }
    assert!(
        app.outer_scroll_offset() > 0.0,
        "pinned view did not scroll to subsequent categories"
    );
    let snap = app.snapshot();
    let outer = snap.outer_scroll_rect;
    let work_rect = snap.category_panels[&work_id].card_rect;
    assert!(
        work_rect.top() < outer.bottom() && work_rect.bottom() > outer.top(),
        "second category did not scroll into pinned viewport: work={work_rect:?}, outer={outer:?}"
    );

    // Unpin restores full header and multi-column layout
    let pin_rect = app.snapshot().pin_rect.unwrap();
    h.click(&mut app, pin_rect.center());
    assert!(!app.is_pinned());
    assert!(!state.borrow().pinned);
    h.resize(&mut app, 760.0, 600.0);

    let snap = app.snapshot();
    assert!(snap.settings_rect.is_some());
    assert!(snap.new_cat_emoji_rect.is_some());
    assert!(snap.new_cat_input_rect.is_some());
    assert!(snap.new_cat_add_rect.is_some());
    let life_restored = snap.category_panels[&life_id].card_rect;
    let work_restored = snap.category_panels[&work_id].card_rect;
    assert!((life_restored.top() - work_restored.top()).abs() < 1.0);
    assert!(work_restored.left() > life_restored.right());

    // Scrolling while unpinned and re-pinning resets outer scroll to first category and collapses again
    app.set_outer_scroll_offset_for_test(100.0);
    h.pump(&mut app);
    let pin_rect = app.snapshot().pin_rect.unwrap();
    h.click(&mut app, pin_rect.center());
    assert!(app.is_pinned());
    assert!(state.borrow().pinned);
    assert_eq!(app.outer_scroll_offset(), 0.0);

    h.resize(&mut app, PINNED_WINDOW_WIDTH, PINNED_WINDOW_HEIGHT);
    let snap = app.snapshot();
    assert!(snap.settings_rect.is_none());
    assert!(snap.new_cat_add_rect.is_none());
    let outer = snap.outer_scroll_rect;
    assert!((snap.category_panels[&life_id].card_rect.top() - outer.top()).abs() < 1.0);
    assert!(snap.category_panels[&work_id].card_rect.top() >= outer.bottom() + GRID_GAP - 1.0);
}

#[test]
fn test_startup_pin_failure_leaves_app_unpinned() {
    let state = Rc::new(RefCell::new(MockPlatformState {
        fail_pin: true,
        ..Default::default()
    }));
    let mut app = new_app(Box::new(MockPlatform {
        state: state.clone(),
    }));
    let mut h = TestHarness::new(760.0, 600.0);
    h.pump(&mut app);

    assert!(!app.is_pinned());
    assert!(!state.borrow().pinned);
    assert_eq!(app.active_modal_title(), Some("Error"));
    let ok_rect = find_modal_button(&app, "OK");
    h.click(&mut app, ok_rect.center());

    assert!(app.active_modal_title().is_none());
    assert!(!app.is_pinned());
    assert!(app.snapshot().settings_rect.is_some());

    state.borrow_mut().fail_pin = false;
    let pin_rect = app.snapshot().pin_rect.unwrap();
    h.click(&mut app, pin_rect.center());
    assert!(app.is_pinned());
    assert!(state.borrow().pinned);
}
