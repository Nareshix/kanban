use iced::border::Border;
use iced::widget::{column, container, row, scrollable, text, Space};
use iced::{Alignment, Background, Color, Element, Length, Subscription, Task};
use iced_drop::droppable;

use iced_shadcn::{
    button, card, input, label, ButtonProps, ButtonSize, ButtonVariant, CardProps, CardSize,
    CardVariant, InputProps, InputSize, InputVariant, Theme,
};

pub fn main() -> iced::Result {
    iced::application(KanbanApp::default, KanbanApp::update, KanbanApp::view)
        .title("Kanban Board - Iced") // FIXED: Passing static string directly
        .subscription(KanbanApp::subscription)
        .run()
}

// ── Data model ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
enum Message {
    OpenAdd(Col),
    UpdateAddTitle(String),
    UpdateAddDesc(String),
    SubmitAdd,

    OpenEdit(u64, Col, String, String),
    UpdateEditTitle(String),
    UpdateEditDesc(String),
    SubmitEdit,

    CloseDialog,

    MoveCardCol { id: u64, from: Col, to: Col },
    DeleteCard(u64, Col),

    // Drag and drop messages
    DragCardDropped { card_id: u64, point: iced::Point, rect: iced::Rectangle },
    HandleDropZones { card_id: u64, zones: Vec<(iced::widget::Id, iced::Rectangle)> },

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

impl Default for KanbanApp {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            columns: [
                vec![
                    KanbanCard { id: 1, title: "Design system setup".into(), description: "Configure color tokens and typography".into() },
                    KanbanCard { id: 2, title: "Write unit tests".into(), description: "Cover core business logic".into() },
                ],
                vec![KanbanCard { id: 3, title: "Implement auth flow".into(), description: "OAuth2 with refresh tokens".into() }],
                vec![KanbanCard { id: 4, title: "Project scaffolding".into(), description: "Initial repo setup done".into() }],
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
}

impl KanbanApp {
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
                if from_col == to_col && *from_idx < insert_idx { insert_idx -= 1; }
                let insert_idx = insert_idx.min(self.columns[*to_col as usize].len());
                self.columns[*to_col as usize].insert(insert_idx, card);
            }
            Command::Add { col, card } => self.columns[*col as usize].push(card.clone()),
            Command::Delete { col, original_index, .. } => { self.columns[*col as usize].remove(*original_index); }
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
            Command::Move { id, from_col, to_col, from_idx, .. } => {
                if let Some(current_idx) = self.columns[*to_col as usize].iter().position(|c| c.id == *id) {
                    let card = self.columns[*to_col as usize].remove(current_idx);
                    self.columns[*from_col as usize].insert(*from_idx, card);
                }
            }
            Command::Add { col, card } => self.columns[*col as usize].retain(|c| c.id != card.id),
            Command::Delete { col, card, original_index } => self.columns[*col as usize].insert(*original_index, card.clone()),
            Command::Edit { id, col, old_title, old_desc, .. } => {
                if let Some(c) = self.columns[*col as usize].iter_mut().find(|c| c.id == *id) {
                    c.title = old_title.clone();
                    c.description = old_desc.clone();
                }
            }
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenAdd(col) => {
                self.add_open = true;
                self.add_col = col;
                self.add_title.clear();
                self.add_desc.clear();
            }
            Message::UpdateAddTitle(val) => self.add_title = val,
            Message::UpdateAddDesc(val) => self.add_desc = val,
            Message::SubmitAdd => {
                if !self.add_title.trim().is_empty() {
                    let card = KanbanCard { id: self.next_id, title: self.add_title.trim().to_string(), description: self.add_desc.trim().to_string() };
                    self.next_id += 1;
                    self.dispatch(Command::Add { col: self.add_col, card });
                    self.add_open = false;
                }
            }
            Message::OpenEdit(id, col, title, desc) => {
                self.edit_open = true;
                self.edit_id = id;
                self.edit_col = col;
                self.edit_title = title;
                self.edit_desc = desc;
            }
            Message::UpdateEditTitle(val) => self.edit_title = val,
            Message::UpdateEditDesc(val) => self.edit_desc = val,
            Message::SubmitEdit => {
                if !self.edit_title.trim().is_empty() {
                    if let Some(idx) = self.columns[self.edit_col as usize].iter().position(|c| c.id == self.edit_id) {
                        let old_title = self.columns[self.edit_col as usize][idx].title.clone();
                        let old_desc = self.columns[self.edit_col as usize][idx].description.clone();
                        self.dispatch(Command::Edit {
                            id: self.edit_id, col: self.edit_col, old_title,
                            new_title: self.edit_title.trim().to_string(), old_desc, new_desc: self.edit_desc.trim().to_string()
                        });
                    }
                    self.edit_open = false;
                }
            }
            Message::CloseDialog => {
                self.add_open = false;
                self.edit_open = false;
            }
            Message::Undo => {
                if let Some(cmd) = self.undo_stack.pop() {
                    self.undo_command(&cmd);
                    self.redo_stack.push(cmd);
                }
            }
            Message::Redo => {
                if let Some(cmd) = self.redo_stack.pop() {
                    self.execute_command(&cmd);
                    self.undo_stack.push(cmd);
                }
            }
            Message::DeleteCard(id, col) => {
                if let Some(idx) = self.columns[col as usize].iter().position(|c| c.id == id) {
                    let card = self.columns[col as usize][idx].clone();
                    self.dispatch(Command::Delete { col, card, original_index: idx });
                }
            }
            Message::MoveCardCol { id, from, to } => {
                if let Some(from_idx) = self.columns[from as usize].iter().position(|c| c.id == id) {
                    let to_idx = self.columns[to as usize].len();
                    self.dispatch(Command::Move { id, from_col: from, to_col: to, from_idx, to_idx });
                }
            }
            // ── Drag and Drop Actions ──
            Message::DragCardDropped { card_id, point, rect: _ } => {
                return iced_drop::zones_on_point(
                    move |zones| Message::HandleDropZones { card_id, zones },
                    point,
                    None,
                    None,
                );
            }
            Message::HandleDropZones { card_id, zones } => {
                if let Some((zone_id, _)) = zones.first() {
                    let target_col = if *zone_id == iced::widget::Id::new("col_todo") {
                        Some(Col::Todo)
                    } else if *zone_id == iced::widget::Id::new("col_inprogress") {
                        Some(Col::InProgress)
                    } else if *zone_id == iced::widget::Id::new("col_done") {
                        Some(Col::Done)
                    } else {
                        None
                    };

                    if let Some(to) = target_col {
                        let mut from_col = None;
                        for col in [Col::Todo, Col::InProgress, Col::Done] {
                            if self.columns[col as usize].iter().any(|c| c.id == card_id) {
                                from_col = Some(col);
                                break;
                            }
                        }

                        if let Some(from) = from_col {
                            let from_idx = self.columns[from as usize].iter().position(|c| c.id == card_id).unwrap();
                            let to_idx = self.columns[to as usize].len();
                            self.dispatch(Command::Move { id: card_id, from_col: from, to_col: to, from_idx, to_idx });
                        }
                    }
                }
            }
        }
        Task::none()
    }

    // Global Keybinding subscriptions for Undo/Redo
    fn subscription(&self) -> Subscription<Message> {
        iced::event::listen_with(|event, _status, _window| {
            if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
                if modifiers.command() {
                    match key.as_ref() {
                        iced::keyboard::Key::Character("z") => {
                            if modifiers.shift() { return Some(Message::Redo); }
                            else { return Some(Message::Undo); }
                        }
                        iced::keyboard::Key::Character("y") => { return Some(Message::Redo); }
                        _ => {}
                    }
                }
            }
            None
        })
    }

    fn view(&self) -> Element<Message> {
        let content = if self.add_open {
            self.view_add_dialog()
        } else if self.edit_open {
            self.view_edit_dialog()
        } else {
            self.view_board()
        };

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgb8(1, 4, 9))),
                ..Default::default()
            })
            .into()
    }

    fn view_board(&self) -> Element<Message> {
        let col_defs = [
            (Col::Todo, "Todo", "This item hasn't been started"),
            (Col::InProgress, "In Progress", "This is actively being worked on"),
            (Col::Done, "Done", "This has been completed"),
        ];

        let columns_ui = col_defs.iter().map(|(col_id, title, desc)| {
            let cards = &self.columns[*col_id as usize];

            // FIXED: Using ButtonSize::Size1 instead of ButtonSize::Sm
            let mut col_content = column![
                row![
                    text(*title).size(15).color(Color::WHITE),
                    Space::new().width(Length::Fixed(6.0)).height(Length::Fixed(0.0)),
                    text(cards.len().to_string()).size(13).color(Color::from_rgb8(100, 100, 120)),
        Space::new().width(Length::Fill).height(Length::Fixed(0.0)), // Use Fill for expansion
                    button("+", Some(Message::OpenAdd(*col_id)), ButtonProps::new().variant(ButtonVariant::Ghost).size(ButtonSize::Size1), &self.theme)
                ].align_y(Alignment::Center),
                text(*desc).size(12).color(Color::from_rgb8(75, 75, 95)),
            ].spacing(8);

            let mut card_list = column![].spacing(10);
            for kcard in cards {
                let cid = kcard.id;

                // FIXED: Using ButtonSize::Size1 instead of ButtonSize::Sm
                let card_inner = column![
                    text(&kcard.title).size(14).color(Color::WHITE),
                    text(&kcard.description).size(12).color(Color::from_rgb8(100, 100, 120)),
                    row![
                        button("<-", prev_col(*col_id).map(|p| Message::MoveCardCol { id: cid, from: *col_id, to: p }), ButtonProps::new().variant(ButtonVariant::Ghost).size(ButtonSize::Size1), &self.theme),
                        button("->", next_col(*col_id).map(|n| Message::MoveCardCol { id: cid, from: *col_id, to: n }), ButtonProps::new().variant(ButtonVariant::Ghost).size(ButtonSize::Size1), &self.theme),
                        Space::new().width(Length::Fill),
                        button("Edit", Some(Message::OpenEdit(cid, *col_id, kcard.title.clone(), kcard.description.clone())), ButtonProps::new().variant(ButtonVariant::Ghost).size(ButtonSize::Size1), &self.theme),
                        button("X", Some(Message::DeleteCard(cid, *col_id)), ButtonProps::new().variant(ButtonVariant::Ghost).size(ButtonSize::Size1), &self.theme),
                    ].align_y(Alignment::Center)
                ].spacing(8);

                let card_widget = card(card_inner, CardProps::new().variant(CardVariant::Surface).size(CardSize::Size2), &self.theme);

                let wrapped_card = droppable(card_widget)
                    .on_drop(move |point, rect| Message::DragCardDropped { card_id: cid, point, rect });

                card_list = card_list.push(wrapped_card);
            }

            col_content = col_content.push(scrollable(card_list).height(Length::Fill));
            col_content = col_content.push(
                button("+ Add item", Some(Message::OpenAdd(*col_id)), ButtonProps::new().variant(ButtonVariant::Ghost).size(ButtonSize::Size1), &self.theme)
                    .width(Length::Fill)
            );

            let col_id_string = match col_id {
                Col::Todo => "col_todo",
                Col::InProgress => "col_inprogress",
                Col::Done => "col_done",
            };

            container(col_content)
                .id(iced::widget::Id::new(col_id_string))
                .width(Length::Fixed(340.0))
                .height(Length::Fill)
                .padding(18)
                .style(|_theme| iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgb8(13, 17, 23))),
                    border: Border { radius: 10.0.into(), width: 1.0, color: Color::from_rgb8(48, 54, 61) },
                    ..Default::default()
                })
                .into()
        });

        scrollable(
            row(columns_ui).spacing(24).padding(24)
        ).direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::default()))
        .into()
    }

    // ── Dialog Views ──────────────────────────────────────────────────────────

    fn view_add_dialog(&self) -> Element<Message> {
        let content = column![
            text("Add card").size(18).color(Color::WHITE),
            text("Fill in the details for your new task.").size(14).color(Color::from_rgb8(100, 100, 120)),
            Space::new().height(Length::Fixed(12.0)),

            label("Title", &self.theme),
            input(&self.add_title, "Task title", Some(Message::UpdateAddTitle), InputProps::new().size(InputSize::Size2).variant(InputVariant::Surface), &self.theme).width(Length::Fill),
            Space::new().height(Length::Fixed(8.0)),

            label("Description", &self.theme),
            input(&self.add_desc, "Optional description", Some(Message::UpdateAddDesc), InputProps::new().size(InputSize::Size2).variant(InputVariant::Surface), &self.theme).width(Length::Fill),
            Space::new().height(Length::Fixed(18.0)),

            // FIXED: Using ButtonSize::Size3 instead of ButtonSize::Md
            row![
                Space::new().width(Length::Fill),
                button("Cancel", Some(Message::CloseDialog), ButtonProps::new().variant(ButtonVariant::Outline).size(ButtonSize::Size3), &self.theme),
                button("Add card", Some(Message::SubmitAdd), ButtonProps::new().variant(ButtonVariant::Solid).size(ButtonSize::Size3), &self.theme),
            ].spacing(12)
        ].spacing(4);

        self.modal_container(content)
    }

    fn view_edit_dialog(&self) -> Element<Message> {
        let content = column![
            text("Edit card").size(18).color(Color::WHITE),
            Space::new().height(Length::Fixed(12.0)),

            label("Title", &self.theme),
            input(&self.edit_title, "Task title", Some(Message::UpdateEditTitle), InputProps::new().size(InputSize::Size2).variant(InputVariant::Surface), &self.theme).width(Length::Fill),
            Space::new().height(Length::Fixed(8.0)),

            label("Description", &self.theme),
            input(&self.edit_desc, "Description", Some(Message::UpdateEditDesc), InputProps::new().size(InputSize::Size2).variant(InputVariant::Surface), &self.theme).width(Length::Fill),
            Space::new().height(Length::Fixed(18.0)),

            // FIXED: Using ButtonSize::Size3 instead of ButtonSize::Md
            row![
                Space::new().width(Length::Fill),
                button("Cancel", Some(Message::CloseDialog), ButtonProps::new().variant(ButtonVariant::Outline).size(ButtonSize::Size3), &self.theme),
                button("Save", Some(Message::SubmitEdit), ButtonProps::new().variant(ButtonVariant::Solid).size(ButtonSize::Size3), &self.theme),
            ].spacing(12)
        ].spacing(4);

        self.modal_container(content)
    }

    fn modal_container<'a>(&self, content: iced::widget::Column<'a, Message>) -> Element<'a, Message> {
        let box_container = container(content)
            .width(Length::Fixed(420.0))
            .padding(24)
            .style(|_theme| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgb8(22, 27, 34))),
                border: Border { radius: 10.0.into(), width: 1.0, color: Color::from_rgb8(48, 54, 61) },
                ..Default::default()
            });

        container(box_container)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_theme| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgba8(1, 4, 9, 0.85))),
                ..Default::default()
            })
            .into()
    }
}