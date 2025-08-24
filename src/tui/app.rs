use std::{
    thread,
    time::{Duration, Instant},
};

use ratatui::{
    DefaultTerminal,
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout, Rect},
};

use crate::data::project::Project;

use super::{
    keymap::InputHandler,
    views::{header::Header, sidebar::SideBar, timeline::TimeLineView},
};

pub struct App {
    pub state: AppState,
    pub terminal: DefaultTerminal,
    pub input_handler: InputHandler,
    pub fps: u16,
    pub main_view: MainView,
}
pub struct AppState {
    pub project: Project,
}
impl AppState {
    pub fn new(project: Project) -> Self {
        return Self { project };
    }
}
enum MainView {
    Timeline,
}
pub enum AppLayout {
    RIGHT_BAR,
    LEFT_BAR,
}
impl AppLayout {
    pub fn build(&self, area: Rect) -> Vec<Rect> {
        match self {
            AppLayout::RIGHT_BAR => {
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
            AppLayout::LEFT_BAR => todo!(),
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
            state: AppState::new(project),
        };
    }
    pub fn draw(&mut self) -> bool {
        let mut render_next = true;
        let frame_time = Duration::from_secs_f64(1.0 / self.fps as f64);
        let start = Instant::now();
        self.terminal
            .draw(|f| {
                if let Some(event) = self.input_handler.read_event() {
                    if let KeyCode::Char('q') = event.code {
                        render_next = false;
                        return;
                    }
                }
                let area = f.area();
                let layout = AppLayout::RIGHT_BAR.build(area.clone());
                let header = Header::new(&self.state);
                let sidebar = SideBar::new();
                f.render_widget(header, layout[0]);
                f.render_widget(
                    match self.main_view {
                        MainView::Timeline => TimeLineView::new(&self.state),
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
