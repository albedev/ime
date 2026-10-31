//! Testo, cursore, selezione e undo. Niente I/O qui, così è tutto testabile.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub row: usize,
    pub col: usize, // indice in caratteri, non in byte
}

#[derive(Clone, Copy)]
pub enum Motion {
    Left,
    Right,
    Up(usize),
    Down(usize),
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    DocStart,
    DocEnd,
}

struct Snapshot {
    lines: Vec<String>,
    cursor: Pos,
}

pub struct Editor {
    pub lines: Vec<String>,
    pub cursor: Pos,
    pub anchor: Option<Pos>,
    pub dirty: bool,
    pub changed_from: usize, // prima riga modificata dall'ultimo render (usize::MAX = nessuna)
    crlf: bool,
    want_col: Option<usize>, // colonna "desiderata" mentre si va su/giù
    typing: bool,            // caratteri consecutivi = un solo passo di undo
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

fn byte_idx(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map_or(s.len(), |(i, _)| i)
}

pub const INDENT: &str = "    ";
const PAIRS: [(char, char); 6] = [('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\''), ('`', '`')];

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Editor {
    pub fn from_text(text: &str) -> Self {
        let crlf = text.contains("\r\n");
        let sep = if crlf { "\r\n" } else { "\n" };
        Self {
            lines: text.split(sep).map(String::from).collect(),
            cursor: Pos::default(),
            anchor: None,
            dirty: false,
            changed_from: usize::MAX,
            crlf,
            want_col: None,
            typing: false,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn to_text(&self) -> String {
        self.lines.join(if self.crlf { "\r\n" } else { "\n" })
    }

    fn char_at(&self, row: usize, col: usize) -> Option<char> {
        self.lines[row].chars().nth(col)
    }

    pub fn line_len(&self, row: usize) -> usize {
        self.lines[row].chars().count()
    }

    /// Selezione ordinata (inizio, fine), se non vuota.
    pub fn selection(&self) -> Option<(Pos, Pos)> {
        let a = self.anchor?;
        (a != self.cursor).then(|| (a.min(self.cursor), a.max(self.cursor)))
    }

    pub fn selected_text(&self) -> Option<String> {
        let (a, b) = self.selection()?;
        let first = &self.lines[a.row];
        if a.row == b.row {
            return Some(first[byte_idx(first, a.col)..byte_idx(first, b.col)].to_string());
        }
        let mut out = first[byte_idx(first, a.col)..].to_string();
        for l in &self.lines[a.row + 1..b.row] {
            out.push('\n');
            out.push_str(l);
        }
        let last = &self.lines[b.row];
        out.push('\n');
        out.push_str(&last[..byte_idx(last, b.col)]);
        Some(out)
    }

    // ---- movimento ----

    fn target(&self, m: Motion) -> Pos {
        let Pos { row, col } = self.cursor;
        let last = self.lines.len() - 1;
        let vertical = |r: usize| Pos { row: r, col: self.want_col.unwrap_or(col).min(self.line_len(r)) };
        match m {
            Motion::Left if col > 0 => Pos { row, col: col - 1 },
            Motion::Left if row > 0 => Pos { row: row - 1, col: self.line_len(row - 1) },
            Motion::Right if col < self.line_len(row) => Pos { row, col: col + 1 },
            Motion::Right if row < last => Pos { row: row + 1, col: 0 },
            Motion::Left | Motion::Right => self.cursor,
            // come su macOS: ↑ sulla prima riga va a inizio riga, ↓ sull'ultima a fine riga
            Motion::Up(_) if row == 0 => Pos { row, col: 0 },
            Motion::Up(n) => vertical(row.saturating_sub(n)),
            Motion::Down(_) if row == last => Pos { row, col: self.line_len(row) },
            Motion::Down(n) => vertical((row + n).min(last)),
            Motion::LineStart => Pos { row, col: 0 },
            Motion::LineEnd => Pos { row, col: self.line_len(row) },
            Motion::DocStart => Pos::default(),
            Motion::DocEnd => Pos { row: last, col: self.line_len(last) },
            Motion::WordLeft => self.word_left(),
            Motion::WordRight => self.word_right(),
        }
    }

    fn word_left(&self) -> Pos {
        let Pos { row, col } = self.cursor;
        if col == 0 {
            return if row > 0 { Pos { row: row - 1, col: self.line_len(row - 1) } } else { self.cursor };
        }
        let chars: Vec<char> = self.lines[row].chars().collect();
        let mut c = col;
        while c > 0 && !is_word(chars[c - 1]) {
            c -= 1;
        }
        while c > 0 && is_word(chars[c - 1]) {
            c -= 1;
        }
        Pos { row, col: c }
    }

    fn word_right(&self) -> Pos {
        let Pos { row, col } = self.cursor;
        let chars: Vec<char> = self.lines[row].chars().collect();
        if col >= chars.len() {
            return if row + 1 < self.lines.len() { Pos { row: row + 1, col: 0 } } else { self.cursor };
        }
        let mut c = col;
        while c < chars.len() && !is_word(chars[c]) {
            c += 1;
        }
        while c < chars.len() && is_word(chars[c]) {
            c += 1;
        }
        Pos { row, col: c }
    }

    pub fn go(&mut self, m: Motion, select: bool) {
        // come su macOS: ← / → senza Shift chiudono la selezione sul suo bordo
        if let (false, Some((a, b)), Motion::Left | Motion::Right) = (select, self.selection(), m) {
            return self.move_to(if matches!(m, Motion::Left) { a } else { b }, false);
        }
        let keep = matches!(m, Motion::Up(_) | Motion::Down(_)).then(|| self.want_col.unwrap_or(self.cursor.col));
        let t = self.target(m);
        self.move_to(t, select);
        self.want_col = keep;
    }

    pub fn move_to(&mut self, p: Pos, select: bool) {
        if select {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        let row = p.row.min(self.lines.len() - 1);
        self.cursor = Pos { row, col: p.col.min(self.line_len(row)) };
        self.want_col = None;
        self.typing = false;
    }

    pub fn select_all(&mut self) {
        self.move_to(Pos::default(), false);
        self.go(Motion::DocEnd, true);
    }

    pub fn select_word_at(&mut self, p: Pos) {
        self.move_to(p, false);
        let chars: Vec<char> = self.lines[self.cursor.row].chars().collect();
        let (mut a, mut b) = (self.cursor.col, self.cursor.col);
        while a > 0 && is_word(chars[a - 1]) {
            a -= 1;
        }
        while b < chars.len() && is_word(chars[b]) {
            b += 1;
        }
        if a == b && b < chars.len() {
            b += 1; // doppio clic su un simbolo: seleziona quel carattere
        }
        self.move_to(Pos { row: p.row, col: a }, false);
        self.move_to(Pos { row: p.row, col: b }, true);
    }

    pub fn select_line(&mut self, row: usize) {
        self.move_to(Pos { row, col: 0 }, false);
        let row = self.cursor.row;
        let end = if row + 1 < self.lines.len() { Pos { row: row + 1, col: 0 } } else { Pos { row, col: self.line_len(row) } };
        self.move_to(end, true);
    }

    // ---- modifica ----

    // shortcut: ogni passo di undo copia tutto il buffer; per file da decine di MB passare a un log di operazioni
    fn checkpoint(&mut self, typing: bool) {
        if !(typing && self.typing) {
            self.undo.push(Snapshot { lines: self.lines.clone(), cursor: self.cursor });
        }
        self.typing = typing;
        self.redo.clear();
        self.dirty = true;
    }

    fn remove(&mut self, a: Pos, b: Pos) {
        let tail = self.lines[b.row][byte_idx(&self.lines[b.row], b.col)..].to_string();
        let i = byte_idx(&self.lines[a.row], a.col);
        self.lines[a.row].truncate(i);
        self.lines[a.row].push_str(&tail);
        self.lines.drain(a.row + 1..=b.row);
        self.changed_from = self.changed_from.min(a.row);
        self.cursor = a;
        self.anchor = None;
    }

    pub fn insert(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let typing = self.selection().is_none() && text.chars().count() == 1 && text != "\n";
        self.checkpoint(typing);
        if let Some((a, b)) = self.selection() {
            self.remove(a, b);
        }
        self.anchor = None;
        let Pos { row, col } = self.cursor;
        self.changed_from = self.changed_from.min(row);
        let i = byte_idx(&self.lines[row], col);
        let tail = self.lines[row].split_off(i);
        let mut parts: Vec<String> = text.split('\n').map(String::from).collect();
        let first = parts.remove(0);
        self.lines[row].push_str(&first);
        let r = row + parts.len();
        self.lines.splice(row + 1..row + 1, parts);
        let c = self.line_len(r);
        self.lines[r].push_str(&tail);
        self.cursor = Pos { row: r, col: c };
        self.want_col = None;
    }

    /// Cancella la selezione, oppure dal cursore fino alla destinazione di `m`.
    pub fn delete(&mut self, m: Motion) {
        if self.selection().is_none() {
            let t = self.target(m);
            if t == self.cursor {
                return;
            }
            self.anchor = Some(t);
        }
        let (a, b) = self.selection().unwrap();
        self.checkpoint(false);
        self.remove(a, b);
    }

    pub fn undo(&mut self) {
        self.history(false)
    }

    pub fn redo(&mut self) {
        self.history(true)
    }

    fn history(&mut self, redo: bool) {
        let (from, to) = if redo { (&mut self.redo, &mut self.undo) } else { (&mut self.undo, &mut self.redo) };
        let Some(s) = from.pop() else { return };
        to.push(Snapshot { lines: std::mem::replace(&mut self.lines, s.lines), cursor: self.cursor });
        self.cursor = s.cursor;
        self.anchor = None;
        self.typing = false;
        self.dirty = true;
        self.changed_from = 0;
    }

    /// Un carattere digitato: chiude da solo ( [ { " ' ` e scavalca la chiusura già presente.
    pub fn type_char(&mut self, c: char) {
        let Pos { row, col } = self.cursor;
        let next = self.char_at(row, col);
        let prev = col.checked_sub(1).and_then(|c| self.char_at(row, c));
        if self.selection().is_none() && next == Some(c) && PAIRS.iter().any(|p| p.1 == c) {
            return self.go(Motion::Right, false);
        }
        if let Some(&(_, close)) = PAIRS.iter().find(|p| p.0 == c) {
            if let Some(sel) = self.selected_text() {
                return self.insert(&format!("{c}{sel}{close}")); // con selezione: la racchiude
            }
            // niente coppia davanti a una parola, né per l'apostrofo dopo una lettera (l'altro, don't)
            let blocked = next.is_some_and(is_word) || (c == close && prev.is_some_and(is_word));
            if !blocked {
                self.insert(&format!("{c}{close}"));
                self.cursor.col -= 1;
                return;
            }
        }
        self.insert(c.encode_utf8(&mut [0; 4]));
    }

    /// ⌫ semplice: dentro una coppia vuota "()" cancella entrambi.
    pub fn backspace(&mut self) {
        let Pos { row, col } = self.cursor;
        if self.selection().is_none() && col > 0 {
            let pair = (self.char_at(row, col - 1), self.char_at(row, col));
            if PAIRS.iter().any(|&(a, b)| pair == (Some(a), Some(b))) {
                self.anchor = Some(Pos { row, col: col - 1 });
                self.cursor.col += 1;
            }
        }
        self.delete(Motion::Left);
    }

    /// ↩ con rientro: mantiene l'indentazione e apre un blocco se il cursore è tra {} [] ().
    pub fn newline(&mut self) {
        let Pos { row, col } = self.cursor;
        let indent: String = self.lines[row].chars().take(col).take_while(|c| *c == ' ' || *c == '\t').collect();
        let pair = (col.checked_sub(1).and_then(|c| self.char_at(row, c)), self.char_at(row, col));
        if self.selection().is_none() && matches!(pair, (Some('{'), Some('}')) | (Some('['), Some(']')) | (Some('('), Some(')'))) {
            let unit = if indent.starts_with('\t') { "\t" } else { INDENT };
            self.insert(&format!("\n{indent}{unit}\n{indent}"));
            self.cursor = Pos { row: row + 1, col: indent.chars().count() + unit.chars().count() };
        } else {
            self.insert(&format!("\n{indent}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(row: usize, col: usize) -> Pos {
        Pos { row, col }
    }

    #[test]
    fn round_trip_preserves_line_endings() {
        for t in ["", "a\nb\n", "a\r\nb\r\n", "no newline"] {
            assert_eq!(Editor::from_text(t).to_text(), t);
        }
    }

    #[test]
    fn word_motion_like_macos() {
        let mut e = Editor::from_text("foo.bar  baz\nqux");
        e.go(Motion::WordRight, false);
        assert_eq!(e.cursor, at(0, 3));
        e.go(Motion::WordRight, false);
        assert_eq!(e.cursor, at(0, 7));
        e.go(Motion::LineEnd, false);
        e.go(Motion::WordRight, false);
        assert_eq!(e.cursor, at(1, 0));
        e.go(Motion::WordLeft, false);
        assert_eq!(e.cursor, at(0, 12));
        e.go(Motion::WordLeft, false);
        assert_eq!(e.cursor, at(0, 9));
    }

    #[test]
    fn multiline_insert_replace_and_undo() {
        let mut e = Editor::from_text("hello world");
        e.move_to(at(0, 6), false);
        e.go(Motion::LineEnd, true);
        assert_eq!(e.selected_text().as_deref(), Some("world"));
        e.insert("big\nnew"); // incolla sopra la selezione
        assert_eq!(e.to_text(), "hello big\nnew");
        assert_eq!(e.cursor, at(1, 3));
        e.undo();
        assert_eq!(e.to_text(), "hello world");
        e.redo();
        assert_eq!(e.to_text(), "hello big\nnew");
    }

    #[test]
    fn typing_is_one_undo_step_and_backspace_joins_lines() {
        let mut e = Editor::from_text("a\nb");
        e.move_to(at(1, 0), false);
        e.delete(Motion::Left);
        assert_eq!(e.to_text(), "ab");
        for c in ["x", "y", "z"] {
            e.insert(c);
        }
        assert_eq!(e.to_text(), "axyzb");
        e.undo();
        assert_eq!(e.to_text(), "ab");
    }

    #[test]
    fn unicode_columns_and_word_delete() {
        let mut e = Editor::from_text("però così");
        e.go(Motion::LineEnd, false);
        e.delete(Motion::WordLeft);
        assert_eq!(e.to_text(), "però ");
        e.select_word_at(at(0, 1));
        assert_eq!(e.selected_text().as_deref(), Some("però"));
    }

    #[test]
    fn auto_pairs() {
        let mut e = Editor::from_text("");
        e.type_char('(');
        assert_eq!((e.to_text().as_str(), e.cursor), ("()", at(0, 1)));
        e.type_char(')'); // scavalca invece di duplicare
        assert_eq!((e.to_text().as_str(), e.cursor), ("()", at(0, 2)));
        e.move_to(at(0, 1), false);
        e.backspace(); // coppia vuota: via entrambe
        assert_eq!(e.to_text(), "");
        for c in "l'a".chars() {
            e.type_char(c); // apostrofo dopo una lettera: nessuna coppia
        }
        assert_eq!(e.to_text(), "l'a");
        e.select_all();
        e.type_char('"'); // con selezione: la racchiude
        assert_eq!(e.to_text(), "\"l'a\"");
    }

    #[test]
    fn newline_keeps_indent_and_opens_blocks() {
        let mut e = Editor::from_text("    fn x() ");
        e.go(Motion::LineEnd, false);
        e.type_char('{');
        e.newline();
        assert_eq!(e.to_text(), "    fn x() {\n        \n    }");
        assert_eq!(e.cursor, at(1, 8));
        e.type_char('a');
        e.newline();
        assert_eq!(e.lines[2], "        ");
    }

    #[test]
    fn left_collapses_selection() {
        let mut e = Editor::from_text("abcdef");
        e.move_to(at(0, 1), false);
        e.go(Motion::WordRight, true);
        e.go(Motion::Left, false);
        assert_eq!((e.cursor, e.selection()), (at(0, 1), None));
    }
}
