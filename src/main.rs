#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use chrono::{Datelike, Duration, Local, Months, NaiveDate};
use iced::widget::canvas::{self, Path, Stroke};
use iced::widget::{column, container, row, scrollable, text, Space};
use iced::advanced::widget::Id as WidgetId;
use iced_drop::droppable;
use iced::{Alignment, Background, Color, Element, Font, Length, Point, Rectangle, Size, Task};
use iced_shadcn::{
    button, card, dialog, input, separator, tabs_content, tabs_contents, tabs_list, tabs_trigger,
    ButtonProps, ButtonSize, ButtonVariant, CardProps, CardVariant, DialogAlign, DialogProps,
    InputProps, SeparatorProps, TabsHover, TabsListProps, TabsListVariant, TabsRootProps, Theme,
};

fn main() -> iced::Result {
    iced::application(GtdApp::new, GtdApp::update, GtdApp::view)
        .theme(|_: &GtdApp| iced::Theme::Dark)
        .subscription(GtdApp::subscription)
        .window(iced::window::Settings {
            size: Size::new(1400.0, 850.0),
            ..Default::default()
        })
        .run()
}

// ── Data model ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct GtdTask {
    id: u64,
    title: String,
    description: String,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Col {
    Inbox = 0,
    NextActions = 1,
    Scheduled = 2,
    WaitingFor = 3,
    Projects = 4,
    SomedayMaybe = 5,
    Reference = 6,
}

const NUM_COLS: usize = 7;

fn col_color(col: Col) -> Color {
    match col {
        Col::Inbox => Color::from_rgb8(56, 189, 248),      // Light Blue
        Col::NextActions => Color::from_rgb8(34, 197, 94), // Green
        Col::Scheduled => Color::from_rgb8(234, 179, 8),   // Yellow
        Col::WaitingFor => Color::from_rgb8(249, 115, 22), // Orange
        Col::Projects => Color::from_rgb8(139, 92, 246),   // Purple
        Col::SomedayMaybe => Color::from_rgb8(168, 162, 158),// Gray
        Col::Reference => Color::from_rgb8(71, 85, 105),   // Slate
    }
}

fn col_name(col: Col) -> &'static str {
    match col {
        Col::Inbox => "Inbox",
        Col::NextActions => "Next Actions",
        Col::Scheduled => "Calendar",
        Col::WaitingFor => "Waiting For",
        Col::Projects => "Projects",
        Col::SomedayMaybe => "Someday / Maybe",
        Col::Reference => "Reference",
    }
}

fn apply_opacity(mut color: Color, opacity: f32) -> Color {
    color.a *= opacity;
    color
}

fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    if months == 0 {
        return date;
    }
    if months > 0 {
        date.checked_add_months(Months::new(months as u32))
            .unwrap_or(date)
            .with_day(1)
            .unwrap()
    } else {
        date.checked_sub_months(Months::new((-months) as u32))
            .unwrap_or(date)
            .with_day(1)
            .unwrap()
    }
}

fn strong_text<'a>(content: &str) -> iced::widget::Text<'a> {
    text(content.to_string()).font(Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    })
}

// ── Process Wizard State ──────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum ProcessStep {
    IsActionable,
    NonActionableDest,
    IsMultiStep,
    NextActionDefinition,
    DelegateOrDefer,
    DeferDest,
}

// ── Undo / Redo Architecture ──────────────────────────────────────────────────

#[derive(Clone, Debug)]
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
        card: GtdTask,
    },
    Delete {
        col: Col,
        card: GtdTask,
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

// ── App State & Messages ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum Message {
    TabSelected(String),

    // Quick Dump
    QuickDumpChanged(String),
    SubmitQuickDump,

    // Dialog Triggers
    OpenAdd(Col, Option<NaiveDate>),
    OpenEdit(
        u64,
        Col,
        String,
        String,
        Option<NaiveDate>,
        Option<NaiveDate>,
    ),
    CloseDialogs,

    // Form Inputs
    AddTitleChanged(String),
    AddDescChanged(String),
    AddStartChanged(String),
    AddEndChanged(String),
    SubmitAdd,

    EditTitleChanged(String),
    EditDescChanged(String),
    EditStartChanged(String),
    EditEndChanged(String),
    SubmitEdit,

    // Process Flowchart Logic
    StartProcess(u64),
    ProcessSetStep(ProcessStep),
    ProcessTitleChanged(String),
    ProcessDateChanged(String),
    ProcessExecute(ProcessOutcome),

    // Actions
    MoveCard(u64, Col, Col),
    DeleteCard(u64, Col),
    Undo,
    Redo,

    // Drag and Drop
    CardDropped(u64, Col, Point, Rectangle),
    HandleDropZones(u64, Col, Vec<(WidgetId, Rectangle)>),

    // Calendar Navigation
    CalendarPrevMonth,
    CalendarNextMonth,
    CalendarToday,
}

#[derive(Clone, Debug)]
enum ProcessOutcome {
    Trash,
    Incubate,     // SomedayMaybe
    Reference,
    Project,
    DoIt,         // Less than 2 minutes -> Delete/Done
    Delegate,     // Waiting For
    DeferASAP,    // Next Actions
    DeferCalendar,// Scheduled
}

struct GtdApp {
    theme: Theme,
    columns: [Vec<GtdTask>; NUM_COLS],
    next_id: u64,

    active_tab: String,
    calendar_view_month: NaiveDate,

    undo_stack: Vec<Command>,
    redo_stack: Vec<Command>,

    col_ids: [WidgetId; NUM_COLS],
    moving_card: Option<u64>,

    // Quick Dump
    quick_dump_text: String,

    // Flowchart / Process State
    process_open: bool,
    process_item_id: u64,
    process_step: ProcessStep,
    process_title_input: String,
    process_date_input: String,

    // Dialog state
    add_open: bool,
    add_col: Col,
    add_title: String,
    add_desc: String,
    add_start_str: String,
    add_end_str: String,

    edit_open: bool,
    edit_id: u64,
    edit_col: Col,
    edit_title: String,
    edit_desc: String,
    edit_start_str: String,
    edit_end_str: String,
}

impl GtdApp {
    fn new() -> (Self, Task<Message>) {
        let today = Local::now().date_naive();
        let app = Self {
            theme: Theme::dark(),
            active_tab: "board".to_string(),
            calendar_view_month: NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap(),
            columns: [
                vec![], // Inbox
                vec![], // Next Actions
                vec![], // Scheduled
                vec![], // Waiting For
                vec![], // Projects
                vec![], // SomedayMaybe
                vec![], // Reference
            ],
            next_id: 1,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            col_ids: [
                WidgetId::new("col_inbox"),
                WidgetId::new("col_next"),
                WidgetId::new("col_scheduled"),
                WidgetId::new("col_waiting"),
                WidgetId::new("col_projects"),
                WidgetId::new("col_someday"),
                WidgetId::new("col_reference"),
            ],
            moving_card: None,

            quick_dump_text: String::new(),

            process_open: false,
            process_item_id: 0,
            process_step: ProcessStep::IsActionable,
            process_title_input: String::new(),
            process_date_input: String::new(),

            add_open: false,
            add_col: Col::Inbox,
            add_title: String::new(),
            add_desc: String::new(),
            add_start_str: String::new(),
            add_end_str: String::new(),
            edit_open: false,
            edit_id: 0,
            edit_col: Col::Inbox,
            edit_title: String::new(),
            edit_desc: String::new(),
            edit_start_str: String::new(),
            edit_end_str: String::new(),
        };
        (app, Task::none())
    }

    fn execute_command(&mut self, cmd: &Command) {
        match cmd {
            Command::Move { from_col, to_col, from_idx, to_idx, .. } => {
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
            Command::Delete { col, original_index, .. } => {
                self.columns[*col as usize].remove(*original_index);
            }
            Command::Edit { id, col, new_title, new_desc, new_start, new_end, .. } => {
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
            Command::Move { id, from_col, to_col, from_idx, .. } => {
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
            Command::Edit { id, col, old_title, old_desc, old_start, old_end, .. } => {
                if let Some(c) = self.columns[*col as usize].iter_mut().find(|c| c.id == *id) {
                    c.title = old_title.clone();
                    c.description = old_desc.clone();
                    c.start_date = *old_start;
                    c.end_date = *old_end;
                }
            }
        }
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::event::listen_with(|event, _status, _window| {
            if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
                let cmd = modifiers.control() || modifiers.logo();
                if cmd {
                    return match key.as_ref() {
                        iced::keyboard::Key::Character("z") => {
                            if modifiers.shift() { Some(Message::Redo) } else { Some(Message::Undo) }
                        }
                        iced::keyboard::Key::Character("y") => Some(Message::Redo),
                        _ => None,
                    };
                }
            }
            None
        })
    }

    fn dispatch(&mut self, cmd: Command) {
        self.execute_command(&cmd);
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(tab) => self.active_tab = tab,

            // ── Quick Dump Smart Logic ──
            Message::QuickDumpChanged(val) => self.quick_dump_text = val,
            Message::SubmitQuickDump => {
                if !self.quick_dump_text.trim().is_empty() {
                    let mut text = self.quick_dump_text.trim().to_string();
                    let lowercase_text = text.to_lowercase();
                    let today = Local::now().date_naive();

                    let mut target_date = None;
                    let mut target_col = Col::Inbox;

                    // Magic Scheduling Keywords
                    if lowercase_text.ends_with(" tomorrow") || lowercase_text.ends_with(" tmr") {
                        target_date = Some(today + Duration::days(1));
                        target_col = Col::Scheduled; // smart route!

                        let cut_len = if lowercase_text.ends_with(" tmr") { 4 } else { 9 };
                        text = text[..text.len() - cut_len].trim().to_string();
                    } else if lowercase_text.ends_with(" today") {
                        target_date = Some(today);
                        target_col = Col::Scheduled; // smart route!
                        text = text[..text.len() - 6].trim().to_string();
                    }

                    let card = GtdTask {
                        id: self.next_id,
                        title: text,
                        description: String::new(),
                        start_date: target_date,
                        end_date: target_date,
                    };
                    self.next_id += 1;

                    self.dispatch(Command::Add { col: target_col, card });
                    self.quick_dump_text.clear();
                }
            }

            Message::OpenAdd(col, date) => {
                self.add_open = true;
                self.add_col = col;
                self.add_title.clear();
                self.add_desc.clear();

                if let Some(d) = date {
                    self.add_start_str = d.format("%Y-%m-%d").to_string();
                    self.add_end_str = self.add_start_str.clone();
                } else {
                    self.add_start_str.clear();
                    self.add_end_str.clear();
                }
            }
            Message::OpenEdit(id, col, title, desc, start, end) => {
                self.edit_open = true;
                self.edit_id = id;
                self.edit_col = col;
                self.edit_title = title;
                self.edit_desc = desc;
                self.edit_start_str = start.map_or("".into(), |d| d.format("%Y-%m-%d").to_string());
                self.edit_end_str = end.map_or("".into(), |d| d.format("%Y-%m-%d").to_string());
            }
            Message::CloseDialogs => {
                self.add_open = false;
                self.edit_open = false;
                self.process_open = false;
            }

            Message::AddTitleChanged(val) => self.add_title = val,
            Message::AddDescChanged(val) => self.add_desc = val,
            Message::AddStartChanged(val) => self.add_start_str = val,
            Message::AddEndChanged(val) => self.add_end_str = val,
            Message::SubmitAdd => {
                if !self.add_title.trim().is_empty() {
                    let start_d = NaiveDate::parse_from_str(&self.add_start_str, "%Y-%m-%d").ok();
                    let end_d = NaiveDate::parse_from_str(&self.add_end_str, "%Y-%m-%d").ok();

                    // Smart Routing: If adding to Inbox but it has a date -> Route to Calendar
                    let mut target_col = self.add_col;
                    if target_col == Col::Inbox && (start_d.is_some() || end_d.is_some()) {
                        target_col = Col::Scheduled;
                    }

                    let card = GtdTask {
                        id: self.next_id,
                        title: self.add_title.trim().to_string(),
                        description: self.add_desc.trim().to_string(),
                        start_date: start_d,
                        end_date: end_d,
                    };
                    self.next_id += 1;

                    self.dispatch(Command::Add { col: target_col, card });
                    self.add_open = false;
                }
            }

            Message::EditTitleChanged(val) => self.edit_title = val,
            Message::EditDescChanged(val) => self.edit_desc = val,
            Message::EditStartChanged(val) => self.edit_start_str = val,
            Message::EditEndChanged(val) => self.edit_end_str = val,
            Message::SubmitEdit => {
                if !self.edit_title.trim().is_empty() {
                    if let Some(idx) = self.columns[self.edit_col as usize].iter().position(|c| c.id == self.edit_id) {
                        let old_card = &self.columns[self.edit_col as usize][idx];
                        let cmd = Command::Edit {
                            id: self.edit_id,
                            col: self.edit_col,
                            old_title: old_card.title.clone(),
                            new_title: self.edit_title.trim().to_string(),
                            old_desc: old_card.description.clone(),
                            new_desc: self.edit_desc.trim().to_string(),
                            old_start: old_card.start_date,
                            new_start: NaiveDate::parse_from_str(&self.edit_start_str, "%Y-%m-%d").ok(),
                            old_end: old_card.end_date,
                            new_end: NaiveDate::parse_from_str(&self.edit_end_str, "%Y-%m-%d").ok(),
                        };
                        self.dispatch(cmd);
                    }
                    self.edit_open = false;
                }
            }

            // ── Process Flowchart ──────────────────────────────────────────
            Message::StartProcess(id) => {
                self.process_open = true;
                self.process_item_id = id;
                self.process_step = ProcessStep::IsActionable;
                self.process_date_input.clear();

                if let Some(item) = self.columns[Col::Inbox as usize].iter().find(|i| i.id == id) {
                    self.process_title_input = item.title.clone();
                }
            }
            Message::ProcessSetStep(step) => {
                self.process_step = step;
            }
            Message::ProcessTitleChanged(val) => self.process_title_input = val,
            Message::ProcessDateChanged(val) => self.process_date_input = val,
            Message::ProcessExecute(outcome) => {
                let id = self.process_item_id;
                let from_col = Col::Inbox;

                // Save the title change if it was edited during the wizard
                if let Some(idx) = self.columns[from_col as usize].iter().position(|c| c.id == id) {
                    let old_card = &self.columns[from_col as usize][idx];
                    if old_card.title != self.process_title_input {
                        let cmd = Command::Edit {
                            id, col: from_col,
                            old_title: old_card.title.clone(), new_title: self.process_title_input.clone(),
                            old_desc: old_card.description.clone(), new_desc: old_card.description.clone(),
                            old_start: old_card.start_date, new_start: old_card.start_date,
                            old_end: old_card.end_date, new_end: old_card.end_date,
                        };
                        self.dispatch(cmd);
                    }
                }

                // Execute dispatch based on diagram
                if let Some(from_idx) = self.columns[from_col as usize].iter().position(|c| c.id == id) {
                    match outcome {
                        ProcessOutcome::Trash | ProcessOutcome::DoIt => {
                            let card = self.columns[from_col as usize][from_idx].clone();
                            self.dispatch(Command::Delete { col: from_col, card, original_index: from_idx });
                        }
                        ProcessOutcome::Incubate => {
                            self.dispatch(Command::Move { id, from_col, to_col: Col::SomedayMaybe, from_idx, to_idx: self.columns[Col::SomedayMaybe as usize].len() });
                        }
                        ProcessOutcome::Reference => {
                            self.dispatch(Command::Move { id, from_col, to_col: Col::Reference, from_idx, to_idx: self.columns[Col::Reference as usize].len() });
                        }
                        ProcessOutcome::Project => {
                            self.dispatch(Command::Move { id, from_col, to_col: Col::Projects, from_idx, to_idx: self.columns[Col::Projects as usize].len() });
                        }
                        ProcessOutcome::Delegate => {
                            self.dispatch(Command::Move { id, from_col, to_col: Col::WaitingFor, from_idx, to_idx: self.columns[Col::WaitingFor as usize].len() });
                        }
                        ProcessOutcome::DeferASAP => {
                            self.dispatch(Command::Move { id, from_col, to_col: Col::NextActions, from_idx, to_idx: self.columns[Col::NextActions as usize].len() });
                        }
                        ProcessOutcome::DeferCalendar => {
                            // Update date before moving
                            let parsed_date = NaiveDate::parse_from_str(&self.process_date_input, "%Y-%m-%d").ok();
                            if let Some(date) = parsed_date {
                                let old_card = &self.columns[from_col as usize][from_idx];
                                self.dispatch(Command::Edit {
                                    id, col: from_col,
                                    old_title: old_card.title.clone(), new_title: old_card.title.clone(),
                                    old_desc: old_card.description.clone(), new_desc: old_card.description.clone(),
                                    old_start: old_card.start_date, new_start: Some(date),
                                    old_end: old_card.end_date, new_end: Some(date),
                                });
                            }
                            self.dispatch(Command::Move { id, from_col, to_col: Col::Scheduled, from_idx, to_idx: self.columns[Col::Scheduled as usize].len() });
                        }
                    }
                }
                self.process_open = false;
            }

            Message::MoveCard(id, from, to) => {
                if let Some(from_idx) = self.columns[from as usize].iter().position(|c| c.id == id) {
                    let to_idx = self.columns[to as usize].len();
                    self.dispatch(Command::Move { id, from_col: from, to_col: to, from_idx, to_idx });
                }
            }
            Message::DeleteCard(id, col) => {
                if let Some(idx) = self.columns[col as usize].iter().position(|c| c.id == id) {
                    let card = self.columns[col as usize][idx].clone();
                    self.dispatch(Command::Delete { col, card, original_index: idx });
                }
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

            Message::CardDropped(card_id, from_col, point, _rect) => {
                self.moving_card = Some(card_id);
                return iced_drop::zones_on_point(
                    move |zones| Message::HandleDropZones(card_id, from_col, zones),
                    point, None, None,
                );
            }
            Message::HandleDropZones(card_id, from_col, zones) => {
                self.moving_card = None;
                if let Some((zone_id, _)) = zones.first() {
                    let all_cols = [Col::Inbox, Col::NextActions, Col::Scheduled, Col::WaitingFor, Col::Projects, Col::SomedayMaybe, Col::Reference];
                    let to_col = all_cols.iter().find(|&&col| &self.col_ids[col as usize] == zone_id).copied();

                    if let Some(to_col) = to_col {
                        if from_col != to_col {
                            if let Some(from_idx) = self.columns[from_col as usize].iter().position(|c| c.id == card_id) {
                                let to_idx = self.columns[to_col as usize].len();
                                self.dispatch(Command::Move { id: card_id, from_col, to_col, from_idx, to_idx });
                            }
                        }
                    }
                }
            }

            Message::CalendarPrevMonth => self.calendar_view_month = add_months(self.calendar_view_month, -1),
            Message::CalendarNextMonth => self.calendar_view_month = add_months(self.calendar_view_month, 1),
            Message::CalendarToday => {
                let today = Local::now().date_naive();
                self.calendar_view_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<Message> {
        let theme = &self.theme;

        let toolbar = row![
            button("Undo", if self.undo_stack.is_empty() { None } else { Some(Message::Undo) }, ButtonProps::new().variant(ButtonVariant::Ghost).size(ButtonSize::Size1), theme),
            button("Redo", if self.redo_stack.is_empty() { None } else { Some(Message::Redo) }, ButtonProps::new().variant(ButtonVariant::Ghost).size(ButtonSize::Size1), theme),
        ].spacing(8).align_y(Alignment::Center);

        let list = tabs_list(
            vec![
                tabs_trigger("board", "GTD Dashboard"),
                tabs_trigger("timeline", "Timeline"),
                tabs_trigger("calendar", "Calendar"),
                tabs_trigger("table", "Table"),
                tabs_trigger("issues", "List View"),
            ],
            &self.active_tab,
            Some(Message::TabSelected),
            TabsRootProps::new(),
            TabsListProps::new().variant(TabsListVariant::Pill).transparent_container(true).hover(TabsHover::Soft),
            theme,
        );

        let quick_dump_ui = row![
            container(input(&self.quick_dump_text, "Dump idea (e.g. 'Call mom tmr')", Some(Message::QuickDumpChanged), InputProps::new(), theme)).width(Length::Fixed(350.0)),
            button("Quick Dump", Some(Message::SubmitQuickDump), ButtonProps::new().variant(ButtonVariant::Solid).size(ButtonSize::Size1), theme)
        ].spacing(8).align_y(Alignment::Center);

        let content = tabs_contents(
            vec![
                tabs_content("board", self.view_board(theme)),
                tabs_content("timeline", self.view_timeline(theme)),
                tabs_content("calendar", self.view_calendar(theme)),
                tabs_content("table", self.view_table(theme)),
                tabs_content("issues", self.view_issues(theme)),
            ],
            &self.active_tab,
        );

        let main_view = column![
            row![list, Space::new().width(Length::Fill), quick_dump_ui, Space::new().width(Length::Fill), toolbar]
                .align_y(Alignment::Center)
                .width(Length::Fill),
            content,
        ]
        .spacing(24)
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill);

        let mut app_ui: Element<Message> = container(main_view)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_t| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgb8(1, 4, 9))),
                text_color: Some(Color::WHITE),
                ..Default::default()
            })
            .into();

        app_ui = dialog(app_ui, self.add_open, self.view_add_dialog(theme), Message::CloseDialogs, DialogProps::new().align(DialogAlign::Center), theme);
        app_ui = dialog(app_ui, self.edit_open, self.view_edit_dialog(theme), Message::CloseDialogs, DialogProps::new().align(DialogAlign::Center), theme);
        app_ui = dialog(app_ui, self.process_open, self.view_process_dialog(theme), Message::CloseDialogs, DialogProps::new().align(DialogAlign::Center), theme);

        app_ui
    }

    // ── Views ────────────────────────────────────────────────────────────────

    fn view_board<'a>(&self, theme: &'a Theme) -> Element<'a, Message> {
        let all_cols = [
            Col::Inbox, Col::NextActions, Col::Scheduled, Col::WaitingFor,
            Col::Projects, Col::SomedayMaybe, Col::Reference
        ];

        let mut columns_ui = Vec::new();

        for col_id in all_cols {
            let color = col_color(col_id);
            let title = col_name(col_id);
            let cards = &self.columns[col_id as usize];

            let mut cards_ui = Vec::new();
            for kcard in cards {
                if self.moving_card == Some(kcard.id) { continue; }

                let cid = kcard.id;
                let ctitle = kcard.title.clone();
                let cdesc = kcard.description.clone();
                let cstart = kcard.start_date;
                let cend = kcard.end_date;

                let mut card_col = column![text(ctitle.clone()).size(14).style(
                    move |_| iced::widget::text::Style { color: Some(Color::WHITE) }
                )].spacing(8);

                if !cdesc.is_empty() {
                    card_col = card_col.push(text(cdesc.clone()).size(12).style(
                        move |_| iced::widget::text::Style { color: Some(Color::from_rgb8(100, 100, 120)) }
                    ));
                }

                // Show due date if exists
                if let Some(d) = cend {
                    card_col = card_col.push(text(format!("Date: {}", d.format("%b %d"))).size(11).style(
                        move |_| iced::widget::text::Style { color: Some(Color::from_rgb8(234, 179, 8)) }
                    ));
                }

                let mut actions_row = row![].spacing(4);

                // If in Inbox, add big PROCESS button
                if col_id == Col::Inbox {
                    actions_row = actions_row.push(button("PROCESS ITEM", Some(Message::StartProcess(cid)), ButtonProps::new().variant(ButtonVariant::Solid).size(ButtonSize::Size1), theme));
                }

                actions_row = actions_row.push(Space::new().width(Length::Fill));

                actions_row = actions_row.push(button("X", Some(Message::DeleteCard(cid, col_id)), ButtonProps::new().variant(ButtonVariant::Ghost).size(ButtonSize::Size1), theme));
                actions_row = actions_row.push(button("Edit", Some(Message::OpenEdit(cid, col_id, ctitle.clone(), cdesc, cstart, cend)), ButtonProps::new().variant(ButtonVariant::Ghost).size(ButtonSize::Size1), theme));

                card_col = card_col.push(actions_row);

                cards_ui.push(
                    droppable(
                        card(card_col, CardProps::new().variant(CardVariant::Surface), theme).padding(14).width(Length::Fill),
                    )
                    .on_drop(move |point, rect| Message::CardDropped(cid, col_id, point, rect))
                    .drag_hide(true)
                    .into(),
                );
            }

            let header = row![
                container(text("")).width(Length::Fixed(12.0)).height(Length::Fixed(12.0)).style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(color)),
                    border: iced::border::Border { radius: 6.0.into(), ..Default::default() },
                    ..Default::default()
                }),
                text(title).size(15),
                text(cards.len().to_string()).size(13).style(move |_| iced::widget::text::Style { color: Some(Color::from_rgb8(100, 100, 120)) }),
                Space::new().width(Length::Fill),
                button("+", Some(Message::OpenAdd(col_id, None)), ButtonProps::new().variant(ButtonVariant::Ghost).size(ButtonSize::Size1), theme),
            ].align_y(Alignment::Center).spacing(8);

            let col_content = column![
                header,
                scrollable(column(cards_ui).spacing(10)).height(Length::Fill),
                button("+ Add to list", Some(Message::OpenAdd(col_id, None)), ButtonProps::new().variant(ButtonVariant::Ghost).size(ButtonSize::Size1), theme).width(Length::Fill)
            ].spacing(14).width(Length::Fixed(300.0));

            let zone_id = self.col_ids[col_id as usize].clone();
            columns_ui.push(
                container(col_content).id(zone_id).padding(18).height(Length::Fill).style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgb8(13, 17, 23))),
                    border: iced::border::Border { color: Color::from_rgb8(48, 54, 61), width: 1.0, radius: 10.0.into() },
                    ..Default::default()
                }).into(),
            );
        }

        scrollable(row(columns_ui).spacing(18).height(Length::Fill))
            .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new()))
            .into()
    }

    fn view_timeline<'a>(&self, theme: &'a Theme) -> Element<'a, Message> {
        let mut all_cards = Vec::new();
        for col_id in [Col::Projects, Col::NextActions, Col::Scheduled, Col::WaitingFor] {
            for card in &self.columns[col_id as usize] {
                all_cards.push((col_id, card.clone()));
            }
        }

        let canvas = canvas::Canvas::new(TimelineProgram { tasks: all_cards, theme: theme.clone() }).width(Length::Fixed(1600.0)).height(Length::Fixed(600.0));
        scrollable(canvas).direction(scrollable::Direction::Both { vertical: scrollable::Scrollbar::new(), horizontal: scrollable::Scrollbar::new() }).width(Length::Fill).height(Length::Fill).into()
    }

    fn view_calendar<'a>(&self, theme: &'a Theme) -> Element<'a, Message> {
        let mut all_cards = Vec::new();
        for col_id in [Col::Scheduled, Col::Projects, Col::NextActions, Col::WaitingFor] {
            for card in &self.columns[col_id as usize] {
                if card.start_date.is_some() || card.end_date.is_some() {
                    all_cards.push((col_id, card.clone()));
                }
            }
        }

        let header = row![
            button("Today", Some(Message::CalendarToday), ButtonProps::new().variant(ButtonVariant::Outline), theme),
            Space::new().width(8.0),
            button("<", Some(Message::CalendarPrevMonth), ButtonProps::new().variant(ButtonVariant::Outline), theme),
            button(">", Some(Message::CalendarNextMonth), ButtonProps::new().variant(ButtonVariant::Outline), theme),
            Space::new().width(16.0),
            text(self.calendar_view_month.format("%B %Y").to_string()).size(22),
        ].align_y(Alignment::Center);

        let canvas = canvas::Canvas::new(CalendarProgram { tasks: all_cards, view_month: self.calendar_view_month, theme: theme.clone() }).width(Length::Fill).height(Length::Fill);
        column![header, canvas].spacing(16).height(Length::Fill).into()
    }

    fn view_table<'a>(&'a self, theme: &'a Theme) -> Element<'a, Message> {
        let mut manual_table = column![
            row![
                container(strong_text("ID")).width(Length::Fixed(60.0)),
                container(strong_text("Title")).width(Length::Fixed(300.0)),
                container(strong_text("List")).width(Length::Fixed(150.0)),
                container(strong_text("Due Date")).width(Length::Fill),
            ].padding([12, 16]),
            separator(SeparatorProps::new(), theme),
        ];

        let all_cols = [Col::Inbox, Col::NextActions, Col::Scheduled, Col::WaitingFor, Col::Projects, Col::SomedayMaybe, Col::Reference];

        for col_id in all_cols {
            for card in &self.columns[col_id as usize] {
                let badge_content = container(text(col_name(col_id)).size(11)).padding([2, 8]).style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(apply_opacity(col_color(col_id), 0.15))),
                    border: iced::border::Border { color: apply_opacity(col_color(col_id), 0.5), width: 1.0, radius: 6.0.into() },
                    ..Default::default()
                });

                manual_table = manual_table.push(
                    row![
                        container(text(card.id.to_string())).width(Length::Fixed(60.0)),
                        container(text(&card.title)).width(Length::Fixed(300.0)),
                        container(badge_content).width(Length::Fixed(150.0)),
                        container(text(card.end_date.map_or("-".to_string(), |d| d.format("%Y-%m-%d").to_string()))).width(Length::Fill),
                    ].padding([12, 16]).align_y(Alignment::Center),
                );
                manual_table = manual_table.push(separator(SeparatorProps::new(), theme));
            }
        }

        container(scrollable(manual_table).height(Length::Fill)).style(move |_| iced::widget::container::Style {
            border: iced::border::Border { color: theme.palette.border, width: 1.0, radius: 6.0.into() },
            ..Default::default()
        }).into()
    }

    fn view_issues<'a>(&self, theme: &'a Theme) -> Element<'a, Message> {
        let mut all_cards = Vec::new();
        for col_id in [Col::Inbox, Col::NextActions, Col::Scheduled, Col::WaitingFor, Col::Projects, Col::SomedayMaybe, Col::Reference] {
            for card in &self.columns[col_id as usize] {
                all_cards.push((col_id, card.clone()));
            }
        }
        all_cards.sort_by_key(|c| c.1.id);

        let mut issues_col = column![].spacing(0);
        for (col, card) in all_cards {
            let icon_color = col_color(col);

            let badge_content = container(text(col_name(col)).size(11)).padding([2, 8]).style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(apply_opacity(col_color(col), 0.15))),
                border: iced::border::Border { color: apply_opacity(col_color(col), 0.5), width: 1.0, radius: 6.0.into() },
                ..Default::default()
            });

            let row_content = row![
                text("●").color(icon_color).size(18),
                column![
                    row![strong_text(&card.title).size(16), badge_content].spacing(8).align_y(Alignment::Center),
                    text(format!("GTD Item #{}", card.id)).size(12).style(move |_| iced::widget::text::Style { color: Some(theme.palette.muted_foreground) })
                ].spacing(4).width(Length::Fill),
            ].spacing(12).padding(16).align_y(Alignment::Start);

            issues_col = issues_col.push(row_content);
            issues_col = issues_col.push(separator(SeparatorProps::new(), theme));
        }

        scrollable(container(issues_col).max_width(900.0)).into()
    }

    // ── Dialogs ────────────────────────────────────────────────────────────────

    fn view_add_dialog<'a>(&'a self, theme: &'a Theme) -> Element<'a, Message> {
        let form = column![
            column![
                text("Title").size(14),
                input(&self.add_title, "Task title / Dump Idea", Some(Message::AddTitleChanged), InputProps::new(), theme),
            ].spacing(6),
            column![
                text("Description").size(14),
                input(&self.add_desc, "Optional details", Some(Message::AddDescChanged), InputProps::new(), theme),
            ].spacing(6),
            row![
                column![
                    text("Start Date").size(14),
                    input(&self.add_start_str, "YYYY-MM-DD", Some(Message::AddStartChanged), InputProps::new(), theme),
                ].spacing(6).width(Length::FillPortion(1)),
                column![
                    text("End Date").size(14),
                    input(&self.add_end_str, "YYYY-MM-DD", Some(Message::AddEndChanged), InputProps::new(), theme),
                ].spacing(6).width(Length::FillPortion(1)),
            ].spacing(16),
            row![
                Space::new().width(Length::Fill),
                button("Cancel", Some(Message::CloseDialogs), ButtonProps::new().variant(ButtonVariant::Outline), theme),
                button("Add to List", Some(Message::SubmitAdd), ButtonProps::new().variant(ButtonVariant::Solid), theme),
            ].spacing(8)
        ].spacing(16);

        container(column![strong_text(format!("Add to {}", col_name(self.add_col)).as_str()).size(18), form].spacing(16))
            .width(Length::Fixed(450.0))
            .padding(24)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(theme.palette.card)),
                border: iced::border::Border { radius: 8.0.into(), width: 1.0, color: theme.palette.border },
                ..Default::default()
            })
            .into()
    }

    fn view_edit_dialog<'a>(&'a self, theme: &'a Theme) -> Element<'a, Message> {
        let form = column![
            column![
                text("Title").size(14),
                input(&self.edit_title, "Task title", Some(Message::EditTitleChanged), InputProps::new(), theme),
            ].spacing(6),
            column![
                text("Description").size(14),
                input(&self.edit_desc, "Optional description", Some(Message::EditDescChanged), InputProps::new(), theme),
            ].spacing(6),
            row![
                column![
                    text("Start Date").size(14),
                    input(&self.edit_start_str, "YYYY-MM-DD", Some(Message::EditStartChanged), InputProps::new(), theme),
                ].spacing(6).width(Length::FillPortion(1)),
                column![
                    text("End Date").size(14),
                    input(&self.edit_end_str, "YYYY-MM-DD", Some(Message::EditEndChanged), InputProps::new(), theme),
                ].spacing(6).width(Length::FillPortion(1)),
            ].spacing(16),
            row![
                Space::new().width(Length::Fill),
                button("Cancel", Some(Message::CloseDialogs), ButtonProps::new().variant(ButtonVariant::Outline), theme),
                button("Save", Some(Message::SubmitEdit), ButtonProps::new().variant(ButtonVariant::Solid), theme),
            ].spacing(8)
        ].spacing(16);

        container(column![strong_text("Edit Item").size(18), form].spacing(16))
            .width(Length::Fixed(450.0))
            .padding(24)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(theme.palette.card)),
                border: iced::border::Border { radius: 8.0.into(), width: 1.0, color: theme.palette.border },
                ..Default::default()
            })
            .into()
    }

    fn view_process_dialog<'a>(&'a self, theme: &'a Theme) -> Element<'a, Message> {
        let title_edit = column![
            text("Item Description:").size(14).style(move |_| iced::widget::text::Style { color: Some(theme.palette.muted_foreground) }),
            input(&self.process_title_input, "Edit item if needed...", Some(Message::ProcessTitleChanged), InputProps::new(), theme)
        ].spacing(6);

        let content = match self.process_step {
            ProcessStep::IsActionable => column![
                strong_text("What is it? Is it Actionable?").size(24),
                title_edit,
                row![
                    button("NO", Some(Message::ProcessSetStep(ProcessStep::NonActionableDest)), ButtonProps::new().variant(ButtonVariant::Outline).size(ButtonSize::Size1), theme).width(Length::Fill),
                    button("YES", Some(Message::ProcessSetStep(ProcessStep::IsMultiStep)), ButtonProps::new().variant(ButtonVariant::Solid).size(ButtonSize::Size1), theme).width(Length::Fill),
                ].spacing(16)
            ].spacing(24),

            ProcessStep::NonActionableDest => column![
                strong_text("Not Actionable. What to do with it?").size(24),
                button("Trash (Eliminate)", Some(Message::ProcessExecute(ProcessOutcome::Trash)), ButtonProps::new().variant(ButtonVariant::Destructive).size(ButtonSize::Size1), theme).width(Length::Fill),
                button("Incubate (Someday / Maybe)", Some(Message::ProcessExecute(ProcessOutcome::Incubate)), ButtonProps::new().variant(ButtonVariant::Outline).size(ButtonSize::Size1), theme).width(Length::Fill),
                button("Keep as Reference", Some(Message::ProcessExecute(ProcessOutcome::Reference)), ButtonProps::new().variant(ButtonVariant::Outline).size(ButtonSize::Size1), theme).width(Length::Fill),
            ].spacing(16),

            ProcessStep::IsMultiStep => column![
                strong_text("Is it multi-step?").size(24),
                title_edit,
                button("YES, requires multiple actions (Convert to Project)", Some(Message::ProcessExecute(ProcessOutcome::Project)), ButtonProps::new().variant(ButtonVariant::Outline).size(ButtonSize::Size1), theme).width(Length::Fill),
                button("NO, it's a single physical action", Some(Message::ProcessSetStep(ProcessStep::NextActionDefinition)), ButtonProps::new().variant(ButtonVariant::Solid).size(ButtonSize::Size1), theme).width(Length::Fill),
            ].spacing(16),

            ProcessStep::NextActionDefinition => column![
                strong_text("What is the Next Action?").size(24),
                title_edit,
                strong_text("Will it take less than 2 minutes?").size(18),
                row![
                    button("YES (Do It Now!)", Some(Message::ProcessExecute(ProcessOutcome::DoIt)), ButtonProps::new().variant(ButtonVariant::Solid).size(ButtonSize::Size1), theme).width(Length::Fill),
                    button("NO (Takes longer)", Some(Message::ProcessSetStep(ProcessStep::DelegateOrDefer)), ButtonProps::new().variant(ButtonVariant::Outline).size(ButtonSize::Size1), theme).width(Length::Fill),
                ].spacing(16)
            ].spacing(24),

            ProcessStep::DelegateOrDefer => column![
                strong_text("Delegate it or Defer it?").size(24),
                button("Delegate It (Waiting For...)", Some(Message::ProcessExecute(ProcessOutcome::Delegate)), ButtonProps::new().variant(ButtonVariant::Outline).size(ButtonSize::Size1), theme).width(Length::Fill),
                button("Defer It (I will do it later)", Some(Message::ProcessSetStep(ProcessStep::DeferDest)), ButtonProps::new().variant(ButtonVariant::Solid).size(ButtonSize::Size1), theme).width(Length::Fill),
            ].spacing(16),

            ProcessStep::DeferDest => column![
                strong_text("How to defer?").size(24),
                button("As Soon As I Can (Next Actions)", Some(Message::ProcessExecute(ProcessOutcome::DeferASAP)), ButtonProps::new().variant(ButtonVariant::Solid).size(ButtonSize::Size1), theme).width(Length::Fill),
                separator(SeparatorProps::new(), theme),
                text("Specific day or time (Calendar):"),
                row![
                    input(&self.process_date_input, "YYYY-MM-DD", Some(Message::ProcessDateChanged), InputProps::new(), theme).width(Length::FillPortion(2)),
                    button("Schedule", Some(Message::ProcessExecute(ProcessOutcome::DeferCalendar)), ButtonProps::new().variant(ButtonVariant::Outline), theme).width(Length::FillPortion(1)),
                ].spacing(8)
            ].spacing(16),
        };

        container(column![
            row![strong_text("Workflow Process").size(16).style(move |_| iced::widget::text::Style { color: Some(theme.palette.primary) }), Space::new().width(Length::Fill), button("X", Some(Message::CloseDialogs), ButtonProps::new().variant(ButtonVariant::Ghost), theme)],
            content
        ].spacing(16))
        .width(Length::Fixed(500.0))
        .padding(24)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(theme.palette.card)),
            border: iced::border::Border { radius: 8.0.into(), width: 1.0, color: theme.palette.border },
            ..Default::default()
        })
        .into()
    }
}

// ── Canvas Programs ───────────────────────────────────────────────────────────

struct TimelineProgram {
    tasks: Vec<(Col, GtdTask)>,
    theme: Theme,
}

impl canvas::Program<Message> for TimelineProgram {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let num_days = 31;
        let day_width = 44.0;
        let row_height = 48.0;
        let label_width = 240.0;
        let header_height = 36.0;
        let base_date = Local::now().date_naive() - Duration::days(5);

        for day in 0..num_days {
            let x = label_width + (day as f32 * day_width);
            let display_date = base_date + Duration::days(day);

            frame.fill_text(canvas::Text {
                content: display_date.format("%d %b").to_string(),
                position: Point::new(x + day_width / 2.0, header_height / 2.0),
                color: Color::from_rgb8(100, 100, 120),
                size: iced::Pixels(12.0),
                align_x: iced::widget::text::Alignment::Center,
                align_y: iced::alignment::Vertical::Center,
                ..Default::default()
            });

            frame.stroke(
                &Path::line(Point::new(x, 0.0), Point::new(x, bounds.height)),
                Stroke::default()
                    .with_width(1.0)
                    .with_color(Color::from_rgb8(25, 30, 38)),
            );
        }

        for (i, (col, card)) in self.tasks.iter().enumerate() {
            let y = header_height + (i as f32 * row_height);

            frame.stroke(
                &Path::line(Point::new(0.0, y), Point::new(bounds.width, y)),
                Stroke::default()
                    .with_width(1.0)
                    .with_color(Color::from_rgb8(35, 40, 48)),
            );

            frame.fill_text(canvas::Text {
                content: card.title.clone(),
                position: Point::new(10.0, y + row_height / 2.0),
                color: Color::WHITE,
                size: iced::Pixels(14.0),
                align_x: iced::alignment::Horizontal::Left.into(),
                align_y: iced::alignment::Vertical::Center,
                ..Default::default()
            });

            let start_offset = card.start_date.map(|d| (d - base_date).num_days() as i32).unwrap_or(0).max(0);
            let end_offset = card.end_date.map(|d| (d - base_date).num_days() as i32).unwrap_or(start_offset + 1).max(start_offset);

            let start_x = label_width + (start_offset as f32 * day_width);
            let end_x = label_width + (end_offset as f32 * day_width) + day_width;

            let pill_rect = Rectangle::new(
                Point::new(start_x, y + 10.0),
                Size::new(end_x - start_x, row_height - 20.0),
            );

            frame.fill(
                &Path::rectangle(pill_rect.position(), pill_rect.size()),
                col_color(*col),
            );
        }

        vec![frame.into_geometry()]
    }
}

struct CalendarProgram {
    tasks: Vec<(Col, GtdTask)>,
    view_month: NaiveDate,
    theme: Theme,
}

impl canvas::Program<Message> for CalendarProgram {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let pointer = cursor.position_in(bounds)?;

        if let canvas::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) = event {
            let header_h = 32.0;
            let col_w = bounds.width / 7.0;
            let row_h = (bounds.height - header_h) / 6.0;

            let grid_top = header_h;
            let days_from_sun = self.view_month.weekday().num_days_from_sunday();
            let start_date = self.view_month - Duration::days(days_from_sun as i64);

            for row in 0..6 {
                let week_start = start_date + Duration::days((row * 7) as i64);
                let week_end = week_start + Duration::days(6);

                let mut current_week_events: Vec<(Col, &GtdTask, NaiveDate, NaiveDate)> = Vec::new();
                for (col, card) in &self.tasks {
                    if let Some(s) = card.start_date.or(card.end_date) {
                        let e = card.end_date.unwrap_or(s);
                        if s <= week_end && e >= week_start {
                            current_week_events.push((*col, card, s.max(week_start), e.min(week_end)));
                        }
                    }
                }
                current_week_events.sort_by_key(|(_, _, cs, ce)| (-((*ce - *cs).num_days()), *cs));

                let mut tracks: Vec<NaiveDate> = Vec::new();
                for (col, card, cs, ce) in current_week_events {
                    let mut assigned_track = 0;
                    loop {
                        if assigned_track >= tracks.len() {
                            tracks.push(ce);
                            break;
                        } else if cs > tracks[assigned_track] {
                            tracks[assigned_track] = ce;
                            break;
                        }
                        assigned_track += 1;
                    }

                    let start_col = (cs - week_start).num_days() as f32;
                    let span = (ce - cs).num_days() as f32 + 1.0;

                    let pill_x = col_w * start_col + 2.0;
                    let pill_y = grid_top + row_h * (row as f32) + 28.0 + (assigned_track as f32 * 20.0);
                    let pill_w = col_w * span - 4.0;
                    let pill_h = 18.0;

                    let pill_rect = Rectangle::new(Point::new(pill_x, pill_y), Size::new(pill_w, pill_h));

                    if pill_rect.contains(pointer) {
                        return Some(canvas::Action::publish(Message::OpenEdit(
                            card.id, col, card.title.clone(), card.description.clone(), card.start_date, card.end_date,
                        )));
                    }
                }

                for col in 0..7 {
                    let cell_date = week_start + Duration::days(col as i64);
                    let x = col_w * (col as f32);
                    let y = grid_top + row_h * (row as f32);
                    let cell_rect = Rectangle::new(Point::new(x, y), Size::new(col_w, row_h));

                    if cell_rect.contains(pointer) {
                        return Some(canvas::Action::publish(Message::OpenAdd(
                            Col::Scheduled, Some(cell_date),
                        )));
                    }
                }
            }
        }
        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let header_h = 32.0;
        let col_w = bounds.width / 7.0;
        let row_h = (bounds.height - header_h) / 6.0;

        let weekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        for (i, day) in weekdays.iter().enumerate() {
            let x = col_w * (i as f32);
            frame.fill_text(canvas::Text {
                content: day.to_string(),
                position: Point::new(x + col_w / 2.0, header_h / 2.0),
                color: self.theme.palette.muted_foreground,
                size: iced::Pixels(14.0),
                align_x: iced::widget::text::Alignment::Center,
                align_y: iced::alignment::Vertical::Center,
                ..Default::default()
            });
        }

        let grid_top = header_h;
        let days_from_sun = self.view_month.weekday().num_days_from_sunday();
        let start_date = self.view_month - Duration::days(days_from_sun as i64);
        let today = Local::now().date_naive();

        for row in 0..6 {
            let week_start = start_date + Duration::days((row * 7) as i64);
            let week_end = week_start + Duration::days(6);

            for col in 0..7 {
                let cell_date = week_start + Duration::days(col as i64);
                let x = col_w * (col as f32);
                let y = grid_top + row_h * (row as f32);

                frame.stroke(
                    &Path::rectangle(Point::new(x, y), Size::new(col_w, row_h)),
                    Stroke::default().with_width(1.0).with_color(self.theme.palette.border),
                );

                let label_text = if cell_date.day() == 1 { cell_date.format("%b %-d").to_string() } else { cell_date.format("%-d").to_string() };
                let is_current_month = cell_date.month() == self.view_month.month();

                if cell_date == today {
                    frame.fill(&Path::circle(Point::new(x + col_w / 2.0, y + 16.0), 12.0), self.theme.palette.primary);
                    frame.fill_text(canvas::Text {
                        content: label_text, position: Point::new(x + col_w / 2.0, y + 16.0), color: self.theme.palette.primary_foreground,
                        size: iced::Pixels(13.0), align_x: iced::widget::text::Alignment::Center, align_y: iced::alignment::Vertical::Center, ..Default::default()
                    });
                } else {
                    let color = if is_current_month { self.theme.palette.foreground } else { self.theme.palette.muted_foreground };
                    frame.fill_text(canvas::Text {
                        content: label_text, position: Point::new(x + col_w / 2.0, y + 16.0), color,
                        size: iced::Pixels(13.0), align_x: iced::widget::text::Alignment::Center, align_y: iced::alignment::Vertical::Center, ..Default::default()
                    });
                }
            }

            let mut current_week_events: Vec<(Col, &GtdTask, NaiveDate, NaiveDate, NaiveDate, NaiveDate)> = Vec::new();
            for (col, card) in &self.tasks {
                if let Some(s) = card.start_date.or(card.end_date) {
                    let e = card.end_date.unwrap_or(s);
                    if s <= week_end && e >= week_start {
                        current_week_events.push((*col, card, s.max(week_start), e.min(week_end), s, e));
                    }
                }
            }
            current_week_events.sort_by_key(|(_, _, cs, ce, _, _)| (-((*ce - *cs).num_days()), *cs));

            let mut tracks: Vec<NaiveDate> = Vec::new();
            for (col, card, cs, ce, _orig_s, _orig_e) in current_week_events {
                let mut assigned_track = 0;
                loop {
                    if assigned_track >= tracks.len() { tracks.push(ce); break; }
                    else if cs > tracks[assigned_track] { tracks[assigned_track] = ce; break; }
                    assigned_track += 1;
                }

                let start_col = (cs - week_start).num_days() as f32;
                let span = (ce - cs).num_days() as f32 + 1.0;

                let pill_x = col_w * start_col + 2.0;
                let pill_y = grid_top + row_h * (row as f32) + 28.0 + (assigned_track as f32 * 20.0);
                let pill_w = col_w * span - 4.0;
                let pill_h = 18.0;

                if pill_y + pill_h < grid_top + row_h * (row as f32) + row_h {
                    let bg_color = col_color(col);

                    frame.fill(&Path::rectangle(Point::new(pill_x, pill_y), Size::new(pill_w, pill_h)), apply_opacity(bg_color, 0.3));

                    frame.fill_text(canvas::Text {
                        content: card.title.clone(), position: Point::new(pill_x + 4.0, pill_y + pill_h / 2.0),
                        color: bg_color, size: iced::Pixels(12.0), align_x: iced::alignment::Horizontal::Left.into(), align_y: iced::alignment::Vertical::Center, ..Default::default()
                    });
                }
            }
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> iced::mouse::Interaction {
        if cursor.is_over(bounds) { iced::mouse::Interaction::Pointer } else { iced::mouse::Interaction::default() }
    }
}