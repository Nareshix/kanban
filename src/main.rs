use eframe::egui::{
    self, Color32, CornerRadius, Frame, Margin, RichText, ScrollArea, Sense, Stroke, Vec2,
};
use egui_shadcn::{
    button, card, dialog, CardProps, CardVariant, ControlSize, ControlVariant, DialogAlign,
    DialogProps, Input, InputSize, Label, Theme,
};

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 820.0])
            .with_title("Kanban Board"),
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

// ── Undo / Redo Architecture (Command Pattern) ─────────────────────────────────

#[derive(Clone)]
enum Command {
    Move { id: u64, from_col: Col, to_col: Col, from_idx: usize, to_idx: usize },
    Add { col: Col, card: KanbanCard },
    Delete { col: Col, card: KanbanCard, original_index: usize },
    Edit {
        id: u64, col: Col,
        old_title: String, new_title: String,
        old_desc: String, new_desc: String
    },
}

// Operations collected during rendering, applied afterwards to avoid borrow checker
enum Action {
    OpenAdd(Col),
    OpenEdit(u64, Col, String, String),

    AddCard { col: Col, title: String, desc: String },
    EditCard { id: u64, col: Col, title: String, desc: String },
    DeleteCard(u64, Col),
    MoveCardCol { id: u64, from: Col, to: Col },
    DndDrop { id: u64, from_col: Col, to_col: Col, to_idx: Option<usize> },

    Undo,
    Redo,
}

// ── App state ─────────────────────────────────────────────────────────────────

struct KanbanApp {
    theme: Theme,
    columns: [Vec<KanbanCard>; 3],
    next_id: u64,

    // Command History
    undo_stack: Vec<Command>,
    redo_stack: Vec<Command>,

    // "Add card" dialog
    add_open: bool,
    add_col: Col,
    add_title: String,
    add_desc: String,

    // "Edit card" dialog
    edit_open: bool,
    edit_id: u64,
    edit_col: Col,
    edit_title: String,
    edit_desc: String,
}

impl KanbanApp {
    fn new() -> Self {
        Self {
            theme: Theme::default(),
            columns: [
                vec![
                    KanbanCard {
                        id: 1,
                        title: "Design system setup".into(),
                        description: "Configure color tokens and typography".into(),
                    },
                    KanbanCard {
                        id: 2,
                        title: "Write unit tests".into(),
                        description: "Cover core business logic".into(),
                    },
                ],
                vec![KanbanCard {
                    id: 3,
                    title: "Implement auth flow".into(),
                    description: "OAuth2 with refresh tokens".into(),
                }],
                vec![KanbanCard {
                    id: 4,
                    title: "Project scaffolding".into(),
                    description: "Initial repo setup done".into(),
                }],
            ],
            next_id: 5,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            add_open: false,
            add_col: Col::Todo,
            add_title: String::new(),
            add_desc: String::new(),
            edit_open: false,
            edit_id: 0,
            edit_col: Col::Todo,
            edit_title: String::new(),
            edit_desc: String::new(),
        }
    }

    fn dispatch(&mut self, cmd: Command) {
        self.execute_command(&cmd);
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
    }

    fn execute_command(&mut self, cmd: &Command) {
        match cmd {
            Command::Move { id: _, from_col, to_col, from_idx, to_idx } => {
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
            Command::Delete { col, card: _, original_index } => {
                self.columns[*col as usize].remove(*original_index);
            }
            Command::Edit { id, col, new_title, new_desc, .. } => {
                if let Some(c) = self.columns[*col as usize].iter_mut().find(|c| c.id == *id) {
                    c.title = new_title.clone();
                    c.description = new_desc.clone();
                }
            }
        }
    }

    fn undo_command(&mut self, cmd: &Command) {
        match cmd {
            Command::Move { id, from_col, to_col, from_idx, to_idx: _ } => {
                if let Some(current_idx) = self.columns[*to_col as usize].iter().position(|c| c.id == *id) {
                    let card = self.columns[*to_col as usize].remove(current_idx);
                    self.columns[*from_col as usize].insert(*from_idx, card);
                }
            }
            Command::Add { col, card } => {
                self.columns[*col as usize].retain(|c| c.id != card.id);
            }
            Command::Delete { col, card, original_index } => {
                self.columns[*col as usize].insert(*original_index, card.clone());
            }
            Command::Edit { id, col, old_title, old_desc, .. } => {
                if let Some(c) = self.columns[*col as usize].iter_mut().find(|c| c.id == *id) {
                    c.title = old_title.clone();
                    c.description = old_desc.clone();
                }
            }
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::OpenAdd(col) => {
                self.add_open = true;
                self.add_col = col;
                self.add_title.clear();
                self.add_desc.clear();
            }
            Action::OpenEdit(id, col, title, desc) => {
                self.edit_open = true;
                self.edit_id = id;
                self.edit_col = col;
                self.edit_title = title;
                self.edit_desc = desc;
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
            Action::AddCard { col, title, desc } => {
                let card = KanbanCard { id: self.next_id, title, description: desc };
                self.next_id += 1;
                self.dispatch(Command::Add { col, card });
            }
            Action::DeleteCard(id, col) => {
                if let Some(idx) = self.columns[col as usize].iter().position(|c| c.id == id) {
                    let card = self.columns[col as usize][idx].clone();
                    self.dispatch(Command::Delete { col, card, original_index: idx });
                }
            }
            Action::EditCard { id, col, title, desc } => {
                if let Some(idx) = self.columns[col as usize].iter().position(|c| c.id == id) {
                    let old_title = self.columns[col as usize][idx].title.clone();
                    let old_desc = self.columns[col as usize][idx].description.clone();
                    self.dispatch(Command::Edit {
                        id, col, old_title, new_title: title, old_desc, new_desc: desc
                    });
                }
            }
            Action::MoveCardCol { id, from, to } => {
                if let Some(from_idx) = self.columns[from as usize].iter().position(|c| c.id == id) {
                    let to_idx = self.columns[to as usize].len();
                    self.dispatch(Command::Move { id, from_col: from, to_col: to, from_idx, to_idx });
                }
            }
            Action::DndDrop { id, from_col, to_col, to_idx } => {
                if let Some(from_idx) = self.columns[from_col as usize].iter().position(|c| c.id == id) {
                    let actual_to_idx = to_idx.unwrap_or_else(|| self.columns[to_col as usize].len());
                    self.dispatch(Command::Move { id, from_col, to_col, from_idx, to_idx: actual_to_idx });
                }
            }
        }
    }
}

const COL_DEFS: [(Col, &str, &str); 3] = [
    (Col::Todo, "Todo", "This item hasn't been started"),
    (Col::InProgress, "In Progress", "This is actively being worked on"),
    (Col::Done, "Done", "This has been completed"),
];

// ── Rendering ─────────────────────────────────────────────────────────────────

impl eframe::App for KanbanApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::dark();
        // Setting the overall App background to GitHub's `#010409`
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

            if i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Y,
            )) {
                actions.push(Action::Redo);
            }
        });

        let mut global_drop: Option<(u64, Col, Col, Option<usize>)> = None;

        egui::CentralPanel::default()
            .frame(Frame::new().fill(Color32::from_rgb(1, 4, 9)))
            .show(ctx, |ui| {
                ui.add_space(24.0);
                ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.add_space(24.0);

                        for (col_id, title, desc) in COL_DEFS {
                            let color = col_color(col_id);
                            let cards = self.columns[col_id as usize].clone();
                            let mut col_payload = None;

                            // Wrap the column in a scope so we can forcefully configure what
                            // `dnd_drop_zone` uses as its background rendering colors
                            ui.scope(|ui| {
                                let mut dnd_visuals = ui.visuals().clone();

                                // Inactive (Normal) Column State: GitHub Dark (#0D1117)
                                dnd_visuals.widgets.inactive.bg_fill = Color32::from_rgb(13, 17, 23);
                                dnd_visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(48, 54, 61));

                                // Active (Hovered) Column State: Subtle Brightness Bump & Theme Colored Border
                                dnd_visuals.widgets.active.bg_fill = Color32::from_rgb(22, 27, 34);
                                dnd_visuals.widgets.active.bg_stroke = Stroke::new(2.0, color);
                                *ui.visuals_mut() = dnd_visuals; // <--- The Fix

                                let column_frame = Frame::new()
                                    .corner_radius(CornerRadius::same(10))
                                    .inner_margin(Margin::same(18));

                                let (_, payload) = ui.dnd_drop_zone::<(Col, u64), _>(column_frame, |ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(340.0);
                                        ui.set_min_height(700.0);

                                        ui.horizontal(|ui| {
                                            let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                                            ui.painter().circle_stroke(rect.center(), 9.0, Stroke::new(2.0, color));
                                            ui.add_space(4.0);
                                            ui.label(RichText::new(title).color(Color32::WHITE).size(15.0).strong());
                                            ui.add_space(6.0);
                                            ui.label(RichText::new(cards.len().to_string()).color(Color32::from_rgb(100, 100, 120)).size(13.0));

                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if button(ui, &theme, "+", ControlVariant::Ghost, ControlSize::Sm, true).clicked() {
                                                    actions.push(Action::OpenAdd(col_id));
                                                }
                                            });
                                        });

                                        ui.add_space(4.0);
                                        ui.label(RichText::new(desc).color(Color32::from_rgb(75, 75, 95)).size(12.0));
                                        ui.add_space(14.0);

                                        // Cards
                                        for (idx, kcard) in cards.iter().enumerate() {
                                            let cid = kcard.id;
                                            let mut card_payload_inner = None;

                                            // Wrap the INDIVIDUAL CARD drop zone so it doesn't draw a grey box over itself
                                            ui.scope(|ui| {
                                                let mut card_visuals = ui.visuals().clone();

                                                // Make the drop zone totally transparent normally
                                                card_visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
                                                card_visuals.widgets.inactive.bg_stroke = Stroke::NONE;

                                                // When hovering EXACTLY over this card slot, highlight it slightly with the column color
                                                card_visuals.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 25);
                                                card_visuals.widgets.active.bg_stroke = Stroke::new(1.0, color);
                                                *ui.visuals_mut() = card_visuals; // <--- The Fix

                                                let (_, cp) = ui.dnd_drop_zone::<(Col, u64), _>(Frame::NONE.inner_margin(0.0), |ui| {
                                                    ui.dnd_drag_source(ui.make_persistent_id(("drag_card", cid)), (col_id, cid), |ui| {
                                                        card(
                                                            ui,
                                                            &theme,
                                                            CardProps::default().with_padding(egui::vec2(14.0, 12.0)).with_variant(CardVariant::Outline).with_shadow(false),
                                                            |cui| {
                                                                cui.vertical(|v| {
                                                                    v.set_width(304.0);
                                                                    v.label(RichText::new(&kcard.title).color(Color32::WHITE).size(13.0).strong());
                                                                    if !kcard.description.is_empty() {
                                                                        v.add_space(4.0);
                                                                        v.label(RichText::new(&kcard.description).color(Color32::from_rgb(100, 100, 120)).size(12.0));
                                                                    }
                                                                    v.add_space(8.0);

                                                                    v.horizontal(|row| {
                                                                        if let Some(prev) = prev_col(col_id) {
                                                                            if button(row, &theme, "<-", ControlVariant::Ghost, ControlSize::Sm, true).clicked() {
                                                                                actions.push(Action::MoveCardCol { id: cid, from: col_id, to: prev });
                                                                            }
                                                                        }
                                                                        if let Some(next) = next_col(col_id) {
                                                                            if button(row, &theme, "->", ControlVariant::Ghost, ControlSize::Sm, true).clicked() {
                                                                                actions.push(Action::MoveCardCol { id: cid, from: col_id, to: next });
                                                                            }
                                                                        }
                                                                        row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |right| {
                                                                            if button(right, &theme, "X", ControlVariant::Ghost, ControlSize::Sm, true).clicked() {
                                                                                actions.push(Action::DeleteCard(cid, col_id));
                                                                            }
                                                                            if button(right, &theme, "Edit", ControlVariant::Ghost, ControlSize::Sm, true).clicked() {
                                                                                actions.push(Action::OpenEdit(cid, col_id, kcard.title.clone(), kcard.description.clone()));
                                                                            }
                                                                        });
                                                                    });
                                                                });
                                                            },
                                                        );
                                                    });
                                                });
                                                card_payload_inner = cp;
                                            });

                                            // Precise card sorting drop mapping
                                            if let Some(payload) = card_payload_inner {
                                                let (from_col, dragged_id) = *payload;
                                                global_drop = Some((dragged_id, from_col, col_id, Some(idx)));
                                            }

                                            ui.add_space(10.0);
                                        }

                                        if button(ui, &theme, "+ Add item", ControlVariant::Ghost, ControlSize::Sm, true).clicked() {
                                            actions.push(Action::OpenAdd(col_id));
                                        }
                                    }); // end ui.vertical
                                }); // end col drop zone
                                col_payload = payload;
                            });

                            if let Some(payload) = col_payload {
                                if global_drop.is_none() {
                                    let (from_col, dragged_id) = *payload;
                                    global_drop = Some((dragged_id, from_col, col_id, None));
                                }
                            }

                            ui.add_space(18.0);
                        }
                    });
                });

                if let Some((dragged_id, from_col, to_col, target_idx)) = global_drop {
                    actions.push(Action::DndDrop { id: dragged_id, from_col, to_col, to_idx: target_idx });
                }

                // ── "Add card" dialog ──────────────────────────────────────
                {
                    let mut open = self.add_open;
                    let mut close_dialog = false;
                    let mut submit = false;
                    let add_title = &mut self.add_title;
                    let add_desc = &mut self.add_desc;

                    let _ = dialog(
                        ui, &theme,
                        DialogProps::new(ui.make_persistent_id("kanban-add-dialog"), &mut open)
                        .with_title("Add card").with_description("Fill in the details for your new task.")
                        .with_align(DialogAlign::Center).with_max_width(420.0).with_height(290.0).scrollable(false)
                        .with_scrim_opacity(160).with_close_on_background(true).with_close_on_escape(true).with_animation(true),
                        |body| {
                            body.add_space(12.0);
                            body.vertical(|v| {
                                v.spacing_mut().item_spacing.y = 6.0;
                                let tid = v.make_persistent_id("add-card-title");
                                Label::new("Title").for_id(tid).size(ControlSize::Sm).show(v, &theme);
                                Input::new(tid).placeholder("Task title").size(InputSize::Size2).width(v.available_width()).show(v, &theme, add_title);
                            });
                            body.add_space(10.0);
                            body.vertical(|v| {
                                v.spacing_mut().item_spacing.y = 6.0;
                                let did = v.make_persistent_id("add-card-desc");
                                Label::new("Description").for_id(did).size(ControlSize::Sm).show(v, &theme);
                                Input::new(did).placeholder("Optional description").size(InputSize::Size2).width(v.available_width()).show(v, &theme, add_desc);
                            });
                            body.add_space(18.0);
                            body.horizontal(|footer| {
                                if button(footer, &theme, "Cancel", ControlVariant::Outline, ControlSize::Md, true).clicked() {
                                    close_dialog = true;
                                }
                                footer.with_layout(egui::Layout::right_to_left(egui::Align::Center), |right| {
                                    if button(right, &theme, "Add card", ControlVariant::Primary, ControlSize::Md, true).clicked() {
                                        submit = true;
                                    }
                                });
                            });
                        },
                    );

                    if submit && !self.add_title.trim().is_empty() {
                        actions.push(Action::AddCard { col: self.add_col, title: self.add_title.trim().to_string(), desc: self.add_desc.trim().to_string() });
                        self.add_title.clear();
                        self.add_desc.clear();
                        open = false;
                    }
                    if close_dialog { open = false; }
                    self.add_open = open;
                }

                // ── "Edit card" dialog ─────────────────────────────────────
                {
                    let mut open = self.edit_open;
                    let mut close_dialog = false;
                    let mut save = false;
                    let edit_title = &mut self.edit_title;
                    let edit_desc = &mut self.edit_desc;

                    let _ = dialog(
                        ui, &theme,
                        DialogProps::new(ui.make_persistent_id("kanban-edit-dialog"), &mut open)
                        .with_title("Edit card").with_align(DialogAlign::Center).with_max_width(420.0).with_height(270.0).scrollable(false)
                        .with_scrim_opacity(160).with_close_on_background(true).with_close_on_escape(true).with_animation(true),
                        |body| {
                            body.add_space(12.0);
                            body.vertical(|v| {
                                v.spacing_mut().item_spacing.y = 6.0;
                                let tid = v.make_persistent_id("edit-card-title");
                                Label::new("Title").for_id(tid).size(ControlSize::Sm).show(v, &theme);
                                Input::new(tid).size(InputSize::Size2).width(v.available_width()).show(v, &theme, edit_title);
                            });
                            body.add_space(10.0);
                            body.vertical(|v| {
                                v.spacing_mut().item_spacing.y = 6.0;
                                let did = v.make_persistent_id("edit-card-desc");
                                Label::new("Description").for_id(did).size(ControlSize::Sm).show(v, &theme);
                                Input::new(did).size(InputSize::Size2).width(v.available_width()).show(v, &theme, edit_desc);
                            });
                            body.add_space(18.0);
                            body.horizontal(|footer| {
                                if button(footer, &theme, "Cancel", ControlVariant::Outline, ControlSize::Md, true).clicked() {
                                    close_dialog = true;
                                }
                                footer.with_layout(egui::Layout::right_to_left(egui::Align::Center), |right| {
                                    if button(right, &theme, "Save", ControlVariant::Primary, ControlSize::Md, true).clicked() {
                                        save = true;
                                    }
                                });
                            });
                        },
                    );

                    if save && !self.edit_title.trim().is_empty() {
                        actions.push(Action::EditCard { id: self.edit_id, col: self.edit_col, title: self.edit_title.trim().to_string(), desc: self.edit_desc.trim().to_string() });
                        open = false;
                    }
                    if close_dialog { open = false; }
                    self.edit_open = open;
                }
            });

        for action in actions {
            self.apply(action);
        }
    }
}