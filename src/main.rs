//! ime — Intuitive macOS editor.

mod editor;
mod highlight;

use crossterm::{
    cursor, event,
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event, KeyCode,
        KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, MouseButton, MouseEvent, MouseEventKind,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute, queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{self, BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen},
};
use editor::{Editor, Motion, Pos, INDENT};
use highlight::Syntax;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const TAB: usize = 4;

struct App {
    ed: Editor,
    syn: Syntax,
    path: PathBuf,
    top: usize,
    left: usize,
    height: usize,
    gutter: usize,
    numbers: bool,
    preview: bool,
    ptop: usize, // scroll dell'anteprima markdown
    follow: bool, // false dopo uno scroll col trackpad: la vista non torna sul cursore
    msg: String,
    quit_armed: bool,
    last_click: Option<(Instant, Pos, u8)>,
    debug: bool,
    kitty: bool, // il terminale manda ⌘ (Ghostty, kitty, WezTerm, iTerm2); Terminal.app no
}

fn main() -> io::Result<()> {
    let Some(arg) = std::env::args_os().nth(1) else {
        eprintln!("usage: ime <file>");
        std::process::exit(2);
    };
    let path = PathBuf::from(arg);
    let path = fs::canonicalize(&path).unwrap_or(path); // salvando su un symlink scriviamo il file vero
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            eprintln!("ime: {}: {e}", path.display());
            std::process::exit(1);
        }
    };

    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    let kitty = terminal::supports_keyboard_enhancement().unwrap_or(false);
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore(kitty);
        hook(info);
    }));
    execute!(out, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste, cursor::SetCursorStyle::SteadyBar)?;
    if kitty {
        // solo "disambiguate": i tasti che producono testo (⌥ò = @) arrivano come testo, ⌘ arriva come SUPER
        execute!(out, PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES))?;
    }

    let mut app = App {
        syn: Syntax::for_file(&path, text.lines().next().unwrap_or("")),
        ed: Editor::from_text(&text),
        path,
        top: 0,
        left: 0,
        height: 1,
        gutter: 0,
        numbers: false,
        preview: false,
        ptop: 0,
        follow: true,
        msg: String::new(),
        quit_armed: false,
        last_click: None,
        debug: std::env::var_os("IME_KEYS").is_some(),
        kitty,
    };
    let res = app.run(&mut out);
    restore(kitty);
    res
}

fn restore(kitty: bool) {
    let mut out = io::stdout();
    if kitty {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    let _ = execute!(
        out,
        DisableBracketedPaste,
        DisableMouseCapture,
        cursor::SetCursorStyle::DefaultUserShape,
        LeaveAlternateScreen,
        cursor::Show
    );
    let _ = terminal::disable_raw_mode();
}

impl App {
    fn run(&mut self, out: &mut impl Write) -> io::Result<()> {
        loop {
            self.render(out)?;
            // il trackpad manda raffiche di eventi: li gestiamo tutti e ridisegniamo una volta sola
            loop {
                if self.handle(event::read()?) {
                    return Ok(());
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
    }

    /// Ritorna true per uscire.
    fn handle(&mut self, ev: Event) -> bool {
        if self.debug && !matches!(&ev, Event::Mouse(m) if m.kind == MouseEventKind::Moved) {
            self.msg = format!("{ev:?}");
        }
        match ev {
            Event::Key(k) if k.kind != KeyEventKind::Release => {
                self.follow = true;
                return self.key(k);
            }
            Event::Mouse(m) => self.mouse(m),
            Event::Paste(s) if !self.preview => {
                self.follow = true;
                self.ed.insert(&s);
            }
            _ => {}
        }
        false
    }

    fn key(&mut self, k: KeyEvent) -> bool {
        let armed = std::mem::take(&mut self.quit_armed);
        if !self.debug {
            self.msg.clear();
        }
        let m = k.modifiers;
        let shift = m.contains(KeyModifiers::SHIFT);
        let alt = m.contains(KeyModifiers::ALT);
        let ctrl = m.contains(KeyModifiers::CONTROL);
        let cmd = m.contains(KeyModifiers::SUPER);
        use KeyCode::*;
        use Motion as M;
        match k.code {
            // ⌥⌘ + tasto: comandi dell'editor
            Char(c) if cmd && alt => match c.to_ascii_lowercase() {
                'n' => self.numbers = !self.numbers,
                'm' => self.toggle_preview(),
                _ => {}
            },
            // stessi comandi con ⌃ per i terminali che non mandano ⌘ (Terminal.app); ⌃M non si può: è ↩
            Char('n') if ctrl && !cmd => self.numbers = !self.numbers,
            Char('p') if ctrl && !cmd => self.toggle_preview(),
            Char('q' | 'Q') if ctrl && !cmd => {
                if !self.ed.dirty || armed {
                    return true;
                }
                self.quit_armed = true;
                self.msg = "unsaved changes: ⌃Q again to quit anyway".into();
            }
            // in anteprima si scorre e basta, niente modifiche
            Up if self.preview => self.ptop = self.ptop.saturating_sub(1),
            Down if self.preview => self.ptop += 1,
            PageUp if self.preview => self.ptop = self.ptop.saturating_sub(self.height),
            PageDown if self.preview => self.ptop += self.height,
            _ if self.preview => {}
            // ⌘ = scorciatoie Mac; ⌃ = ripiego per terminali senza kitty protocol
            // (Ghostty manda ⌃A/⌃E per ⌘←/⌘→ e ⌃U per ⌘⌫)
            Char(c) if cmd || ctrl => match (c.to_ascii_lowercase(), cmd) {
                ('s', _) => self.save(),
                ('z', true) if shift => self.ed.redo(),
                ('z', _) => self.ed.undo(),
                ('y', false) => self.ed.redo(),
                ('a', true) => self.ed.select_all(),
                ('a', false) => self.ed.go(M::LineStart, shift),
                ('e', false) => self.ed.go(M::LineEnd, shift),
                ('k', false) => self.ed.delete(M::LineEnd),
                ('u', false) => self.ed.delete(M::LineStart),
                ('c', true) => self.copy(false),
                ('x', true) => self.copy(true),
                ('v', true) => {
                    if let Some(s) = pbpaste() {
                        self.ed.insert(&s)
                    }
                }
                _ => {}
            },
            // terminali legacy mandano ESC b / ESC f per ⌥← / ⌥→
            Char('b') if alt => self.ed.go(M::WordLeft, shift),
            Char('f') if alt => self.ed.go(M::WordRight, shift),
            Char(c) => self.ed.type_char(c),
            Left => self.ed.go(if cmd { M::LineStart } else if alt { M::WordLeft } else { M::Left }, shift),
            Right => self.ed.go(if cmd { M::LineEnd } else if alt { M::WordRight } else { M::Right }, shift),
            Up => self.ed.go(if cmd { M::DocStart } else { M::Up(1) }, shift),
            Down => self.ed.go(if cmd { M::DocEnd } else { M::Down(1) }, shift),
            Home => self.ed.go(if cmd || ctrl { M::DocStart } else { M::LineStart }, shift),
            End => self.ed.go(if cmd || ctrl { M::DocEnd } else { M::LineEnd }, shift),
            PageUp => self.ed.go(M::Up(self.height), shift),
            PageDown => self.ed.go(M::Down(self.height), shift),
            Backspace if cmd || alt => self.ed.delete(if cmd { M::LineStart } else { M::WordLeft }),
            Backspace => self.ed.backspace(),
            Delete => self.ed.delete(if alt { M::WordRight } else { M::Right }),
            Enter => self.ed.newline(),
            Tab => self.ed.insert(INDENT),
            Esc => self.ed.anchor = None,
            _ => {}
        }
        false
    }

    fn toggle_preview(&mut self) {
        if self.syn.name() == "Markdown" {
            self.preview = !self.preview;
            self.ptop = self.top;
        } else {
            self.msg = "preview: markdown files only".into();
        }
    }

    fn mouse(&mut self, m: MouseEvent) {
        if self.preview {
            match m.kind {
                MouseEventKind::ScrollUp => self.ptop = self.ptop.saturating_sub(1),
                MouseEventKind::ScrollDown => self.ptop += 1,
                _ => {}
            }
            return;
        }
        let p = self.pos_at(m.column as usize, m.row as usize);
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.follow = true;
                let n = match self.last_click {
                    Some((t, q, n)) if q == p && t.elapsed() < Duration::from_millis(400) => n % 3 + 1,
                    _ => 1,
                };
                self.last_click = Some((Instant::now(), p, n));
                match n {
                    1 => self.ed.move_to(p, m.modifiers.contains(KeyModifiers::SHIFT)),
                    2 => self.ed.select_word_at(p),
                    _ => self.ed.select_line(p.row),
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.follow = true;
                self.ed.move_to(p, true);
            }
            // una riga per evento: è già il terminale a moltiplicare per velocità del gesto
            MouseEventKind::ScrollUp => {
                self.follow = false;
                self.top = self.top.saturating_sub(1);
            }
            MouseEventKind::ScrollDown => {
                self.follow = false;
                self.top = (self.top + 1).min(self.ed.lines.len().saturating_sub(self.height)).max(self.top);
            }
            MouseEventKind::ScrollLeft => {
                self.follow = false;
                self.left = self.left.saturating_sub(1);
            }
            MouseEventKind::ScrollRight => {
                self.follow = false;
                self.left += 1; // limitato in render alla riga visibile più lunga
            }
            _ => {}
        }
    }

    fn pos_at(&self, x: usize, y: usize) -> Pos {
        let row = (self.top + y).min(self.ed.lines.len() - 1);
        Pos { row, col: col_at(&self.ed.lines[row], x.saturating_sub(self.gutter) + self.left) }
    }

    fn save(&mut self) {
        match write_atomic(&self.path, &self.ed.to_text()) {
            Ok(()) => {
                self.ed.dirty = false;
                self.msg = "saved".into();
            }
            Err(e) => self.msg = format!("save failed: {e}"),
        }
    }

    fn copy(&mut self, cut: bool) {
        let Some(s) = self.ed.selected_text() else { return };
        if let Err(e) = pbcopy(&s) {
            self.msg = format!("copy failed: {e}");
        } else if cut {
            self.ed.delete(Motion::Left); // con una selezione attiva cancella solo quella
        }
    }

    fn render(&mut self, out: &mut impl Write) -> io::Result<()> {
        let (w, h) = terminal::size()?;
        let (w, h) = (w as usize, h as usize);
        self.height = h.saturating_sub(1).max(1);
        queue!(out, BeginSynchronizedUpdate, cursor::Hide)?;
        if self.preview {
            self.render_preview(out, w)?;
        } else {
            self.render_text(out, w)?;
        }
        queue!(out, EndSynchronizedUpdate)?;
        out.flush()
    }

    fn render_text(&mut self, out: &mut impl Write, w: usize) -> io::Result<()> {
        self.syn.invalidate(std::mem::replace(&mut self.ed.changed_from, usize::MAX));
        let ed = &self.ed;
        self.gutter = if self.numbers { ed.lines.len().to_string().len() + 1 } else { 0 };
        let text_w = w.saturating_sub(self.gutter).max(1);
        let cur = ed.cursor;
        let cvx = visual_col(&ed.lines[cur.row], cur.col);
        if self.follow {
            self.top = self.top.min(cur.row).max((cur.row + 1).saturating_sub(self.height));
            self.left = self.left.min(cvx).max((cvx + 1).saturating_sub(text_w));
        }
        let visible = &ed.lines[self.top.min(ed.lines.len())..(self.top + self.height).min(ed.lines.len())];
        let widest = visible.iter().map(|l| visual_col(l, usize::MAX)).max().unwrap_or(0);
        self.left = self.left.min((widest + 1).saturating_sub(text_w));
        let colors = self.syn.colors(&ed.lines, self.top, self.top + self.height);
        let sel = ed.selection();

        for y in 0..self.height {
            queue!(out, cursor::MoveTo(0, y as u16))?;
            let row = self.top + y;
            if let Some(line) = ed.lines.get(row) {
                if self.numbers {
                    let n = format!("{:>1$} ", row + 1, self.gutter - 1);
                    queue!(out, SetForegroundColor(Color::DarkGrey), Print(n), ResetColor)?;
                }
                let (mut vc, mut inv, mut fg) = (0, false, None);
                for (i, ch) in line.chars().enumerate() {
                    let next = advance(vc, ch);
                    if next > self.left {
                        let selected = sel.is_some_and(|(a, b)| (a..b).contains(&Pos { row, col: i }));
                        if selected != inv {
                            inv = selected;
                            queue!(out, SetAttribute(if inv { Attribute::Reverse } else { Attribute::NoReverse }))?;
                        }
                        let c = colors.get(y).and_then(|c| c.get(i)).copied();
                        if c != fg {
                            fg = c;
                            queue!(out, SetForegroundColor(c.unwrap_or(Color::Reset)))?;
                        }
                        // i caratteri di controllo non vanno mai stampati: sarebbero escape sequence verso il terminale
                        let shown = if ch == '\t' { ' ' } else if ch.is_control() { '�' } else { ch };
                        for _ in vc.max(self.left)..next.min(self.left + text_w) {
                            queue!(out, Print(shown))?;
                        }
                    }
                    vc = next;
                    if vc >= self.left + text_w {
                        break;
                    }
                }
                queue!(out, SetAttribute(Attribute::Reset))?;
            }
            queue!(out, Clear(ClearType::UntilNewLine))?;
        }

        let hint = match (self.kitty, self.syn.name() == "Markdown") {
            (true, true) => "⌘S save  ⌃Q quit  ⌥⌘n numbers  ⌥⌘m preview",
            (true, false) => "⌘S save  ⌃Q quit  ⌥⌘n numbers",
            (false, true) => "⌃S save  ⌃Q quit  ⌃N numbers  ⌃P preview",
            (false, false) => "⌃S save  ⌃Q quit  ⌃N numbers",
        };
        let info = format!("{}:{}  {}", cur.row + 1, cur.col + 1, self.syn.name());
        self.status(out, w, &info, hint)?;

        let y = cur.row.checked_sub(self.top).filter(|&y| y < self.height);
        let x = cvx.checked_sub(self.left).filter(|&x| x < text_w);
        if let (Some(x), Some(y)) = (x, y) {
            queue!(out, cursor::MoveTo((self.gutter + x) as u16, y as u16), cursor::Show)?;
        }
        Ok(())
    }

    // shortcut: rifà il layout markdown a ogni frame; mettere in cache per larghezza se i file sono grandi
    fn render_preview(&mut self, out: &mut impl Write, w: usize) -> io::Result<()> {
        // stesso motivo del render normale: nessun carattere di controllo del file arriva al terminale
        let md: String = self.ed.lines.join("\n").chars().map(|c| if c.is_control() && c != '\n' && c != '\t' { '�' } else { c }).collect();
        let rendered = termimad::MadSkin::default().text(&md, Some(w)).to_string();
        let lines: Vec<&str> = rendered.lines().collect();
        self.ptop = self.ptop.min(lines.len().saturating_sub(self.height));
        for y in 0..self.height {
            queue!(out, cursor::MoveTo(0, y as u16))?;
            if let Some(l) = lines.get(self.ptop + y) {
                queue!(out, Print(l), SetAttribute(Attribute::Reset))?;
            }
            queue!(out, Clear(ClearType::UntilNewLine))?;
        }
        self.status(out, w, "PREVIEW", if self.kitty { "⌥⌘m edit  ⌃Q quit" } else { "⌃P edit  ⌃Q quit" })
    }

    fn status(&self, out: &mut impl Write, w: usize, info: &str, hint: &str) -> io::Result<()> {
        let name = self.path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
        let info = format!(" {name}{}  {info}", if self.ed.dirty { " ●" } else { "" });
        let hint = if self.msg.is_empty() { hint } else { &self.msg };
        let pad = w.saturating_sub(info.chars().count() + hint.chars().count() + 1);
        let bar: String = format!("{info}{}{hint} ", " ".repeat(pad)).chars().take(w).collect();
        queue!(out, cursor::MoveTo(0, self.height as u16), SetAttribute(Attribute::Reverse), Print(bar), SetAttribute(Attribute::Reset))
    }
}

// shortcut: ogni carattere conta 1 cella (CJK ed emoji risultano disallineati), passare a unicode-width se serve
fn advance(vc: usize, ch: char) -> usize {
    if ch == '\t' {
        (vc / TAB + 1) * TAB
    } else {
        vc + 1
    }
}

fn visual_col(line: &str, col: usize) -> usize {
    line.chars().take(col).fold(0, advance)
}

fn col_at(line: &str, vx: usize) -> usize {
    let mut vc = 0;
    for (i, ch) in line.chars().enumerate() {
        vc = advance(vc, ch);
        if vx < vc {
            return i;
        }
    }
    line.chars().count()
}

// shortcut: non preserva proprietario/xattr del file originale, solo i permessi
fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    // file temporaneo + rename: un crash a metà salvataggio non tronca l'originale
    let name = path.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let tmp = path.with_file_name(format!(".{}.ime-tmp", name.to_string_lossy()));
    let res = fs::write(&tmp, text)
        .and_then(|_| match fs::metadata(path) {
            Ok(meta) => fs::set_permissions(&tmp, meta.permissions()),
            Err(_) => Ok(()),
        })
        .and_then(|_| fs::rename(&tmp, path));
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res
}

fn pbcopy(s: &str) -> io::Result<()> {
    let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;
    child.stdin.take().unwrap().write_all(s.as_bytes())?;
    child.wait()?;
    Ok(())
}

fn pbpaste() -> Option<String> {
    Command::new("pbpaste").output().ok().and_then(|o| String::from_utf8(o.stdout).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_map_between_chars_and_cells() {
        assert_eq!(visual_col("a\tb", 2), 4);
        assert_eq!(col_at("a\tb", 2), 1); // clic dentro il tab → prima del tab
        assert_eq!(col_at("a\tb", 4), 2);
        assert_eq!(col_at("a\tb", 99), 3);
    }

    #[test]
    fn atomic_save_writes_and_keeps_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("ime-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("x.txt");
        fs::write(&f, "old").unwrap();
        fs::set_permissions(&f, fs::Permissions::from_mode(0o640)).unwrap();
        write_atomic(&f, "new").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "new");
        assert_eq!(fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o640);
        fs::remove_dir_all(&dir).unwrap();
    }
}
