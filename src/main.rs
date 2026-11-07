use std::{
    cmp::{self},
    fmt,
    io::{Result, Stdout, Write, stdout},
    process,
    time::Duration,
};

use crossterm::{
    QueueableCommand, cursor,
    event::{self, Event::Key, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, SetBackgroundColor, SetForegroundColor},
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};

struct Point {
    x: usize,
    y: usize,
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}:{}", self.x, self.y))?;
        Ok(())
    }
}

struct Editor {
    filename: String,
    lines: Vec<String>,
    cursor: Point,
    offset_row: usize,
    offset_col: usize,
    rows: usize,
    cols: usize,
    dirty: bool,
    status: String,
}

impl Editor {
    fn new(filename: String) -> Self {
        let mut lines = vec![];
        if std::path::Path::new(&filename).exists() {
            if let Ok(contents) = std::fs::read_to_string(&filename) {
                for line in contents.lines() {
                    lines.push(line.to_string());
                }
            }
        }
        if lines.is_empty() {
            lines.push(String::new());
        }

        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));

        let fname = filename.clone();
        let llen = lines.len();
        Self {
            filename,
            lines,
            cursor: Point { x: 0, y: 0 },
            rows: rows as usize,
            cols: cols as usize,
            offset_row: 0,
            offset_col: 0,
            dirty: false,
            status: format!("{} : {} - {} lines", fname, Point { x: 0, y: 0 }, llen),
        }
    }

    fn init(&mut self, stdout: &mut Stdout) -> Result<()> {
        execute!(stdout, EnterAlternateScreen)?;
        enable_raw_mode()?;
        Ok(())
    }

    fn backspace(&mut self) {
        if self.cursor.x == 0 && self.cursor.y == 0 {
            return;
        }

        if self.cursor.x > 0 {
            let line = &mut self.lines[self.cursor.y];
            line.remove(self.cursor.x - 1);

            self.cursor.x -= 1;
            self.dirty = true;
        } else if self.cursor.y + 1 < self.lines.len() {
            let old = self.lines.remove(self.cursor.y);

            self.cursor_up();
            self.cursor.x = self.lines[self.cursor.y].len();

            let mut combined = std::mem::take(&mut self.lines[self.cursor.y]);
            combined.push_str(&old);
            self.lines[self.cursor.y] = combined;

            self.dirty = true;
        }
    }

    fn putc(&mut self, c: char) {
        let line = &mut self.lines[self.cursor.y];
        line.insert(self.cursor.x, c);
        self.cursor.x += 1;
        self.dirty = true;
    }

    fn cursor_up(&mut self) {
        if self.cursor.y > 0 {
            self.cursor.y -= 1;
            self.cursor.x = self.cursor.x.min(self.lines[self.cursor.y].len());
        }
    }

    fn cursor_down(&mut self) {
        if self.lines.len() <= (self.cursor.y + 1) as usize {
            self.lines.push(String::new());
        }

        self.cursor.y += 1;
        self.cursor.x = self.cursor.x.min(self.lines[self.cursor.y].len());
    }

    fn cursor_right(&mut self) {
        if self.cursor.x < self.lines[self.cursor.y].len() {
            self.cursor.x += 1;
        } else if self.cursor.y + 1 < self.lines.len() {
            self.cursor.y += 1;
            self.cursor.x = 0;
        }
    }

    fn cursor_left(&mut self) {
        if self.cursor.x == 0 {
            self.cursor_up();
            self.cursor.x = self.lines[self.cursor.y].len();
            return;
        }
        self.cursor.x -= 1;
        if self.cursor.x >= self.lines[self.cursor.y].len() {
            self.cursor_down();
            self.cursor.x = 0;
        }
    }

    fn poll_events(&mut self) -> Result<bool> {
        if !event::poll(Duration::from_millis(500))? {
            return Ok(true);
        }

        if let Key(KeyEvent {
            code, modifiers, ..
        }) = event::read()?
        {
            match (modifiers, code) {
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    return Ok(false);
                }
                (_, KeyCode::Char(c)) => {
                    self.putc(c);
                }
                (_, KeyCode::Backspace) => {
                    self.backspace();
                }
                (_, KeyCode::Up) => {
                    self.cursor_up();
                }
                (_, KeyCode::Down | KeyCode::Enter) => {
                    self.cursor_down();
                }
                (_, KeyCode::Right) => {
                    self.cursor_right();
                }
                (_, KeyCode::Left) => {
                    self.cursor_left();
                }
                _ => {}
            }
        }
        Ok(true)
    }

    fn draw_lines(&self, stdout: &mut Stdout) -> Result<()> {
        let rows = self.rows.saturating_sub(1) as usize;
        for i in 0..rows {
            let line_i = self.offset_row.min(self.lines.len().saturating_sub(1)) + i;

            if line_i >= self.lines.len() {
                stdout.queue(SetForegroundColor(Color::DarkGrey))?;
                stdout.write(b"~")?;
                stdout.queue(SetForegroundColor(Color::Reset))?;
            } else {
                let line = &self.lines[line_i];
                if line.is_empty() && self.offset_col == 0 {
                    stdout.queue(SetForegroundColor(Color::Grey))?;
                    stdout.write(b"~")?;
                    stdout.queue(SetForegroundColor(Color::Reset))?;
                } else if line.len() > self.offset_col {
                    let scrolled = &line[self.offset_col..];
                    let max_width = cmp::min(scrolled.len(), self.cols as usize);
                    stdout.write(scrolled[..max_width].as_bytes())?;
                }
            }
            stdout.write(b"\r\n")?;
        }
        Ok(())
    }

    fn scroll(&mut self) {
        if self.cursor.y < self.offset_row {
            self.offset_row = self.cursor.y;
        } else if self.cursor.y >= self.offset_row + self.rows {
            self.offset_row = self.cursor.y - self.rows + 1;
        }
        if self.offset_row > self.lines.len().saturating_sub(1) {
            self.offset_row = self.lines.len().saturating_sub(1);
        }
        if self.cursor.x < self.offset_col {
            self.offset_col = self.cursor.x;
        } else if self.cursor.x >= (self.offset_col + self.cols as usize - 1) {
            self.offset_col = self.cols - self.cols - 1;
        }
    }

    fn update_status(&mut self) {
        self.status = format!(
            "{} : {} - {} lines",
            self.filename,
            self.cursor,
            self.lines.len(),
        );
    }

    fn draw_status(&self, stdout: &mut Stdout) -> Result<()> {
        stdout.queue(cursor::SavePosition)?;

        stdout.queue(cursor::MoveTo(0, self.rows as u16 - 1))?;

        stdout.queue(SetBackgroundColor(Color::White))?;
        stdout.queue(SetForegroundColor(Color::Black))?;

        let pad = " ".repeat(self.cols as usize);
        stdout.write(pad.as_bytes())?;

        stdout.queue(cursor::MoveTo(0, self.rows as u16 - 1))?;

        stdout.write(self.status.as_bytes())?;
        if self.dirty {
            stdout.queue(SetForegroundColor(Color::DarkRed))?;
            stdout.write(b" (dirty)")?;
            stdout.queue(SetForegroundColor(Color::Reset))?;
        }

        stdout.queue(SetBackgroundColor(Color::Reset))?;
        stdout.queue(SetForegroundColor(Color::Reset))?;

        stdout.queue(cursor::RestorePosition)?;
        Ok(())
    }

    fn draw(&self, stdout: &mut Stdout) -> Result<()> {
        stdout.queue(Clear(ClearType::All))?;
        stdout.queue(cursor::Hide)?;
        stdout.queue(cursor::MoveTo(0, 0))?;

        self.draw_lines(stdout)?;
        self.draw_status(stdout)?;

        stdout.queue(cursor::Show)?;
        stdout.queue(cursor::MoveTo(
            self.cursor.x.saturating_sub(self.offset_col) as u16,
            self.cursor.y.saturating_sub(self.offset_row) as u16,
        ))?;
        stdout.flush()?;
        Ok(())
    }

    fn main(&mut self, stdout: &mut Stdout) -> Result<()> {
        loop {
            if !self.poll_events()? {
                break;
            }
            self.update_status();
            self.scroll();
            self.draw(stdout)?;
        }
        Ok(())
    }

    fn deinit(&mut self, stdout: &mut Stdout) -> Result<()> {
        disable_raw_mode()?;
        execute!(stdout, LeaveAlternateScreen)?;
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        let mut stdout = stdout();
        self.init(&mut stdout)?;
        let res = self.main(&mut stdout);
        self.deinit(&mut stdout)?;
        res
    }
}

pub fn main() {
    let args: Vec<String> = std::env::args().collect();
    let filename = if args.len() > 1 {
        args[1].clone()
    } else {
        "untitled".into()
    };

    let mut editor = Editor::new(filename);
    if let Err(e) = editor.run() {
        eprintln!("error: {}", e);
        process::exit(1);
    }
}
