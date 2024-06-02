mod errors;
mod tui;

use color_eyre::{
    eyre::{bail, WrapErr},
    Result,
};
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::Alignment,
    prelude::*,
    widgets::{block::*, *},
};
use tui_textarea::{CursorMove, TextArea};

fn main() -> Result<()> {
    errors::install_hooks()?;

    let mut terminal = tui::init()?;
    App::default().run(&mut terminal)?;
    tui::restore()?;

    Ok(())
}

const MAX_HISTORIES: usize = usize::MAX;

#[derive(Default, PartialEq)]
enum Mode {
    #[default]
    Insert,
    Normal,
    Visual(VisualType),
    Command,
}

#[derive(Default, PartialEq)]
enum VisualType {
    #[default]
    Character,
    Line,
    Block,
}

struct App {
    should_exit: bool,
    editor: TextArea<'static>,
    commandline: TextArea<'static>,
    mode: Mode,
}

impl Default for App {
    fn default() -> Self {
        let mut editor = TextArea::default();
        editor.set_max_histories(MAX_HISTORIES);

        let mut commandline = TextArea::default();
        commandline.set_max_histories(MAX_HISTORIES);

        Self {
            editor,
            commandline,
            should_exit: false,
            mode: Mode::default(),
        }
    }
}

impl App {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        while !self.should_exit {
            self.update();
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events().wrap_err("handle events failed")?;
        }
        Ok(())
    }

    fn render_frame(&mut self, frame: &mut Frame) {
        let constraints = if self.mode == Mode::Command {
            [Constraint::Percentage(100), Constraint::Min(3)]
        } else {
            [Constraint::Percentage(100), Constraint::Max(0)]
        };

        let app_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(frame.size());

        let buffer_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(1),
                Constraint::Fill(3),
                Constraint::Fill(1),
            ])
            // .horizontal_margin(1)
            .split(app_layout[1]);

        let widget = self.editor.widget();
        frame.render_widget(widget, app_layout[0]);

        let widget = self.commandline.widget();
        frame.render_widget(widget, buffer_layout[1]);
    }

    fn handle_events(&mut self) -> Result<()> {
        match crossterm::event::read()? {
            crossterm::event::Event::Key(key) => self.handle_keyevent(key),
            _ => Ok(()),
        }
    }

    fn handle_keyevent(&mut self, event: crossterm::event::KeyEvent) -> Result<()> {
        match &self.mode {
            Mode::Insert => match event {
                crossterm::event::KeyEvent {
                    code: KeyCode::Esc,
                    kind: KeyEventKind::Press,
                    ..
                } => self.mode = Mode::Normal,

                input => _ = self.editor.input(input),
            },

            Mode::Normal => match event {
                crossterm::event::KeyEvent {
                    code: KeyCode::Char('i'),
                    kind: KeyEventKind::Press,
                    ..
                } => self.mode = Mode::Insert,

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('v'),
                    kind: KeyEventKind::Press,
                    modifiers: _modifiers,
                    ..
                } => {
                    self.editor.start_selection();
                    self.mode = Mode::Visual(VisualType::default());
                }

                crossterm::event::KeyEvent {
                    code: KeyCode::Char(';'),
                    kind: KeyEventKind::Press,
                    ..
                } => self.mode = Mode::Command,

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('p'),
                    kind: KeyEventKind::Press,
                    ..
                } => _ = self.editor.paste(),

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('h'),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                } => self.editor.move_cursor(CursorMove::Back),

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('j'),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                } => self.editor.move_cursor(CursorMove::Down),

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('k'),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                } => self.editor.move_cursor(CursorMove::Up),

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('l'),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                } => self.editor.move_cursor(CursorMove::Forward),

                _ => {}
            },

            Mode::Command => match event {
                crossterm::event::KeyEvent {
                    code: KeyCode::Esc,
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    // Clear the buffer. (Doing it this way preserves the history.)
                    self.commandline.move_cursor(CursorMove::End);
                    self.commandline.delete_line_by_head();

                    self.mode = Mode::Normal;
                }

                crossterm::event::KeyEvent {
                    code: KeyCode::Enter,
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.parse_command()?;

                    // Clear the buffer. (Doing it this way preserves the history.)
                    self.commandline.move_cursor(CursorMove::End);
                    self.commandline.delete_line_by_head();

                    self.mode = Mode::Normal;
                }

                input => _ = self.commandline.input(input),
            },

            Mode::Visual(visual_type) => match event {
                crossterm::event::KeyEvent {
                    code: KeyCode::Esc,
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.editor.cancel_selection();
                    self.mode = Mode::Normal;
                }

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('y'),
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.editor.copy();
                    self.mode = Mode::Normal;
                }

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('d'),
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.editor.cut();
                    self.mode = Mode::Normal;
                }

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('h'),
                    kind: KeyEventKind::Press,
                    ..
                } => match visual_type {
                    VisualType::Character => self.editor.move_cursor(CursorMove::Back),
                    VisualType::Line => {}
                    VisualType::Block => {}
                },

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('j'),
                    kind: KeyEventKind::Press,
                    ..
                } => match visual_type {
                    VisualType::Character => self.editor.move_cursor(CursorMove::Down),
                    VisualType::Line => {}
                    VisualType::Block => {}
                },

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('k'),
                    kind: KeyEventKind::Press,
                    ..
                } => match visual_type {
                    VisualType::Character => self.editor.move_cursor(CursorMove::Up),
                    VisualType::Line => {}
                    VisualType::Block => {}
                },

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('l'),
                    kind: KeyEventKind::Press,
                    ..
                } => match visual_type {
                    VisualType::Character => self.editor.move_cursor(CursorMove::Forward),
                    VisualType::Line => {}
                    VisualType::Block => {}
                },

                _ => {}
            },
        }

        Ok(())
    }

    fn update(&mut self) {
        self.update_editor();
        self.update_commandline();
    }

    fn update_editor(&mut self) {
        self.editor
            .set_line_number_style(Style::default().fg(Color::DarkGray));

        let block = Block::default()
            .borders(Borders::TOP)
            .border_type(BorderType::Double)
            .padding(Padding::uniform(1))
            .title(" Gloop ")
            .title_alignment(Alignment::Center);

        self.editor.set_block(block);
    }

    fn update_commandline(&mut self) {
        self.commandline.set_cursor_line_style(Style::default());

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::horizontal(1))
            .title(" Command ")
            .title_alignment(Alignment::Right);

        self.commandline.set_block(block);
    }

    fn parse_command(&mut self) -> Result<()> {
        let lines = self.commandline.lines();

        if lines.len() > 1 {
            bail!("more lines than expected in commandline");
        }

        if let Some(line) = lines.first() {
            match line.as_str() {
                "q" => self.should_exit = true,
                _ => bail!("unknown command: {}", line),
            }
        }

        Ok(())
    }
}
