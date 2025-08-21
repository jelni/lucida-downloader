use std::sync::mpsc::{self, Receiver};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::prelude::Backend;
use ratatui::style::Stylize;
use ratatui::symbols::border;
use ratatui::text::{Line, Text};
use ratatui::widgets::{
    self, Block, BorderType, Borders, List, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Widget,
};
use ratatui::{Frame, Terminal};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use crate::models::AlbumInfo;

pub struct App {
    albums: Vec<AlbumDownload>,
    rx: Receiver<DownloadMessage>,
    scrollbar: ScrollbarState,
    list: ListState,
    command_input: Input,
    exit: bool,
}

struct AlbumDownload {
    id: u32,
    url: String,
    state: DownloadState,
}

enum DownloadState {
    Resolving,
    Downloading {
        title: String,
        tracks: Vec<TrackDownload>,
    },
    Completed {
        title: String,
    },
    Failed {
        title: Option<String>,
        error: String,
    },
}

enum TrackDownload {
    Status(String),
    Progress(f32),
}

enum DownloadMessage {
    AlbumResolved(AlbumInfo),
    TrackStatus { index: usize, status: TrackDownload },
    AlbumCompleted,
    Error { error: String },
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        Self {
            albums: Vec::new(),
            rx,
            scrollbar: ScrollbarState::default(),
            list: ListState::default(),
            command_input: Input::default(),
            exit: false,
        }
    }

    pub fn run(&mut self, terminal: &mut Terminal<impl Backend>) {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame)).unwrap();
            self.handle_events();
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        // frame.render_widget(self, area);
        ui(self, frame);
    }

    fn handle_events(&mut self) {
        match event::read().unwrap() {
            Event::Key(event) if event.kind == KeyEventKind::Press => self.handle_key_event(event),
            _ => {}
        };
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Up => {
                self.scrollbar.prev();
                self.list.scroll_up_by(1);
            }
            KeyCode::Down => {
                self.scrollbar.next();
                self.list.scroll_down_by(1);
            }
            KeyCode::Enter => self.handle_command(),
            _ => {
                self.command_input.handle_event(&Event::Key(event));
            }
        }
    }

    fn handle_command(&mut self) {
        match self.command_input.value_and_reset().as_str() {
            "q" => self.exit(),
            "test" => {
                self.add_album_to_queue("foo".into());
                self.add_album_to_queue("bar".into());
                self.add_album_to_queue("baz".into());
            }
            _ => (),
        }
    }

    fn add_album_to_queue(&mut self, url: String) {
        let available_id = (0..)
            .find(|&n| self.albums.iter().all(|album| album.id != n))
            .unwrap();

        self.albums.push(AlbumDownload {
            id: available_id,
            url,
            state: DownloadState::Resolving,
        });

        self.scrollbar = self.scrollbar.content_length(self.albums.len());
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

fn ui(app: &mut App, frame: &mut Frame) {
    let area = frame.area();
    let buf = frame.buffer_mut();

    let title = concat!(" ", env!("CARGO_PKG_NAME"), " ").bold();

    Text::from(title).render(area, buf);

    let [_, album_list_area, command_input_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(3),
    ])
    .areas(area);

    widgets::StatefulWidget::render(
        List::new(app.albums.iter().map(|album| match &album.state {
            DownloadState::Resolving => format!("{} - Resolving {}", album.id, album.url),
            DownloadState::Downloading { progress } => {
                format!("{} - Downloading ({:.0})", album.id, progress * 100.)
            }
            DownloadState::Completed => format!("{} - Done", album.id),
            DownloadState::Failed { error } => format!("{} - Error: {error}", album.id),
        }))
        .block(Block::bordered().border_type(BorderType::Rounded)),
        album_list_area,
        buf,
        &mut app.list,
    );

    let [album_list_area_scroll_area] = Layout::vertical([Constraint::Fill(1)])
        .margin(1)
        .areas(album_list_area);

    widgets::StatefulWidget::render(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        album_list_area_scroll_area,
        buf,
        &mut app.scrollbar,
    );

    Paragraph::new(app.command_input.value())
        .block(Block::bordered().border_type(BorderType::Rounded))
        .render(command_input_area, buf);
}
