#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use chrono::{Datelike, Duration, Local, NaiveDate};
use eframe::egui;
use egui::{Align, Color32, CornerRadius, Layout, Margin, RichText, Sense, Stroke, Vec2};
use egui_shadcn::{
    button, card, dialog, separator, tabs, CardProps, CardVariant, ControlSize, ControlVariant,
    DialogAlign, DialogProps, Input, InputSize, Label, SeparatorProps, TabItem, TabsProps,
    TabsVariant, Theme,
};

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 820.0])
            .with_title("Kanban, Timeline, Calendar & Issues"),
        ..Default::default()
    };
    eframe::run_native(
        "Kanban",
        options,
        Box::new(|_| Ok(Box::new(KanbanApp::new()))),
    )
}

// ── Data model ────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct KanbanCard {
    id: u64,
    title: String,
    description: String,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Col {
    Todo = 0,
    InProgress = 1,
    Done = 2,
}

fn col_color(col: Col) -> Color32 {
    match col {
        Col::Todo => Color32::from_rgb(34, 197, 94),
        Col::InProgress => Color32::from_rgb(234, 179, 8),
        Col::Done => Color32::from_rgb(139, 92, 246),
    }
}

fn col_name(col: Col) -> &'static str {
    match col {
        Col::Todo => "Todo",
        Col::InProgress => "In Progress",
        Col::Done => "Done",
    }
}

fn prev_col(col: Col) -> Option<Col> {
    match col {
        Col::Todo => None,
        Col::InProgress => Some(Col::Todo),
        Col::Done => Some(Col::InProgress),
    }
}

fn next_col(col: Col) -> Option<Col> {
    match col {
        Col::Todo => Some(Col::InProgress),
        Col::InProgress => Some(Col::Done),
        Col::Done => None,
    }
}

// ── Custom Shadcn-like UI Helpers ─────────────────────────────────────────────

fn custom_badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::NONE
        .fill(color.gamma_multiply(0.15))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(color).size(11.0).strong());
        });
}

fn custom_avatar(ui: &mut egui::Ui, initials: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 14.0, Color32::from_rgb(30, 40, 50));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials,
        egui::FontId::proportional(12.0),
        Color32::WHITE,
    );
}

// ── Undo / Redo Architecture (Command Pattern) ─────────────────────────────────

#[derive(Clone)]
enum Command {
    Move {
        id: u64,
        from_col: Col,
        to_col: Col,
        from_idx: usize,
        to_idx: usize,
    },
    Add {
        col: Col,
        card: KanbanCard,
    },
    Delete {
        col: Col,
        card: KanbanCard,
        original_index: usize,
    },
    Edit {
        id: u64,
        col: Col,
        old_title: String,
        new_title: String,
        old_desc: String,
        new_desc: String,
        old_start: Option<NaiveDate>,
        new_start: Option<NaiveDate>,
        old_end: Option<NaiveDate>,
        new_end: Option<NaiveDate>,
    },
}

// Operations collected during rendering
enum Action {
    OpenAdd(Col),
    OpenAddWithDate(NaiveDate),
    OpenEdit(
        u64,
        Col,
        String,
        String,
        Option<NaiveDate>,
        Option<NaiveDate>,
    ),

    AddCard {
        col: Col,
        title: String,
        desc: String,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    },
    EditCard {
        id: u64,
        col: Col,
        title: String,
        desc: String,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    },
    DeleteCard(u64, Col),
    MoveCardCol {
        id: u64,
        from: Col,
        to: Col,
    },
    DndDrop {
        id: u64,
        from_col: Col,
        to_col: Col,
        to_idx: Option<usize>,
    },

    Undo,
    Redo,
}

// ── App state ─────────────────────────────────────────────────────────────────

struct KanbanApp {
    theme: Theme,
    columns: [Vec<KanbanCard>; 3],
    next_id: u64,

    active_tab: String,
    calendar_selected_date: Option<NaiveDate>,

    // Command History
    undo_stack: Vec<Command>,
    redo_stack: Vec<Command>,

    // "Add card" dialog
    add_open: bool,
    add_col: Col,
    add_title: String,
    add_desc: String,
    add_start_str: String,
    add_end_str: String,

    // "Edit card" dialog
    edit_open: bool,
    edit_id: u64,
    edit_col: Col,
    edit_title: String,
    edit_desc: String,
    edit_start_str: String,
    edit_end_str: String,
}

impl KanbanApp {
    fn new() -> Self {
        let today = Local::now().date_naive();
        Self {
            theme: Theme::default(),
            active_tab: "board".to_string(),
            calendar_selected_date: Some(today),
            columns: [
                vec![
                    KanbanCard {
                        id: 1,
                        title: "Design system setup".into(),
                        description: "Configure color tokens and typography".into(),
                        start_date: Some(today),
                        end_date: Some(today + Duration::days(3)),
                    },
                    KanbanCard {
                        id: 2,
                        title: "Write unit tests".into(),
                        description: "Cover core business logic".into(),
                        start_date: Some(today + Duration::days(4)),
                        end_date: Some(today + Duration::days(7)),
                    },
                ],
                vec![KanbanCard {
                    id: 3,
                    title: "Implement auth flow".into(),
                    description: "OAuth2 with refresh tokens".into(),
                    start_date: Some(today + Duration::days(1)),
                    end_date: Some(today + Duration::days(5)),
                }],
                vec![KanbanCard {
                    id: 4,
                    title: "Project scaffolding".into(),
                    description: "Initial repo setup done".into(),
                    start_date: Some(today),
                    end_date: Some(today + Duration::days(1)),
                }],
            ],
            next_id: 5,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            add_open: false,
            add_col: Col::Todo,
            add_title: String::new(),
            add_desc: String::new(),
            add_start_str: String::new(),
            add_end_str: String::new(),
            edit_open: false,
            edit_id: 0,
            edit_col: Col::Todo,
            edit_title: String::new(),
            edit_desc: String::new(),
            edit_start_str: String::new(),
            edit_end_str: String::new(),
        }
    }

    fn dispatch(&mut self, cmd: Command) {
        self.execute_command(&cmd);
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
    }

    fn execute_command(&mut self, cmd: &Command) {
        match cmd {
            Command::Move {
                from_col,
                to_col,
                from_idx,
                to_idx,
                ..
            } => {
                let card = self.columns[*from_col as usize].remove(*from_idx);
                let mut insert_idx = *to_idx;

                if from_col == to_col && *from_idx < insert_idx {
                    insert_idx -= 1;
                }

                let insert_idx = insert_idx.min(self.columns[*to_col as usize].len());
                self.columns[*to_col as usize].insert(insert_idx, card);
            }
            Command::Add { col, card } => {
                self.columns[*col as usize].push(card.clone());
            }
            Command::Delete {
                col,
                original_index,
                ..
            } => {
                self.columns[*col as usize].remove(*original_index);
            }
            Command::Edit {
                id,
                col,
                new_title,
                new_desc,
                new_start,
                new_end,
                ..
            } => {
                if let Some(c) = self.columns[*col as usize].iter_mut().find(|c| c.id == *id) {
                    c.title = new_title.clone();
                    c.description = new_desc.clone();
                    c.start_date = *new_start;
                    c.end_date = *new_end;
                }
            }
        }
    }

    fn undo_command(&mut self, cmd: &Command) {
        match cmd {
            Command::Move {
                id,
                from_col,
                to_col,
                from_idx,
                ..
            } => {
                if let Some(current_idx) = self.columns[*to_col as usize]
                    .iter()
                    .position(|c| c.id == *id)
                {
                    let card = self.columns[*to_col as usize].remove(current_idx);
                    self.columns[*from_col as usize].insert(*from_idx, card);
                }
            }
            Command::Add { col, card } => {
                self.columns[*col as usize].retain(|c| c.id != card.id);
            }
            Command::Delete {
                col,
                card,
                original_index,
            } => {
                self.columns[*col as usize].insert(*original_index, card.clone());
            }
            Command::Edit {
                id,
                col,
                old_title,
                old_desc,
                old_start,
                old_end,
                ..
            } => {
                if let Some(c) = self.columns[*col as usize].iter_mut().find(|c| c.id == *id) {
                    c.title = old_title.clone();
                    c.description = old_desc.clone();
                    c.start_date = *old_start;
                    c.end_date = *old_end;
                }
            }
        }
    }

    fn apply(&mut self, action: Action) {
        let today = Local::now().date_naive();
        match action {
            Action::OpenAdd(col) => {
                self.add_open = true;
                self.add_col = col;
                self.add_title.clear();
                self.add_desc.clear();
                self.add_start_str = today.format("%Y-%m-%d").to_string();
                self.add_end_str = (today + Duration::days(1)).format("%Y-%m-%d").to_string();
            }
            Action::OpenAddWithDate(date) => {
                self.add_open = true;
                self.add_col = Col::Todo;
                self.add_title.clear();
                self.add_desc.clear();
                self.add_start_str = date.format("%Y-%m-%d").to_string();
                self.add_end_str = (date + Duration::days(1)).format("%Y-%m-%d").to_string();
            }
            Action::OpenEdit(id, col, title, desc, start, end) => {
                self.edit_open = true;
                self.edit_id = id;
                self.edit_col = col;
                self.edit_title = title;
                self.edit_desc = desc;
                self.edit_start_str = start.map_or("".into(), |d| d.format("%Y-%m-%d").to_string());
                self.edit_end_str = end.map_or("".into(), |d| d.format("%Y-%m-%d").to_string());
            }
            Action::Undo => {
                if let Some(cmd) = self.undo_stack.pop() {
                    self.undo_command(&cmd);
                    self.redo_stack.push(cmd);
                }
            }
            Action::Redo => {
                if let Some(cmd) = self.redo_stack.pop() {
                    self.execute_command(&cmd);
                    self.undo_stack.push(cmd);
                }
            }
            Action::AddCard {
                col,
                title,
                desc,
                start_date,
                end_date,
            } => {
                let card = KanbanCard {
                    id: self.next_id,
                    title,
                    description: desc,
                    start_date,
                    end_date,
                };
                self.next_id += 1;
                self.dispatch(Command::Add { col, card });
            }
            Action::DeleteCard(id, col) => {
                if let Some(idx) = self.columns[col as usize].iter().position(|c| c.id == id) {
                    let card = self.columns[col as usize][idx].clone();
                    self.dispatch(Command::Delete {
                        col,
                        card,
                        original_index: idx,
                    });
                }
            }
            Action::EditCard {
                id,
                col,
                title,
                desc,
                start_date,
                end_date,
            } => {
                if let Some(idx) = self.columns[col as usize].iter().position(|c| c.id == id) {
                    let old_title = self.columns[col as usize][idx].title.clone();
                    let old_desc = self.columns[col as usize][idx].description.clone();
                    let old_start = self.columns[col as usize][idx].start_date;
                    let old_end = self.columns[col as usize][idx].end_date;
                    self.dispatch(Command::Edit {
                        id,
                        col,
                        old_title,
                        new_title: title,
                        old_desc,
                        new_desc: desc,
                        old_start,
                        new_start: start_date,
                        old_end,
                        new_end: end_date,
                    });
                }
            }
            Action::MoveCardCol { id, from, to } => {
                if let Some(from_idx) = self.columns[from as usize].iter().position(|c| c.id == id)
                {
                    let to_idx = self.columns[to as usize].len();
                    self.dispatch(Command::Move {
                        id,
                        from_col: from,
                        to_col: to,
                        from_idx,
                        to_idx,
                    });
                }
            }
            Action::DndDrop {
                id,
                from_col,
                to_col,
                to_idx,
            } => {
                if let Some(from_idx) = self.columns[from_col as usize]
                    .iter()
                    .position(|c| c.id == id)
                {
                    let actual_to_idx =
                        to_idx.unwrap_or_else(|| self.columns[to_col as usize].len());
                    self.dispatch(Command::Move {
                        id,
                        from_col,
                        to_col,
                        from_idx,
                        to_idx: actual_to_idx,
                    });
                }
            }
        }
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

impl eframe::App for KanbanApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::dark();
        // Setting the overall App background slightly dark GitHub style `#010409`
        visuals.panel_fill = Color32::from_rgb(1, 4, 9);
        ctx.set_visuals(visuals);

        let theme = self.theme.clone();
        let mut actions: Vec<Action> = Vec::new();

        ctx.input_mut(|i| {
            if i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                egui::Key::Z,
            )) {
                actions.push(Action::Redo);
            } else if i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Z,
            )) {
                actions.push(Action::Undo);
            }
        });

        let mut global_drop: Option<(u64, Col, Col, Option<usize>)> = None;

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(1, 4, 9)).inner_margin(24.0))
            .show(ctx, |ui| {
                // ── Header / Navigation Tabs ──────────────────────────────────────
                let tab_items = [
                    TabItem::new("board", "Board"),
                    TabItem::new("timeline", "Timeline"),
                    TabItem::new("calendar", "Calendar"),
                    TabItem::new("table", "Table"),
                    TabItem::new("issues", "Issues"),
                ];

                let mut active_tab = self.active_tab.clone();

                tabs(
                    ui,
                    &theme,
                    TabsProps::new(ui.make_persistent_id("main-nav"), &tab_items, &mut active_tab)
                        .with_variant(TabsVariant::Soft),
                    |ui, active_tab_item| {
                        ui.add_space(24.0);
                        match active_tab_item.id.as_str() {
                            "board" => self.render_board(ui, &theme, &mut actions, &mut global_drop),
                            "timeline" => self.render_timeline(ui),
                            "calendar" => self.render_calendar(ui, &theme, &mut actions),
                            "table" => self.render_table(ui, &theme),
                            "issues" => self.render_issues(ui, &theme),
                            _ => {}
                        }
                    },
                );

                self.active_tab = active_tab;

                if let Some((dragged_id, from_col, to_col, target_idx)) = global_drop {
                    actions.push(Action::DndDrop { id: dragged_id, from_col, to_col, to_idx: target_idx });
                }

                // ── "Add card" dialog ──────────────────────────────────────
                self.render_add_dialog(ui, &theme, &mut actions);

                // ── "Edit card" dialog ─────────────────────────────────────
                self.render_edit_dialog(ui, &theme, &mut actions);
            });

        for action in actions {
            self.apply(action);
        }
    }
}

// ── Views ──────────────────────────────────────────────────────────────────────

impl KanbanApp {
    fn render_board(
        &mut self,
        ui: &mut egui::Ui,
        theme: &Theme,
        actions: &mut Vec<Action>,
        global_drop: &mut Option<(u64, Col, Col, Option<usize>)>,
    ) {
        let col_defs = [
            (Col::Todo, "Todo", "This item hasn't been started"),
            (Col::InProgress, "In Progress", "This is actively being worked on"),
            (Col::Done, "Done", "This has been completed"),
        ];

        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for (col_id, title, desc) in col_defs {
                    let color = col_color(col_id);
                    let cards = self.columns[col_id as usize].clone();
                    let mut col_payload = None;

                    ui.scope(|ui| {
                        let mut dnd_visuals = ui.visuals().clone();
                        dnd_visuals.widgets.inactive.bg_fill = Color32::from_rgb(13, 17, 23);
                        dnd_visuals.widgets.inactive.bg_stroke =
                            Stroke::new(1.0, Color32::from_rgb(48, 54, 61));
                        dnd_visuals.widgets.active.bg_fill = Color32::from_rgb(22, 27, 34);
                        dnd_visuals.widgets.active.bg_stroke = Stroke::new(2.0, color);
                        *ui.visuals_mut() = dnd_visuals;

                        let column_frame = egui::Frame::new()
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(Margin::same(18));

                        let (_, payload) =
                            ui.dnd_drop_zone::<(Col, u64), _>(column_frame, |ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(340.0);
                                    ui.set_min_height(600.0); // Make space for dnd drops

                                    ui.horizontal(|ui| {
                                        let (rect, _) =
                                            ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                                        ui.painter().circle_stroke(
                                            rect.center(),
                                            9.0,
                                            Stroke::new(2.0, color),
                                        );
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new(title)
                                                .color(Color32::WHITE)
                                                .size(15.0)
                                                .strong(),
                                        );
                                        ui.add_space(6.0);
                                        ui.label(
                                            RichText::new(cards.len().to_string())
                                                .color(Color32::from_rgb(100, 100, 120))
                                                .size(13.0),
                                        );

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if button(
                                                    ui,
                                                    theme,
                                                    "+",
                                                    ControlVariant::Ghost,
                                                    ControlSize::Sm,
                                                    true,
                                                )
                                                .clicked()
                                                {
                                                    actions.push(Action::OpenAdd(col_id));
                                                }
                                            },
                                        );
                                    });

                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(desc)
                                            .color(Color32::from_rgb(75, 75, 95))
                                            .size(12.0),
                                    );
                                    ui.add_space(14.0);

                                    for (idx, kcard) in cards.iter().enumerate() {
                                        let cid = kcard.id;
                                        let mut card_payload_inner = None;

                                        ui.scope(|ui| {
                                            let mut card_visuals = ui.visuals().clone();
                                            card_visuals.widgets.inactive.bg_fill =
                                                Color32::TRANSPARENT;
                                            card_visuals.widgets.inactive.bg_stroke = Stroke::NONE;
                                            card_visuals.widgets.active.bg_fill =
                                                Color32::from_rgba_unmultiplied(
                                                    color.r(),
                                                    color.g(),
                                                    color.b(),
                                                    25,
                                                );
                                            card_visuals.widgets.active.bg_stroke =
                                                Stroke::new(1.0, color);
                                            *ui.visuals_mut() = card_visuals;

                                            let (_, cp) = ui.dnd_drop_zone::<(Col, u64), _>(
                                                egui::Frame::NONE.inner_margin(0.0),
                                                |ui| {
                                                    ui.dnd_drag_source(
                                                        ui.make_persistent_id(("drag_card", cid)),
                                                        (col_id, cid),
                                                        |ui| {
                                                            card(
                                                                ui,
                                                                theme,
                                                                CardProps::default()
                                                                    .with_padding(egui::vec2(14.0, 12.0))
                                                                    .with_variant(CardVariant::Outline),
                                                                |cui| {
                                                                    cui.vertical(|v| {
                                                                        v.set_width(304.0);
                                                                        v.label(
                                                                            RichText::new(&kcard.title)
                                                                                .color(Color32::WHITE)
                                                                                .size(13.0)
                                                                                .strong(),
                                                                        );
                                                                        if !kcard.description.is_empty() {
                                                                            v.add_space(4.0);
                                                                            v.label(
                                                                                RichText::new(
                                                                                    &kcard.description,
                                                                                )
                                                                                .color(Color32::from_rgb(100, 100, 120))
                                                                                .size(12.0),
                                                                            );
                                                                        }
                                                                        v.add_space(8.0);

                                                                        v.horizontal(|row| {
                                                                            if let Some(prev) = prev_col(col_id) {
                                                                                if button(row, theme, "<-", ControlVariant::Ghost, ControlSize::Sm, true).clicked() {
                                                                                    actions.push(Action::MoveCardCol { id: cid, from: col_id, to: prev });
                                                                                }
                                                                            }
                                                                            if let Some(next) = next_col(col_id) {
                                                                                if button(row, theme, "->", ControlVariant::Ghost, ControlSize::Sm, true).clicked() {
                                                                                    actions.push(Action::MoveCardCol { id: cid, from: col_id, to: next });
                                                                                }
                                                                            }
                                                                            row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |right| {
                                                                                if button(right, theme, "X", ControlVariant::Ghost, ControlSize::Sm, true).clicked() {
                                                                                    actions.push(Action::DeleteCard(cid, col_id));
                                                                                }
                                                                                if button(right, theme, "Edit", ControlVariant::Ghost, ControlSize::Sm, true).clicked() {
                                                                                    actions.push(Action::OpenEdit(cid, col_id, kcard.title.clone(), kcard.description.clone(), kcard.start_date, kcard.end_date));
                                                                                }
                                                                            });
                                                                        });
                                                                    });
                                                                },
                                                            );
                                                        },
                                                    );
                                                },
                                            );
                                            card_payload_inner = cp;
                                        });

                                        if let Some(payload) = card_payload_inner {
                                            let (from_col, dragged_id) = *payload;
                                            *global_drop =
                                                Some((dragged_id, from_col, col_id, Some(idx)));
                                        }

                                        ui.add_space(10.0);
                                    }

                                    if button(
                                        ui,
                                        theme,
                                        "+ Add item",
                                        ControlVariant::Ghost,
                                        ControlSize::Sm,
                                        true,
                                    )
                                    .clicked()
                                    {
                                        actions.push(Action::OpenAdd(col_id));
                                    }
                                });
                            });
                        col_payload = payload;
                    });

                    if let Some(payload) = col_payload {
                        if global_drop.is_none() {
                            let (from_col, dragged_id) = *payload;
                            *global_drop = Some((dragged_id, from_col, col_id, None));
                        }
                    }

                    ui.add_space(18.0);
                }
            });
        });
    }

    fn render_timeline(&self, ui: &mut egui::Ui) {
        let mut all_cards = Vec::new();
        let col_defs = [Col::Todo, Col::InProgress, Col::Done];
        for col_id in col_defs {
            for card in &self.columns[col_id as usize] {
                all_cards.push((col_id, card.clone()));
            }
        }

        let base_date = Local::now().date_naive() - Duration::days(5);

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let num_days = 31;
                let day_width = 44.0;
                let row_height = 48.0;
                let label_width = 240.0;
                let header_height = 36.0;

                let total_size = Vec2::new(
                    label_width + (num_days as f32 * day_width),
                    header_height + (all_cards.len() as f32 * row_height),
                );

                let (rect, _response) = ui.allocate_exact_size(total_size, Sense::hover());
                let painter = ui.painter_at(rect);

                for day in 0..num_days {
                    let x = rect.left() + label_width + (day as f32 * day_width);
                    let display_date = base_date + Duration::days(day);
                    painter.text(
                        egui::pos2(x + day_width / 2.0, rect.top() + header_height / 2.0),
                        egui::Align2::CENTER_CENTER,
                        display_date.format("%d %b").to_string(),
                        egui::FontId::proportional(12.0),
                        Color32::from_rgb(100, 100, 120),
                    );
                    painter.line_segment(
                        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                        Stroke::new(1.0, Color32::from_rgb(25, 30, 38)),
                    );
                }

                for (i, (col, card)) in all_cards.iter().enumerate() {
                    let y = rect.top() + header_height + (i as f32 * row_height);

                    painter.line_segment(
                        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                        Stroke::new(1.0, Color32::from_rgb(35, 40, 48)),
                    );

                    painter.text(
                        egui::pos2(rect.left() + 10.0, y + row_height / 2.0),
                        egui::Align2::LEFT_CENTER,
                        &card.title,
                        egui::FontId::proportional(14.0),
                        Color32::WHITE,
                    );

                    let start_offset = card
                        .start_date
                        .map(|d| (d - base_date).num_days() as i32)
                        .unwrap_or(0)
                        .max(0);
                    let end_offset = card
                        .end_date
                        .map(|d| (d - base_date).num_days() as i32)
                        .unwrap_or(start_offset + 1)
                        .max(start_offset);

                    let start_x = rect.left() + label_width + (start_offset as f32 * day_width);
                    let end_x =
                        rect.left() + label_width + (end_offset as f32 * day_width) + day_width;

                    let bar_rect = egui::Rect::from_min_max(
                        egui::pos2(start_x, y + 10.0),
                        egui::pos2(end_x, y + row_height - 10.0),
                    );

                    let mut base_color = col_color(*col);

                    let bar_resp = ui.interact(bar_rect, ui.id().with(card.id), Sense::hover());
                    if bar_resp.hovered() {
                        base_color = Color32::from_rgb(
                            (base_color.r() as u16 + 40).min(255) as u8,
                            (base_color.g() as u16 + 40).min(255) as u8,
                            (base_color.b() as u16 + 40).min(255) as u8,
                        );
                        bar_resp.on_hover_ui_at_pointer(|ui| {
                            let sd = card.start_date.map_or("".into(), |d| d.format("%Y-%m-%d").to_string());
                            let ed = card.end_date.map_or("".into(), |d| d.format("%Y-%m-%d").to_string());
                            ui.label(format!("{}\n{} to {}", card.title, sd, ed));
                        });
                    }

                    painter.rect_filled(bar_rect, CornerRadius::same(6), base_color);
                    painter.rect_stroke(
                        bar_rect,
                        CornerRadius::same(6),
                        Stroke::new(1.0, Color32::BLACK),
                        egui::StrokeKind::Inside,
                    );
                }
            });
    }

    fn render_calendar(&mut self, ui: &mut egui::Ui, theme: &Theme, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            card(
                ui,
                theme,
                CardProps::default().with_padding(egui::vec2(16.0, 16.0)),
                |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Calendar").strong().size(16.0));
                        ui.add_space(8.0);

                        // Fake a calendar with egui grid
                        let today = Local::now().date_naive();
                        let start_of_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
                        let start_weekday = start_of_month.weekday().num_days_from_monday();

                        egui::Grid::new("calendar_grid").spacing([8.0, 8.0]).show(ui, |ui| {
                            for day in ["M", "T", "W", "T", "F", "S", "S"] {
                                ui.label(RichText::new(day).color(theme.palette.muted_foreground).strong());
                            }
                            ui.end_row();

                            let mut current_date = start_of_month - Duration::days(start_weekday as i64);

                            for _week in 0..5 {
                                for _day in 0..7 {
                                    let mut btn = egui::Button::new(current_date.format("%d").to_string());

                                    if current_date.month() != today.month() {
                                        btn = btn.fill(Color32::TRANSPARENT);
                                    } else if Some(current_date) == self.calendar_selected_date {
                                        btn = btn.fill(theme.palette.primary);
                                    }

                                    if ui.add_sized([32.0, 32.0], btn).clicked() {
                                        self.calendar_selected_date = Some(current_date);
                                    }
                                    current_date += Duration::days(1);
                                }
                                ui.end_row();
                            }
                        });

                        if let Some(d) = self.calendar_selected_date {
                            ui.add_space(16.0);
                            if button(ui, theme, "+ Add Task Here", ControlVariant::Outline, ControlSize::Sm, true).clicked() {
                                actions.push(Action::OpenAddWithDate(d));
                            }
                        }
                    });
                },
            );

            ui.add_space(24.0);

            // Display tasks that land on the selected date
            ui.vertical(|ui| {
                ui.label(RichText::new("Tasks for this day").size(18.0).strong());
                ui.add_space(12.0);

                let mut any_tasks = false;
                if let Some(selected_date) = self.calendar_selected_date {
                    let col_defs = [Col::Todo, Col::InProgress, Col::Done];
                    for col_id in col_defs {
                        for card in &self.columns[col_id as usize] {
                            let match_date = card.start_date.unwrap_or(Local::now().date_naive());
                            if match_date == selected_date {
                                any_tasks = true;
                                ui.horizontal(|ui| {
                                    custom_badge(ui, col_name(col_id), col_color(col_id));
                                    ui.label(&card.title);
                                });
                                ui.add_space(6.0);
                            }
                        }
                    }
                }
                if !any_tasks {
                    ui.label(RichText::new("No tasks scheduled.").color(theme.palette.muted_foreground));
                }
            });
        });
    }

    fn render_table(&mut self, ui: &mut egui::Ui, theme: &Theme) {
        let mut all_cards = Vec::new();
        let col_defs = [Col::Todo, Col::InProgress, Col::Done];
        for col_id in col_defs {
            for card in &self.columns[col_id as usize] {
                all_cards.push((col_id, card.clone()));
            }
        }

        egui::Frame::NONE
            .stroke(Stroke::new(1.0, theme.palette.border))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                egui::Grid::new("table_grid").striped(true).spacing([40.0, 16.0]).show(ui, |ui| {
                    ui.label(RichText::new("ID").strong());
                    ui.label(RichText::new("Title").strong());
                    ui.label(RichText::new("Status").strong());
                    ui.label(RichText::new("Due Date").strong());
                    ui.end_row();

                    for (col, card) in all_cards {
                        ui.label(card.id.to_string());
                        ui.label(&card.title);
                        custom_badge(ui, col_name(col), col_color(col));
                        if let Some(d) = card.end_date {
                            ui.label(d.format("%Y-%m-%d").to_string());
                        } else {
                            ui.label("-");
                        }
                        ui.end_row();
                    }
                });
            });
    }

    fn render_issues(&self, ui: &mut egui::Ui, theme: &Theme) {
        let mut all_cards = Vec::new();
        let col_defs = [Col::Todo, Col::InProgress, Col::Done];
        for col_id in col_defs {
            for card in &self.columns[col_id as usize] {
                all_cards.push((col_id, card.clone()));
            }
        }
        all_cards.sort_by_key(|c| c.1.id);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_max_width(900.0);
            for (col, card) in all_cards {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let icon_color = if col == Col::Done {
                        Color32::from_rgb(139, 92, 246)
                    } else {
                        Color32::from_rgb(34, 197, 94)
                    };
                    ui.label(RichText::new("●").color(icon_color).size(18.0));

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&card.title).strong().size(16.0));
                            custom_badge(ui, col_name(col), col_color(col));
                        });
                        ui.label(
                            RichText::new(format!("#{} opened recently by developer", card.id))
                                .color(theme.palette.muted_foreground)
                                .size(12.0),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        custom_avatar(ui, "DV");
                    });
                });
                separator(ui, theme, SeparatorProps::default());
            }
        });
    }

    // ── Dialogs ────────────────────────────────────────────────────────────────

    fn render_add_dialog(&mut self, ui: &mut egui::Ui, theme: &Theme, actions: &mut Vec<Action>) {
        let mut open = self.add_open;
        let mut close_dialog = false;
        let mut submit = false;

        let add_title = &mut self.add_title;
        let add_desc = &mut self.add_desc;
        let add_start_str = &mut self.add_start_str;
        let add_end_str = &mut self.add_end_str;

        let _ = dialog(
            ui,
            theme,
            DialogProps::new(ui.make_persistent_id("kanban-add-dialog"), &mut open)
                .with_title("Add task")
                .with_description("Fill in the details for your new task.")
                .with_align(DialogAlign::Center)
                .with_max_width(450.0)
                .scrollable(false)
                .with_scrim_opacity(160)
                .with_close_on_background(true)
                .with_close_on_escape(true)
                .with_animation(true),
            |body| {
                body.add_space(12.0);
                body.vertical(|v| {
                    v.spacing_mut().item_spacing.y = 6.0;
                    let tid = v.make_persistent_id("add-card-title");
                    Label::new("Title").for_id(tid).size(ControlSize::Sm).show(v, theme);
                    Input::new(tid)
                        .placeholder("Task title")
                        .size(InputSize::Size2)
                        .width(v.available_width())
                        .show(v, theme, add_title);
                });
                body.add_space(10.0);
                body.vertical(|v| {
                    v.spacing_mut().item_spacing.y = 6.0;
                    let did = v.make_persistent_id("add-card-desc");
                    Label::new("Description").for_id(did).size(ControlSize::Sm).show(v, theme);
                    Input::new(did)
                        .placeholder("Optional description")
                        .size(InputSize::Size2)
                        .width(v.available_width())
                        .show(v, theme, add_desc);
                });
                body.add_space(10.0);
                body.horizontal(|h| {
                    h.vertical(|v| {
                        v.spacing_mut().item_spacing.y = 6.0;
                        let sid = v.make_persistent_id("add-card-start");
                        Label::new("Start Date").for_id(sid).size(ControlSize::Sm).show(v, theme);
                        Input::new(sid)
                            .placeholder("YYYY-MM-DD")
                            .size(InputSize::Size2)
                            .width(180.0)
                            .show(v, theme, add_start_str);
                    });
                    h.add_space(16.0);
                    h.vertical(|v| {
                        v.spacing_mut().item_spacing.y = 6.0;
                        let eid = v.make_persistent_id("add-card-end");
                        Label::new("End Date").for_id(eid).size(ControlSize::Sm).show(v, theme);
                        Input::new(eid)
                            .placeholder("YYYY-MM-DD")
                            .size(InputSize::Size2)
                            .width(180.0)
                            .show(v, theme, add_end_str);
                    });
                });
                body.add_space(18.0);
                body.horizontal(|footer| {
                    if button(footer, theme, "Cancel", ControlVariant::Outline, ControlSize::Md, true).clicked() {
                        close_dialog = true;
                    }
                    footer.with_layout(egui::Layout::right_to_left(egui::Align::Center), |right| {
                        if button(right, theme, "Add task", ControlVariant::Primary, ControlSize::Md, true).clicked() {
                            submit = true;
                        }
                    });
                });
            },
        );

        if submit && !self.add_title.trim().is_empty() {
            actions.push(Action::AddCard {
                col: self.add_col,
                title: self.add_title.trim().to_string(),
                desc: self.add_desc.trim().to_string(),
                start_date: NaiveDate::parse_from_str(&self.add_start_str, "%Y-%m-%d").ok(),
                end_date: NaiveDate::parse_from_str(&self.add_end_str, "%Y-%m-%d").ok(),
            });
            open = false;
        }
        if close_dialog { open = false; }
        self.add_open = open;
    }

    fn render_edit_dialog(&mut self, ui: &mut egui::Ui, theme: &Theme, actions: &mut Vec<Action>) {
        let mut open = self.edit_open;
        let mut close_dialog = false;
        let mut save = false;

        let edit_title = &mut self.edit_title;
        let edit_desc = &mut self.edit_desc;
        let edit_start_str = &mut self.edit_start_str;
        let edit_end_str = &mut self.edit_end_str;

        let _ = dialog(
            ui,
            theme,
            DialogProps::new(ui.make_persistent_id("kanban-edit-dialog"), &mut open)
                .with_title("Edit task")
                .with_align(DialogAlign::Center)
                .with_max_width(450.0)
                .scrollable(false)
                .with_scrim_opacity(160)
                .with_close_on_background(true)
                .with_close_on_escape(true)
                .with_animation(true),
            |body| {
                body.add_space(12.0);
                body.vertical(|v| {
                    v.spacing_mut().item_spacing.y = 6.0;
                    let tid = v.make_persistent_id("edit-card-title");
                    Label::new("Title").for_id(tid).size(ControlSize::Sm).show(v, theme);
                    Input::new(tid).size(InputSize::Size2).width(v.available_width()).show(v, theme, edit_title);
                });
                body.add_space(10.0);
                body.vertical(|v| {
                    v.spacing_mut().item_spacing.y = 6.0;
                    let did = v.make_persistent_id("edit-card-desc");
                    Label::new("Description").for_id(did).size(ControlSize::Sm).show(v, theme);
                    Input::new(did).size(InputSize::Size2).width(v.available_width()).show(v, theme, edit_desc);
                });
                body.add_space(10.0);
                body.horizontal(|h| {
                    h.vertical(|v| {
                        v.spacing_mut().item_spacing.y = 6.0;
                        let sid = v.make_persistent_id("edit-card-start");
                        Label::new("Start Date").for_id(sid).size(ControlSize::Sm).show(v, theme);
                        Input::new(sid)
                            .placeholder("YYYY-MM-DD")
                            .size(InputSize::Size2)
                            .width(180.0)
                            .show(v, theme, edit_start_str);
                    });
                    h.add_space(16.0);
                    h.vertical(|v| {
                        v.spacing_mut().item_spacing.y = 6.0;
                        let eid = v.make_persistent_id("edit-card-end");
                        Label::new("End Date").for_id(eid).size(ControlSize::Sm).show(v, theme);
                        Input::new(eid)
                            .placeholder("YYYY-MM-DD")
                            .size(InputSize::Size2)
                            .width(180.0)
                            .show(v, theme, edit_end_str);
                    });
                });
                body.add_space(18.0);
                body.horizontal(|footer| {
                    if button(footer, theme, "Cancel", ControlVariant::Outline, ControlSize::Md, true).clicked() {
                        close_dialog = true;
                    }
                    footer.with_layout(egui::Layout::right_to_left(egui::Align::Center), |right| {
                        if button(right, theme, "Save", ControlVariant::Primary, ControlSize::Md, true).clicked() {
                            save = true;
                        }
                    });
                });
            },
        );

        if save && !self.edit_title.trim().is_empty() {
            actions.push(Action::EditCard {
                id: self.edit_id,
                col: self.edit_col,
                title: self.edit_title.trim().to_string(),
                desc: self.edit_desc.trim().to_string(),
                start_date: NaiveDate::parse_from_str(&self.edit_start_str, "%Y-%m-%d").ok(),
                end_date: NaiveDate::parse_from_str(&self.edit_end_str, "%Y-%m-%d").ok(),
            });
            open = false;
        }
        if close_dialog { open = false; }
        self.edit_open = open;
    }
}