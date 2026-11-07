use std::{
    cmp, fmt,
    io::{Result, Stdout, Write, stdout},
    process,
    time::Duration,
};

use crossterm::{
    QueueableCommand, cursor,
    event::{self, Event::Key, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, Colored::ForegroundColor, SetBackgroundColor, SetColors, SetForegroundColor},
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};

struct Point {
    x: u16,
    y: u16,
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
    rows: u16,
    cols: u16,
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
            rows: rows,
            cols: cols,
            dirty: false,
            status: format!("{} - {} - {} lines", fname, Point { x: 0, y: 0 }, llen),
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
            let line = &mut self.lines[self.cursor.y as usize];
            line.remove(self.cursor.x as usize - 1);

            self.cursor.x -= 1;
            self.dirty = true;
        } else if self.cursor.y + 1 < self.lines.len() as u16 {
            let old = self.lines.remove(self.cursor.y as usize);

            self.cursor_up();
            self.cursor.x = self.lines[self.cursor.y as usize].len() as u16;

            let mut combined = std::mem::take(&mut self.lines[self.cursor.y as usize]);
            combined.push_str(&old);
            self.lines[self.cursor.y as usize] = combined;

            self.dirty = true;
        }
    }

    fn putc(&mut self, c: char) {
        let line = &mut self.lines[self.cursor.y as usize];
        line.insert(self.cursor.x as usize, c);
        self.cursor.x += 1;
        self.dirty = true;
    }

    fn cursor_up(&mut self) {
        if self.cursor.y > 0 {
            self.cursor.y -= 1;
            self.cursor.x = self
                .cursor
                .x
                .min(self.lines[self.cursor.y as usize].len() as u16);
        }
    }

    fn cursor_down(&mut self) {
        if self.cursor.y == cmp::max(self.rows - 1, 2) {
            return;
        }
        if self.lines.len() <= (self.cursor.y + 1) as usize {
            self.lines.push(String::new());
        }

        self.cursor.y += 1;
        self.cursor.x = self
            .cursor
            .x
            .min(self.lines[self.cursor.y as usize].len() as u16);
    }

    fn cursor_right(&mut self) {
        if self.cursor.x < self.lines[self.cursor.y as usize].len() as u16 {
            self.cursor.x += 1;
        } else if self.cursor.y + 1 < self.lines.len() as u16 {
            self.cursor.y += 1;
            self.cursor.x = 0;
        }
    }

    fn cursor_left(&mut self) {
        if self.cursor.x == 0 {
            self.cursor_up();
            self.cursor.x = self.lines[self.cursor.y as usize].len() as u16;
            return;
        }
        self.cursor.x -= 1;
        if self.cursor.x >= self.lines[self.cursor.y as usize].len() as u16 {
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
        for i in 0..self.lines.len() {
            if i >= (cmp::max(self.rows - 1, 2)) as usize {
                break;
            }

            let line = self.lines[i].clone();
            if line.is_empty() {
                stdout.queue(SetForegroundColor(Color::Grey))?;
                stdout.write(b"~")?;
                stdout.queue(SetForegroundColor(Color::Reset))?;
            }

            stdout.write(line.as_bytes())?;
            stdout.write(b"\r\n")?;
        }
        Ok(())
    }
    
    fn update_status()

    fn draw_status(&self, stdout: &mut Stdout) -> Result<()> {
        stdout.queue(cursor::SavePosition)?;

        stdout.queue(cursor::MoveTo(0, self.rows - 1))?;

        stdout.queue(SetBackgroundColor(Color::White))?;
        stdout.queue(SetForegroundColor(Color::Black))?;

        let pad = " ".repeat(self.cols as usize);
        stdout.write(pad.as_bytes())?;

        stdout.queue(cursor::MoveTo(0, self.rows - 1))?;

        stdout.write(self.status.as_bytes())?;

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
        stdout.queue(cursor::MoveTo(self.cursor.x, self.cursor.y))?;
        stdout.flush()?;
        Ok(())
    }

    fn main(&mut self, stdout: &mut Stdout) -> Result<()> {
        loop {
            if !self.poll_events()? {
                break;
            }
            self.draw(stdout)?;
        }
        Ok(())
    }

    fn deinit(&mut self, stdout: &mut Stdout) -> Result<()> {
        execute!(stdout, LeaveAlternateScreen)?;
        disable_raw_mode()?;
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        let mut stdout = stdout();
        self.init(&mut stdout)?;
        self.main(&mut stdout)?;
        self.deinit(&mut stdout)?;
        Ok(())
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
