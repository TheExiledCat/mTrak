use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

use ratatui::{
    DefaultTerminal,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
};

use crate::data::{pattern::Pattern, project::Project};

use super::{
    constants,
    keymap::InputHandler,
    views::{header::Header, sidebar::SideBar, timeline::TimeLineView},
};

pub struct App {
    pub state: Rc<RefCell<AppState>>,
    pub terminal: DefaultTerminal,
    pub input_handler: InputHandler,
    pub fps: u16,
    pub main_view: MainView,
}
pub struct AppState {
    pub project: Project,
    pub selected_pattern_index: usize,
    /// render cache for storing pattern channels efficiently, maps pattern IDs to Channels
    pub channel_cache: ChannelCache,
    pub row_index: usize,
    pub column_index: usize,
    pub channel_index: usize,
    pub is_editing: bool,
    pub row_number_lookup: String,
}

impl AppState {
    pub fn new(project: Project) -> Self {
        let patterns = &project.patterns;
        let channel_cache = ChannelCache::new(&patterns);
        return Self {
            project,
            selected_pattern_index: 0,
            channel_cache,
            row_index: 0,
            channel_index: 0,
            column_index: 0,
            is_editing: false,
            row_number_lookup: String::new(),
        };
    }
}
pub struct ChannelCache {
    patterns: HashMap<usize, Vec<ChannelCacheRow>>,
}

pub struct ChannelCacheRow {
    pub channels: [Line<'static>; constants::CHANNEL_COUNT],
}
impl<'a> ChannelCache {
    pub fn new(patterns: &[Pattern]) -> Self {
        let mut map = HashMap::<usize, Vec<ChannelCacheRow>>::new();

        for (i, pattern) in patterns.iter().enumerate() {
            let mut rows = Vec::new();
            for row in &pattern.rows {
                let channels: [Line<'static>; constants::CHANNEL_COUNT] = row
                    .channels
                    .iter()
                    .map(|c| Line::from(c.to_string()))
                    .collect::<Vec<Line<'static>>>()
                    .try_into()
                    .unwrap();
                rows.push(ChannelCacheRow { channels })
            }
            map.insert(i, rows);
        }
        let cache = ChannelCache { patterns: map };

        return cache;
    }
    pub fn update_all_dirty(&mut self, patterns: &[Pattern]) {
        for (i, pattern) in patterns.iter().enumerate() {
            for row_index in 0..pattern.row_count {
                let row = &pattern.rows[row_index];
                if !row.dirty {
                    continue;
                }
                self.patterns.get_mut(&i).unwrap()[row_index].channels = row
                    .channels
                    .iter()
                    .map(|c| Line::from(c.to_string()))
                    .collect::<Vec<Line<'static>>>()
                    .try_into()
                    .unwrap();
            }
        }
    }
    pub fn update_dirty(&mut self, pattern: &mut Pattern, pattern_index: usize, row_index: usize) {
        let row = pattern.rows.get_mut(row_index).unwrap();

        if !row.dirty {
            return;
        }
        row.dirty = false;
        self.patterns.get_mut(&pattern_index).unwrap()[row_index].channels = row
            .channels
            .iter()
            .map(|c| Line::from(c.to_string()))
            .collect::<Vec<Line<'static>>>()
            .try_into()
            .unwrap();
    }
    pub fn get_row(&'a self, pattern_index: usize, row_index: usize) -> &'a ChannelCacheRow {
        return &self.patterns.get(&pattern_index).unwrap()[row_index];
    }
    pub fn get_all_rows_for_channel(
        &'a self,
        pattern_index: usize,
        channel_index: usize,
    ) -> Vec<Line<'a>> {
        return self
            .patterns
            .get(&pattern_index)
            .unwrap()
            .iter()
            .map(|r| r.channels[channel_index].clone())
            .collect();
    }
    pub fn get_row_mut(
        &'a mut self,
        pattern_index: usize,
        row_index: usize,
    ) -> &'a mut ChannelCacheRow {
        return self
            .patterns
            .get_mut(&pattern_index)
            .unwrap()
            .get_mut(row_index)
            .unwrap();
    }
}
pub enum MainView {
    Timeline,
}
pub enum AppLayout {
    RightBar,
    LeftBar,
}
impl AppLayout {
    pub fn build(&self, area: Rect) -> Vec<Rect> {
        match self {
            AppLayout::RightBar => {
                let root = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
                    .split(area);
                let mut left_area = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
                    .split(root[0])
                    .to_vec();
                let rightbar = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(100)])
                    .split(root[1])
                    .to_vec();
                left_area.extend(rightbar);

                left_area
            }
            AppLayout::LeftBar => todo!(),
        }
    }
}
fn all_map(state: Rc<RefCell<AppState>>, event: &KeyEvent) -> bool {
    let row_count =
        state.borrow().project.patterns[state.borrow().selected_pattern_index].row_count;

    match event.code {
        KeyCode::Char('s') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            let project_name = state.borrow().project.name.clone();
            let mut state = state.borrow_mut();
            let saved = state.project.is_saved();
            if saved {
                state.project.save(project_name).unwrap();
            } else {
                //TODO SAVING LOGIC
            }
        }
        KeyCode::Up => {
            if state.borrow().row_index > 0 {
                state.borrow_mut().row_index -= 1;
            }
        }
        KeyCode::Down => {
            if state.borrow().row_index < row_count - 1 {
                state.borrow_mut().row_index += 1;
            }
        }
        KeyCode::Left => {
            if !state.borrow().is_editing {
                let new_index = state.borrow().channel_index.saturating_sub(1);
                state.borrow_mut().channel_index = new_index;
            } else {
                let new_index = state.borrow().column_index.saturating_sub(1);
                state.borrow_mut().column_index = new_index;
            }
        }
        KeyCode::Right => {
            if !(state.borrow().is_editing) {
                let new_index = state.borrow().channel_index + 1;
                if new_index < constants::CHANNEL_COUNT {
                    state.borrow_mut().channel_index = new_index;
                }
            } else {
                let new_index = state.borrow().column_index + 1;
                if new_index < constants::CHANNEL_COLUMN_COUNT {
                    state.borrow_mut().column_index = new_index;
                }
            }
        }
        KeyCode::Char(' ') => {
            let editing = !state.borrow().is_editing;
            state.borrow_mut().is_editing = editing;
        }
        _ => return false,
    }

    return true;
}
impl App {
    pub fn new(terminal: DefaultTerminal, fps: u16, project: Project) -> Self {
        let mut input_handler = InputHandler::new();
        input_handler.register_handler(all_map);
        return Self {
            terminal,
            input_handler,
            fps,
            main_view: MainView::Timeline,
            state: Rc::new(RefCell::new(AppState::new(project))),
        };
    }
    pub fn draw(&mut self) -> bool {
        let mut render_next = true;
        let frame_time = Duration::from_secs_f64(1.0 / self.fps as f64);
        let start = Instant::now();
        let header = Header::new(self.state.clone());
        let sidebar = SideBar::new(self.state.clone());
        let timeline = TimeLineView::new(self.state.clone());
        let row_count = self.state.borrow().project.patterns
            [self.state.borrow().selected_pattern_index]
            .row_count;
        self.terminal
            .draw(|f| {
                if let Some(event) = self.input_handler.read_event() {
                    // GLOBAL MAPPINGS
                    if let KeyCode::Char('q') = event.code
                        && event.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        render_next = false;
                        return;
                    }
                    let mut was_row_number = false;
                    self.input_handler.handle_event(self.state.clone(), event);

                    match event.code {
                        KeyCode::Char(num)
                            if num.is_ascii_hexdigit() && !self.state.borrow().is_editing =>
                        {
                            let num_char = num.to_ascii_uppercase();
                            self.state.borrow_mut().row_number_lookup.push(num_char);
                            let row_number = self.state.borrow().row_number_lookup.clone();
                            if row_number.chars().count() > 4 {
                                self.state.borrow_mut().row_number_lookup = String::new();
                            }
                            was_row_number = true;
                        }
                        KeyCode::Enter => {
                            let row_number = self.state.borrow().row_number_lookup.clone();
                            if row_number.chars().count() > 0 {
                                let num = u16::from_str_radix(&row_number, 16).unwrap();
                                if num < row_count as u16 {
                                    self.state.borrow_mut().row_index = num as usize;
                                }
                            }
                        }
                        _ => (),
                    }
                    if !was_row_number && self.state.borrow().row_number_lookup.chars().count() > 0
                    {
                        self.state.borrow_mut().row_number_lookup = String::new();
                    }
                }
                let area = f.area();

                let layout = AppLayout::RightBar.build(area.clone());
                f.render_widget(header, layout[0]);
                f.render_widget(
                    match self.main_view {
                        MainView::Timeline => timeline,
                    },
                    layout[1],
                );
                f.render_widget(sidebar, layout[2]);
                f.render_widget(
                    Line::raw(self.state.borrow().row_index.to_string()),
                    layout[2],
                );
            })
            .unwrap();
        let elapsed = start.elapsed();
        if elapsed < frame_time {
            thread::sleep(frame_time - elapsed);
        }
        return render_next;
    }
}
