# ime — Intuitive macOS editor

A fast terminal text editor that feels native on macOS: the same shortcuts you use in every Mac app, mouse and trackpad support, syntax highlighting and a markdown preview. Written in Rust.

```
ime notes.md
```

- **Mac keybindings out of the box**: ⌥←/→ by word, ⌘←/→ to line start/end, ⌘↑/↓ to start/end of file, Shift to select, ⌘Z / ⇧⌘Z, ⌘S, ⌘A, ⌘C/X/V.
- **Works with non-US keyboard layouts**: ⌥ keeps typing your characters (`⌥ò` → `@` on an Italian layout); editor commands use ⌥⌘ instead.
- **Mouse & trackpad**: click to place the cursor, drag to select, double-click for a word, triple-click for a line, ⇧-click to extend, smooth vertical and horizontal scrolling.
- **Syntax highlighting** for ~75 languages (Rust, Go, Python, JS/TS, PHP, C/C++, Markdown, shell, …).
- **Auto-closing** `( [ { " ' `` ` ``, typing over closers, auto-indent on ↩.
- **Markdown preview** (⌥⌘m) right in the terminal.
- **Safe saves**: written to a temp file and renamed, keeps permissions, CRLF line endings and the trailing newline.

---

## Install

```sh
brew install albedev/tap/ime
```

Universal binary for Apple Silicon and Intel Macs. To update: `brew upgrade ime`.

**From source** (needs a Rust toolchain from [rustup](https://rustup.rs)):

```sh
cargo install --git https://github.com/albedev/ime
```

## Usage

```sh
ime path/to/file      # opens it, or creates it on first save
```

The status bar shows the file name, a `●` when there are unsaved changes, the cursor position, the detected language, and the shortcuts that work **in your current terminal**.

---

## Shortcuts

ime has two sets of shortcuts:

- **⌘ shortcuts**, used when the terminal supports the [kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) and is set up as described below.
- **⌃ fallbacks**, which work in every terminal, including Terminal.app.

The status bar tells you which set is active.

### Moving and selecting

Add ⇧ (Shift) to any of these to select while moving.

| Action | Shortcut | Fallback |
|---|---|---|
| Character / line | ← → ↑ ↓ | |
| Word left / right | ⌥← / ⌥→ | |
| Line start / end | ⌘← / ⌘→ | ⌃A / ⌃E, Home / End |
| File start / end | ⌘↑ / ⌘↓ | ⌃Home / ⌃End |
| Page up / down | PgUp / PgDn (fn↑ / fn↓) | |
| Select all | ⌘A | — |
| Clear selection | Esc | |

### Editing

| Action | Shortcut | Fallback |
|---|---|---|
| Delete word left / right | ⌥⌫ / ⌥⌦ | |
| Delete to line start | ⌘⌫ | ⌃U |
| Delete to line end | | ⌃K |
| New line (keeps indentation, opens `{}` blocks) | ↩ | |
| Indent (4 spaces) | ⇥ | |
| Undo / redo | ⌘Z / ⇧⌘Z | ⌃Z / ⌃Y |
| Copy / cut | ⌘C / ⌘X | ⌃C / ⌃X |
| Paste | ⌘V | your terminal's paste |
| Save | ⌘S | ⌃S |
| Quit (press twice to discard unsaved changes) | ⌃Q | ⌃Q |

### Editor commands

| Action | Shortcut | Fallback |
|---|---|---|
| Show / hide line numbers | ⌥⌘n | ⌃N |
| Markdown preview on / off (`.md` files) | ⌥⌘m | ⌃P |

### Mouse & trackpad

| Action | Gesture |
|---|---|
| Place cursor | click |
| Select | drag |
| Select word / line | double-click / triple-click |
| Extend selection | ⇧-click |
| Scroll | two-finger swipe (vertical and horizontal) |

---

## Terminal setup

### Why setup is needed

A terminal only passes key presses to the program running inside it as text. Two macOS keys need special handling:

- **⌘ (Command).** Terminals normally keep ⌘ for themselves. Terminals that implement the **kitty keyboard protocol** can pass ⌘ to ime, and ime turns the protocol on automatically. But most terminals also use some ⌘ shortcuts for their own menus (⌘A = select the whole screen, for example). Those need to be freed or remapped.
- **⌥ (Option).** On non-US layouts ⌥ types characters you need (`@ # [ ] { } ~`). Don't enable "Option as Meta" or "Option as Alt". ime doesn't need it, because arrow keys with ⌥ work either way. Some ⌥ letters are *dead keys* (on the Italian layout ⌥n starts composing `ñ`), so a few ⌥⌘ shortcuts may need an explicit mapping (below).

### Compatibility

| Terminal | ⌘ shortcuts | Setup | Tested |
|---|---|---|---|
| [cmux](#cmux) | ✅ | 3 small changes | ✅ yes |
| [Ghostty](#ghostty) | ✅ | 4 config lines | ⚠️ not yet |
| [Terminal.app](#terminalapp) | ❌ never (no kitty protocol) | none, use the ⌃ fallbacks | ⚠️ fallback mode only |
| [iTerm2](#iterm2) | ✅ via key mappings | key mappings | ⚠️ not yet |
| [kitty](#kitty) | ✅ | 2 config lines | ⚠️ not yet |
| [WezTerm](#wezterm) | ✅ | 2 config lines | ⚠️ not yet |
| [Alacritty](#alacritty) | ✅ via key bindings | key bindings | ⚠️ not yet |
| [VS Code, JetBrains, tmux, SSH](#other-terminals) | ❌ | none, use the ⌃ fallbacks | ⚠️ not yet |

"Not yet" means the settings below follow each terminal's documentation but haven't been tried on a real machine. Reports and fixes are welcome.

### Universal fallback: send the key sequence yourself

If your terminal swallows a ⌘ shortcut, map it in the terminal's settings to **send this escape sequence**. ime understands these sequences in every terminal, even ones without the kitty protocol:

| Shortcut | Escape sequence | As text |
|---|---|---|
| ⌘A | `ESC [97;9u` | `\e[97;9u` |
| ⌘C | `ESC [99;9u` | `\e[99;9u` |
| ⌘X | `ESC [120;9u` | `\e[120;9u` |
| ⌘S | `ESC [115;9u` | `\e[115;9u` |
| ⌘Z | `ESC [122;9u` | `\e[122;9u` |
| ⇧⌘Z | `ESC [122;10u` | `\e[122;10u` |
| ⌥⌘n | `ESC [110;11u` | `\e[110;11u` |
| ⌥⌘m | `ESC [109;11u` | `\e[109;11u` |

The format is `ESC [ <key code> ; <modifiers> u`. The key code is the character's Unicode value (`a` = 97). The modifiers value is 1 + Shift 1 + ⌥ 2 + ⌃ 4 + ⌘ 8.

The catch: a remapped key sends this sequence to **every** program, so in a plain shell the shortcut may print junk like `[97;9u`.

---

### cmux

[cmux](https://github.com/manaflow-ai/cmux) is built on Ghostty's engine (it reports itself as `TERM_PROGRAM=ghostty`), but it has its own shortcuts and config files.

**1. Send ⌘A, ⌥⌘n and ⌥⌘m to ime.** Append to `~/Library/Application Support/com.cmuxterm.app/config.ghostty`:

```ini
# ime: send these keys straight to the program (skips menus and dead keys)
keybind = super+a=csi:97;9u
keybind = super+alt+n=csi:110;11u
keybind = super+alt+m=csi:109;11u
# ⌘C copies the terminal selection if there is one, otherwise goes to ime
keybind = performable:super+c=copy_to_clipboard
```

**2. Free ⌥⌘n.** cmux uses it for "New Browser Workspace". In `~/.config/cmux/cmux.json`, add right after `"schemaVersion": 1,`:

```jsonc
"shortcuts" : {
  "bindings" : {
    "newBrowserWorkspace" : ""
  }
},
```

**3. Free ⌘A from the "Select All" menu item.** Menu shortcuts win over keybinds, so move it to an unused combo (⌃⌥⇧⌘A):

```sh
defaults write com.cmuxterm.app NSUserKeyEquivalents -dict-add "Select All" '@~^$a'
```

Reload with ⌘⇧, after steps 1–2. Step 3 needs cmux to be **quit and reopened**.

**Trade-offs**

- ⌥⌘n no longer opens a browser workspace.
- In a plain shell, ⌘A, ⌥⌘n and ⌥⌘m print junk.
- ⌘A no longer selects the whole terminal (use ⌃⌥⇧⌘A).
- ⌘A may also stop selecting all in cmux's own text fields (address bar, search…).

**Undo**

```sh
defaults delete com.cmuxterm.app NSUserKeyEquivalents
# then remove the lines/blocks added in steps 1 and 2
```

### Ghostty

Add to `~/.config/ghostty/config` (or `~/Library/Application Support/com.mitchellh.ghostty/config.ghostty`):

```ini
# ime: send these keys straight to the program (skips menus and dead keys)
keybind = super+a=csi:97;9u
keybind = super+alt+n=csi:110;11u
keybind = super+alt+m=csi:109;11u
# ⌘C copies the terminal selection if there is one, otherwise goes to ime
keybind = performable:super+c=copy_to_clipboard
```

Reload with ⌘⇧,. Leave `macos-option-as-alt` unset: Ghostty already picks the right default for your layout.

Trade-off: in a plain shell ⌘A no longer selects the screen, and ⌘A / ⌥⌘n / ⌥⌘m print junk.

### Terminal.app

Terminal.app doesn't support the kitty keyboard protocol, so **⌘ shortcuts can never reach ime**. Use the ⌃ fallbacks instead; the status bar lists them. Everything else (⌥ word movement, Shift selection, mouse, trackpad, ⌘V paste) works.

Settings → Profiles → your profile:

- **Keyboard** tab:
  - Leave "Use Option as Meta key" **off** on non-US layouts. ⌥←/→ still move by word, because Terminal sends `ESC b` / `ESC f` for them by default.
  - Line start/end: use ⌃A / ⌃E.
- **View menu**:
  - Make sure **Allow Mouse Reporting** is checked (⌘R toggles it). Without it, clicks and scrolling go to Terminal instead of ime.

Missing here: select all (⌘A). Select with the mouse or with ⇧ + movement keys instead.

### iTerm2

Settings → Profiles → your profile → **Keys**:

1. **General**: set *Left/Right Option key* to **Normal** on non-US layouts.
2. **Key Mappings** → *Presets…* → **Natural Text Editing**. This gives you ⌥←/→, ⌘←/→ and ⌘⌫ as line movement and deletion.
3. Add one mapping per ⌘ shortcut you want: **+** → press the shortcut → Action **Send Escape Sequence** → value from the [table above](#universal-fallback-send-the-key-sequence-yourself) **without** the leading `ESC`:

   | Shortcut | Send Escape Sequence |
   |---|---|
   | ⌘A | `[97;9u` |
   | ⌘S | `[115;9u` |
   | ⌘Z | `[122;9u` |
   | ⇧⌘Z | `[122;10u` |
   | ⌥⌘n | `[110;11u` |
   | ⌥⌘m | `[109;11u` |

   Leave ⌘C / ⌘V alone so normal copy & paste keeps working; use ⌃C / ⌃X to copy from ime.

Mouse: Profiles → Terminal → **Enable mouse reporting** (on by default).

### kitty

kitty is where the keyboard protocol comes from, so ⌘ reaches ime out of the box. In `~/.config/kitty/kitty.conf`:

```ini
# keep ⌥ typing characters on non-US layouts (this is the default)
macos_option_as_alt no
# ⌘C: copy kitty's selection if there is one, otherwise pass the key to ime
map cmd+c copy_or_noop
```

### WezTerm

In `~/.wezterm.lua`:

```lua
local wezterm = require 'wezterm'
local config = wezterm.config_builder()

config.enable_kitty_keyboard = true                         -- lets ⌘ reach ime
config.send_composed_key_when_left_alt_is_pressed = true    -- ⌥ò = @ on non-US layouts

return config
```

WezTerm keeps ⌘C / ⌘V for itself; use ⌃C / ⌃X to copy from ime.

### Alacritty

In `~/.config/alacritty/alacritty.toml`:

```toml
[window]
option_as_alt = "None"   # keep ⌥ typing characters on non-US layouts

[keyboard]
bindings = [
  { key = "A", mods = "Command",         chars = "\u001b[97;9u" },
  { key = "S", mods = "Command",         chars = "\u001b[115;9u" },
  { key = "Z", mods = "Command",         chars = "\u001b[122;9u" },
  { key = "Z", mods = "Command|Shift",   chars = "\u001b[122;10u" },
  { key = "N", mods = "Command|Option",  chars = "\u001b[110;11u" },
  { key = "M", mods = "Command|Option",  chars = "\u001b[109;11u" },
]
```

### Other terminals

**VS Code / JetBrains integrated terminals, tmux, SSH sessions**: use the ⌃ fallbacks. Mouse and trackpad work as long as the terminal has mouse reporting on.

- **tmux**: ⌘ shortcuts usually don't make it through tmux. Add `set -g mouse on` for mouse support.
- **SSH**: copy/cut use macOS `pbcopy`, so they only work when ime runs on your Mac, not on the remote machine.

---

## Troubleshooting

**See exactly what ime receives**

```sh
IME_KEYS=1 ime file.txt
```

The status bar then shows every key and mouse event as ime sees it. For ⌥⌘n you should see something like `Char('n') … ALT | SUPER`.

**Common problems**

| Problem | Fix |
|---|---|
| A shortcut does nothing and nothing shows up | The terminal (or one of its menus) took it: free it or map it as described above. |
| A ⌥⌘ shortcut only works with ⇧ | ⌥ + that letter is a dead key on your layout: map it to its escape sequence. |
| The status bar shows ⌃ shortcuts in a terminal that should support ⌘ | The terminal didn't answer the kitty protocol query (check it's enabled, e.g. WezTerm's `enable_kitty_keyboard`). |
| Clicking or scrolling doesn't work | Turn on mouse reporting in the terminal. |
| `ime: …: stream did not contain valid UTF-8` | ime only opens UTF-8 text files. |

## Known limitations

- Wide characters (CJK, emoji) count as one column, so the cursor drifts on those lines.
- The syntax colors use a fixed dark theme and look washed out on light terminal backgrounds.
- After a jump into a very large file, highlighting restarts 300 lines above the view. A comment or string longer than that can be colored wrong.
- Not there yet: find/replace, multiple files, soft wrap.

## Development

```sh
cargo test                  # unit tests
cargo run -- samples/test.md
```

`samples/` has a `.txt`, `.md` and `.php` file for trying the editor and its highlighting.

## License

[MIT](LICENSE) © 2026 Alberindo Loffrè
