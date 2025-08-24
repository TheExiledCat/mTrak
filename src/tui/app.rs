use std::{
    cell::RefCell,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

use ratatui::{
    DefaultTerminal,
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout, Rect},
};

use crate::data::{pattern::Pattern, project::Project};

use super::{
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
}
impl AppState {
    pub fn new(project: Project) -> Self {
        return Self {
            project,
            selected_pattern_index: 0,
        };
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

impl App {
    pub fn new(terminal: DefaultTerminal, fps: u16, project: Project) -> Self {
        return Self {
            terminal,
            input_handler: InputHandler::new(),
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
        self.terminal
            .draw(|f| {
                if let Some(event) = self.input_handler.read_event() {
                    if let KeyCode::Char('q') = event.code {
                        render_next = false;
                        return;
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
            })
            .unwrap();
        let elapsed = start.elapsed();
        if elapsed < frame_time {
            thread::sleep(frame_time - elapsed);
        }
        return render_next;
    }
}
