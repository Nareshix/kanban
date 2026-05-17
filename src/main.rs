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

// Deferred mutations – collected during rendering, applied afterwards to avoid
// borrow-checker conflicts inside nested closures.
enum Action {
    OpenAdd(Col),
    OpenEdit(u64, Col, String, String),
    Move(u64, Col, Col),
    Delete(u64, Col),
}

// ── App state ─────────────────────────────────────────────────────────────────

struct KanbanApp {
    theme: Theme,
    columns: [Vec<KanbanCard>; 3],
    next_id: u64,

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
            Action::Move(id, from, to) => {
                let fi = from as usize;
                let ti = to as usize;
                if let Some(pos) = self.columns[fi].iter().position(|c| c.id == id) {
                    let card = self.columns[fi].remove(pos);
                    self.columns[ti].push(card);
                }
            }
            Action::Delete(id, col) => {
                self.columns[col as usize].retain(|c| c.id != id);
            }
        }
    }
}

// ── Column header/description metadata ────────────────────────────────────────

const COL_DEFS: [(Col, &str, &str); 3] = [
    (Col::Todo, "Todo", "This item hasn't been started"),
    (Col::InProgress, "In Progress", "This is actively being worked on"),
    (Col::Done, "Done", "This has been completed"),
];

// ── Rendering ─────────────────────────────────────────────────────────────────

impl eframe::App for KanbanApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        // Dark background
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(8, 8, 12);
        ctx.set_visuals(visuals);

        // Clone theme so we can pass &Theme into closures without holding &self
        let theme = self.theme.clone();
        let mut actions: Vec<Action> = Vec::new();

        egui::CentralPanel::default()
            .frame(Frame::new().fill(Color32::from_rgb(8, 8, 12)))
            .show(ctx, |ui| {
                // ── Board columns ──────────────────────────────────────────
                ui.add_space(24.0);
                ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.add_space(24.0);

                        for (col_id, title, desc) in COL_DEFS {
                            let color = col_color(col_id);
                            // Clone the card list for this column so we can
                            // render cards while still being able to push
                            // actions without holding a borrow of self.columns.
                            let cards = self.columns[col_id as usize].clone();

                            Frame::new()
                                .fill(Color32::from_rgb(13, 13, 18))
                                .corner_radius(CornerRadius::same(10))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(28, 28, 38)))
                                .inner_margin(Margin::same(18))
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                    ui.set_width(340.0);
                                    ui.set_min_height(700.0);

                                    // Header row
                                    ui.horizontal(|ui| {
                                        let (rect, _) = ui.allocate_exact_size(
                                            Vec2::splat(22.0),
                                            Sense::hover(),
                                        );
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
                                                    &theme,
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

                                    // Cards
                                    for kcard in &cards {
                                        let cid = kcard.id;

                                        card(
                                            ui,
                                            &theme,
                                            CardProps::default()
                                                .with_padding(egui::vec2(14.0, 12.0))
                                                .with_variant(CardVariant::Outline)
                                                .with_shadow(false),
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
                                                        RichText::new(&kcard.description)
                                                            .color(Color32::from_rgb(
                                                                100, 100, 120,
                                                            ))
                                                            .size(12.0),
                                                    );
                                                }
                                                v.add_space(8.0);

                                                // Action row: ← → | Edit ✕
                                                v.horizontal(|row| {
                                                    if let Some(prev) = prev_col(col_id) {
                                                        if button(
                                                            row,
                                                            &theme,
                                                            "←",
                                                            ControlVariant::Ghost,
                                                            ControlSize::Sm,
                                                            true,
                                                        )
                                                        .clicked()
                                                        {
                                                            actions.push(Action::Move(
                                                                cid, col_id, prev,
                                                            ));
                                                        }
                                                    }
                                                    if let Some(next) = next_col(col_id) {
                                                        if button(
                                                            row,
                                                            &theme,
                                                            "→",
                                                            ControlVariant::Ghost,
                                                            ControlSize::Sm,
                                                            true,
                                                        )
                                                        .clicked()
                                                        {
                                                            actions.push(Action::Move(
                                                                cid, col_id, next,
                                                            ));
                                                        }
                                                    }
                                                    row.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |right| {
                                                            if button(
                                                                right,
                                                                &theme,
                                                                "✕",
                                                                ControlVariant::Ghost,
                                                                ControlSize::Sm,
                                                                true,
                                                            )
                                                            .clicked()
                                                            {
                                                                actions.push(Action::Delete(
                                                                    cid, col_id,
                                                                ));
                                                            }
                                                            if button(
                                                                right,
                                                                &theme,
                                                                "Edit",
                                                                ControlVariant::Ghost,
                                                                ControlSize::Sm,
                                                                true,
                                                            )
                                                            .clicked()
                                                            {
                                                                actions.push(Action::OpenEdit(
                                                                    cid,
                                                                    col_id,
                                                                    kcard.title.clone(),
                                                                    kcard.description.clone(),
                                                                ));
                                                            }
                                                        },
                                                    );
                                                });
                                                }); // end cui.vertical
                                            },
                                        );
                                        ui.add_space(10.0);
                                    }

                                    // Bottom "Add item" shortcut
                                    if button(
                                        ui,
                                        &theme,
                                        "+ Add item",
                                        ControlVariant::Ghost,
                                        ControlSize::Sm,
                                        true,
                                    )
                                    .clicked()
                                    {
                                        actions.push(Action::OpenAdd(col_id));
                                    }
                                    }); // end ui.vertical
                                });

                            ui.add_space(18.0);
                        }
                    });
                });

                // ── "Add card" dialog ──────────────────────────────────────
                // Following the same pattern as the dialog example:
                // copy the bool, pass &mut to dialog, write back after.
                {
                    let mut open = self.add_open;
                    let mut close = false;
                    let mut submit = false;
                    let add_title = &mut self.add_title;
                    let add_desc = &mut self.add_desc;

                    let _ = dialog(
                        ui,
                        &theme,
                        DialogProps::new(
                            ui.make_persistent_id("kanban-add-dialog"),
                            &mut open,
                        )
                        .with_title("Add card")
                        .with_description("Fill in the details for your new task.")
                        .with_align(DialogAlign::Center)
                        .with_max_width(420.0)
                        .with_height(290.0)
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
                                Label::new("Title")
                                    .for_id(tid)
                                    .size(ControlSize::Sm)
                                    .show(v, &theme);
                                Input::new(tid)
                                    .placeholder("Task title")
                                    .size(InputSize::Size2)
                                    .width(v.available_width())
                                    .show(v, &theme, add_title);
                            });

                            body.add_space(10.0);

                            body.vertical(|v| {
                                v.spacing_mut().item_spacing.y = 6.0;
                                let did = v.make_persistent_id("add-card-desc");
                                Label::new("Description")
                                    .for_id(did)
                                    .size(ControlSize::Sm)
                                    .show(v, &theme);
                                Input::new(did)
                                    .placeholder("Optional description")
                                    .size(InputSize::Size2)
                                    .width(v.available_width())
                                    .show(v, &theme, add_desc);
                            });

                            body.add_space(18.0);

                            body.horizontal(|footer| {
                                if button(
                                    footer,
                                    &theme,
                                    "Cancel",
                                    ControlVariant::Outline,
                                    ControlSize::Md,
                                    true,
                                )
                                .clicked()
                                {
                                    close = true;
                                }
                                footer.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |right| {
                                        if button(
                                            right,
                                            &theme,
                                            "Add card",
                                            ControlVariant::Primary,
                                            ControlSize::Md,
                                            true,
                                        )
                                        .clicked()
                                        {
                                            submit = true;
                                        }
                                    },
                                );
                            });
                        },
                    );

                    if submit && !self.add_title.trim().is_empty() {
                        let col_idx = self.add_col as usize;
                        self.columns[col_idx].push(KanbanCard {
                            id: self.next_id,
                            title: self.add_title.trim().to_string(),
                            description: self.add_desc.trim().to_string(),
                        });
                        self.next_id += 1;
                        self.add_title.clear();
                        self.add_desc.clear();
                        open = false;
                    }
                    if close {
                        open = false;
                    }
                    self.add_open = open;
                }

                // ── "Edit card" dialog ─────────────────────────────────────
                {
                    let mut open = self.edit_open;
                    let mut close = false;
                    let mut save = false;
                    let edit_title = &mut self.edit_title;
                    let edit_desc = &mut self.edit_desc;

                    let _ = dialog(
                        ui,
                        &theme,
                        DialogProps::new(
                            ui.make_persistent_id("kanban-edit-dialog"),
                            &mut open,
                        )
                        .with_title("Edit card")
                        .with_align(DialogAlign::Center)
                        .with_max_width(420.0)
                        .with_height(270.0)
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
                                Label::new("Title")
                                    .for_id(tid)
                                    .size(ControlSize::Sm)
                                    .show(v, &theme);
                                Input::new(tid)
                                    .size(InputSize::Size2)
                                    .width(v.available_width())
                                    .show(v, &theme, edit_title);
                            });

                            body.add_space(10.0);

                            body.vertical(|v| {
                                v.spacing_mut().item_spacing.y = 6.0;
                                let did = v.make_persistent_id("edit-card-desc");
                                Label::new("Description")
                                    .for_id(did)
                                    .size(ControlSize::Sm)
                                    .show(v, &theme);
                                Input::new(did)
                                    .size(InputSize::Size2)
                                    .width(v.available_width())
                                    .show(v, &theme, edit_desc);
                            });

                            body.add_space(18.0);

                            body.horizontal(|footer| {
                                if button(
                                    footer,
                                    &theme,
                                    "Cancel",
                                    ControlVariant::Outline,
                                    ControlSize::Md,
                                    true,
                                )
                                .clicked()
                                {
                                    close = true;
                                }
                                footer.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |right| {
                                        if button(
                                            right,
                                            &theme,
                                            "Save",
                                            ControlVariant::Primary,
                                            ControlSize::Md,
                                            true,
                                        )
                                        .clicked()
                                        {
                                            save = true;
                                        }
                                    },
                                );
                            });
                        },
                    );

                    if save && !self.edit_title.trim().is_empty() {
                        let col_idx = self.edit_col as usize;
                        if let Some(c) =
                            self.columns[col_idx].iter_mut().find(|c| c.id == self.edit_id)
                        {
                            c.title = self.edit_title.trim().to_string();
                            c.description = self.edit_desc.trim().to_string();
                        }
                        open = false;
                    }
                    if close {
                        open = false;
                    }
                    self.edit_open = open;
                }
            });

        for action in actions {
            self.apply(action);
        }
    }
}