mod errors;
mod tui;

use base64::prelude::*;
use color_eyre::{
    eyre::{bail, WrapErr},
    owo_colors::OwoColorize,
    Result,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::Alignment,
    prelude::*,
    style::Stylize,
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
                    code: KeyCode::Char('u'),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                } => _ = self.editor.undo(),

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('U'),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                } => _ = self.editor.redo(),

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('i'),
                    kind: KeyEventKind::Press,
                    ..
                } => self.mode = Mode::Insert,

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('a'),
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.editor.move_cursor(CursorMove::Forward);
                    self.mode = Mode::Insert;
                }

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
                    code: KeyCode::Char(';') | KeyCode::Char(' '),
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

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('b'),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                } => self.editor.move_cursor(CursorMove::WordBack),

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('w'),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                } => self.editor.move_cursor(CursorMove::WordForward),

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('E'),
                    kind: KeyEventKind::Press,
                    ..
                } => self.editor.move_cursor(CursorMove::End),

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('0'),
                    kind: KeyEventKind::Press,
                    ..
                } => self.editor.move_cursor(CursorMove::Head),

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('I'),
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.editor.move_cursor(CursorMove::Head);
                    self.mode = Mode::Insert;
                }

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('A'),
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.editor.move_cursor(CursorMove::End);
                    self.mode = Mode::Insert;
                }

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('o'),
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.editor.move_cursor(CursorMove::End);
                    self.editor.insert_newline();
                    self.mode = Mode::Insert;
                }

                crossterm::event::KeyEvent {
                    code: KeyCode::Char('O'),
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.editor.move_cursor(CursorMove::Up);
                    self.editor.move_cursor(CursorMove::End);
                    self.editor.insert_newline();
                    self.mode = Mode::Insert;
                }

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
                KeyEvent {
                    code: KeyCode::Esc,
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.editor.cancel_selection();
                    self.mode = Mode::Normal;
                }

                KeyEvent {
                    code: KeyCode::Char('y'),
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.editor.copy();
                    self.mode = Mode::Normal;
                }

                KeyEvent {
                    code: KeyCode::Char('d'),
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.editor.cut();
                    self.mode = Mode::Normal;
                }

                KeyEvent {
                    code: KeyCode::Char('c'),
                    kind: KeyEventKind::Press,
                    ..
                } => {
                    self.editor.cut();
                    self.mode = Mode::Insert;
                }

                KeyEvent {
                    code: KeyCode::Char('h'),
                    kind: KeyEventKind::Press,
                    ..
                } => match visual_type {
                    VisualType::Character => self.editor.move_cursor(CursorMove::Back),
                    VisualType::Line => {}
                    VisualType::Block => {}
                },

                KeyEvent {
                    code: KeyCode::Char('j'),
                    kind: KeyEventKind::Press,
                    ..
                } => match visual_type {
                    VisualType::Character => self.editor.move_cursor(CursorMove::Down),
                    VisualType::Line => {}
                    VisualType::Block => {}
                },

                KeyEvent {
                    code: KeyCode::Char('k'),
                    kind: KeyEventKind::Press,
                    ..
                } => match visual_type {
                    VisualType::Character => self.editor.move_cursor(CursorMove::Up),
                    VisualType::Line => {}
                    VisualType::Block => {}
                },

                KeyEvent {
                    code: KeyCode::Char('l'),
                    kind: KeyEventKind::Press,
                    ..
                } => match visual_type {
                    VisualType::Character => self.editor.move_cursor(CursorMove::Forward),
                    VisualType::Line => {}
                    VisualType::Block => {}
                },

                KeyEvent {
                    code: KeyCode::Char('E'),
                    kind: KeyEventKind::Press,
                    ..
                } => self.editor.move_cursor(CursorMove::End),

                KeyEvent {
                    code: KeyCode::Char('0'),
                    kind: KeyEventKind::Press,
                    ..
                } => self.editor.move_cursor(CursorMove::Head),

                KeyEvent {
                    code: KeyCode::Char('b'),
                    kind: KeyEventKind::Press,
                    ..
                } => match visual_type {
                    VisualType::Character => self.editor.move_cursor(CursorMove::WordBack),
                    VisualType::Line => {}
                    VisualType::Block => {}
                },

                KeyEvent {
                    code: KeyCode::Char('w'),
                    kind: KeyEventKind::Press,
                    ..
                } => match visual_type {
                    VisualType::Character => self.editor.move_cursor(CursorMove::WordForward),
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
        if self.mode == Mode::Command {
            self.editor
                .set_cursor_style(Style::default().bg(Color::DarkGray));
        } else {
            self.editor.set_cursor_style(Style::default().reversed());
        }

        self.editor
            .set_line_number_style(Style::default().fg(Color::Yellow));

        let block = Block::default()
            .borders(Borders::TOP)
            .border_type(BorderType::Double)
            .padding(Padding::uniform(1))
            .title(Span::from(" Gloop ").yellow())
            .title_alignment(Alignment::Center);

        self.editor.set_block(block);
    }

    fn update_commandline(&mut self) {
        self.commandline.set_cursor_line_style(Style::default());

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::horizontal(1))
            .title(Span::from(" Command ").yellow())
            .title_alignment(Alignment::Right);

        self.commandline.set_block(block);
    }

    fn parse_command(&mut self) -> Result<()> {
        let lines = self.commandline.lines();

        if lines.len() > 1 {
            bail!("more lines than expected in commandline");
        }

        if let Some(line) = lines.first() {
            match line.as_str().split_whitespace().next().unwrap_or_default() {
                "q" => self.should_exit = true,

                "json" => match line.as_str().split_whitespace().nth(1).unwrap_or_default() {
                    "format" => {
                        let ugly_json = self.editor.lines().join("\n");

                        self.editor.select_all();
                        self.editor.cut();

                        let pretty_json = jsonxf::pretty_print(&ugly_json).unwrap();

                        self.editor.insert_str(pretty_json);
                    }

                    _ => bail!("unknown command: {}", line),
                },

                "base64" => match line.as_str().split_whitespace().nth(1).unwrap_or_default() {
                    "encode" => {
                        let text = self.editor.lines().join("\n");

                        self.editor.select_all();
                        self.editor.cut();

                        let encoded = BASE64_STANDARD.encode(text);
                        self.editor.insert_str(encoded);
                    }

                    "decode" => {
                        let text = self.editor.lines().join("\n");

                        self.editor.select_all();
                        self.editor.cut();

                        let decoded = BASE64_STANDARD.decode(text)?;
                        let decoded = String::from_utf8(decoded)?;
                        self.editor.insert_str(decoded);
                    }

                    _ => bail!("unknown command: {}", line),
                },

                _ => bail!("unknown command: {}", line),
            }
        }

        Ok(())
    }
}
