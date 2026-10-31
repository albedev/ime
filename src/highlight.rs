//! Colori della sintassi via syntect (grammatiche Sublime incluse: Rust, Go, Python, JS, C, Markdown, ...).

use crossterm::style::Color;
use std::path::Path;
use syntect::{
    highlighting::{HighlightIterator, HighlightState, Highlighter, Theme, ThemeSet},
    parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet},
};

pub struct Syntax {
    set: SyntaxSet,
    theme: Theme,
    syntax: SyntaxReference,
    // stato del parser all'inizio di ogni riga già analizzata
    // shortcut: uno stato per riga costa memoria su file enormi; salvarne uno ogni N righe se serve
    states: Vec<(ParseState, HighlightState)>,
    base: usize, // riga a cui corrisponde states[0]
}

/// Righe analizzate sopra la vista dopo un salto lontano.
const SYNC: usize = 300;

impl Syntax {
    pub fn for_file(path: &Path, first_line: &str) -> Self {
        let set = SyntaxSet::load_defaults_newlines();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let file = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        let syntax = set
            .find_syntax_by_extension(ext)
            .or_else(|| set.find_syntax_by_extension(file)) // Makefile, Dockerfile...
            .or_else(|| set.find_syntax_by_first_line(first_line)) // #!/bin/bash
            .unwrap_or_else(|| set.find_syntax_plain_text())
            .clone();
        // shortcut: tema scuro fisso, su terminali chiari i colori rendono poco; aggiungere un tema chiaro se serve
        let theme = ThemeSet::load_defaults().themes.remove("base16-ocean.dark").expect("bundled theme");
        Self { set, theme, syntax, states: Vec::new(), base: 0 }
    }

    pub fn name(&self) -> &str {
        &self.syntax.name
    }

    /// Dopo una modifica alla riga `row` gli stati delle righe successive non valgono più.
    pub fn invalidate(&mut self, row: usize) {
        if row < self.base {
            self.states.clear();
        } else {
            self.states.truncate((row - self.base).saturating_add(1));
        }
    }

    /// Un colore per carattere, per le righe `from..to`.
    pub fn colors(&mut self, lines: &[String], from: usize, to: usize) -> Vec<Vec<Color>> {
        let to = to.min(lines.len());
        if self.syntax.name == "Plain Text" || from >= to {
            return Vec::new();
        }
        let hl = Highlighter::new(&self.theme);
        // salto lontano (⌘↓ su un file enorme): invece di analizzare tutto dall'inizio si riparte SYNC righe
        // sopra con uno stato pulito, come vim e micro; un commento lungo più di SYNC righe può uscire scolorito
        if from < self.base || from > self.base + self.states.len() + 2 * SYNC {
            self.base = from.saturating_sub(SYNC);
            self.states.clear();
        }
        if self.states.is_empty() {
            self.states.push((ParseState::new(&self.syntax), HighlightState::new(&hl, ScopeStack::new())));
        }
        let mut out = Vec::new();
        let mut buf = String::new();
        let start = from.min(self.base + self.states.len() - 1);
        for (r, line) in lines.iter().enumerate().take(to).skip(start) {
            buf.clear();
            buf.push_str(line);
            buf.push('\n'); // le grammatiche "newlines" vogliono il fine riga
            let (mut ps, mut hs) = self.states[r - self.base].clone();
            let ops = ps.parse_line(&buf, &self.set).unwrap_or_default();
            let mut colors = Vec::with_capacity(buf.len());
            for (style, text) in HighlightIterator::new(&mut hs, &ops, &buf, &hl) {
                let c = style.foreground;
                colors.extend(text.chars().map(|_| Color::Rgb { r: c.r, g: c.g, b: c.b }));
            }
            if r + 1 == self.base + self.states.len() {
                self.states.push((ps, hs));
            }
            if r >= from {
                out.push(colors);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_language_and_colors_incrementally() {
        let mut s = Syntax::for_file(Path::new("a.rs"), "");
        assert_eq!(s.name(), "Rust");
        let lines: Vec<String> = ["/* a", "b */ fn x() {}"].map(String::from).to_vec();
        let all = s.colors(&lines, 0, 2);
        assert_eq!(all[1].len(), lines[1].len() + 1);
        // la riga 1 inizia dentro un commento: partendo dalla cache deve dare gli stessi colori
        assert_eq!(s.colors(&lines, 1, 2)[0], all[1]);
        assert_ne!(all[1][0], all[1][6], "comment vs keyword should differ");
        assert_eq!(Syntax::for_file(Path::new("README.md"), "").name(), "Markdown");
    }

    #[test]
    fn far_jump_does_not_parse_whole_file() {
        let lines: Vec<String> = (0..50_000).map(|i| format!("let x{i} = {i}; // n")).collect();
        let mut s = Syntax::for_file(Path::new("a.rs"), "");
        let near = s.colors(&lines, 0, 5);
        let t = std::time::Instant::now();
        let far = s.colors(&lines, 49_990, 50_000);
        assert!(t.elapsed().as_secs_f32() < 1.0, "took {:?}", t.elapsed());
        assert_eq!(far.len(), 10);
        assert_eq!(far[0][..4], near[0][..4]); // `let` colorato uguale in cima e in fondo
        s.invalidate(10); // modifica sopra la finestra: si riparte da capo senza panico
        assert_eq!(s.colors(&lines, 0, 1).len(), 1);
    }
}

