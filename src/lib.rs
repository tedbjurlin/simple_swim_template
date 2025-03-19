#![no_std]

use buffer::TextEditor;
use num::Integer;
use pc_keyboard::{DecodedKey, KeyCode};
use pluggable_interrupt_os::vga_buffer::{is_drawable, plot, Color, ColorCode};

use core::
    prelude::rust_2024::derive
;

mod buffer;

const EDITOR_POSITION: [(usize, usize); 4] = [(0, 1), (40, 1), (0, 13), (40, 13)];

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct SwimInterface {
    editors: [TextEditor; 4],
    focused_editor: usize,
}

impl Default for SwimInterface {
    fn default() -> Self {
        Self {
            editors: [
                TextEditor::new(38, 10, true),
                TextEditor::new(38, 10, false),
                TextEditor::new(38, 10, false),
                TextEditor::new(38, 10, false),
            ],
            focused_editor: 0,
        }
    }
}

pub fn safe_add<const LIMIT: usize>(a: usize, b: usize) -> usize {
    (a + b).mod_floor(&LIMIT)
}

pub fn add1<const LIMIT: usize>(value: usize) -> usize {
    safe_add::<LIMIT>(value, 1)
}

pub fn sub1<const LIMIT: usize>(value: usize) -> usize {
    safe_add::<LIMIT>(value, LIMIT - 1)
}

impl SwimInterface {

    pub fn tick(&mut self) {
        self.draw_current();
    }

    fn draw_current(&mut self) {
        const HEADER: [char; 6] = ['H', 'e', 'a', 'd', 'e', 'r'];
        for i in 0..6 {
            plot( HEADER[i], i, 0, ColorCode::new(Color::Green, Color::Black));
        }
        for i in 0..4 {
            self.draw_outline(EDITOR_POSITION[i].0, EDITOR_POSITION[i].1, i == self.focused_editor);
            plot('F', EDITOR_POSITION[i].0 + 19, EDITOR_POSITION[i].1, ColorCode::new(Color::Green, Color::Black));
            plot((i + 49) as u8 as char, EDITOR_POSITION[i].0 + 20, EDITOR_POSITION[i].1, ColorCode::new(Color::Green, Color::Black));
            self.editors[i].draw_window(EDITOR_POSITION[i].0, EDITOR_POSITION[i].1);
        }
    }

    fn draw_outline(&self, x: usize, y: usize, focused: bool) {
        for i in x + 1..x + 19 {
            for j in [y, y + 11] {
                if focused {
                    plot(205u8 as char, i, j, ColorCode::new(Color::Green, Color::Black));

                } else {
                    plot(196u8 as char, i, j, ColorCode::new(Color::Green, Color::Black));
                }
            }
        }
        for i in x + 21..x + 39 {
            for j in [y, y + 11] {
                if focused {
                    plot(205u8 as char, i, j, ColorCode::new(Color::Green, Color::Black));

                } else {
                    plot(196u8 as char, i, j, ColorCode::new(Color::Green, Color::Black));
                }
            }
        }
        if focused {
            plot(205u8 as char, x + 19, y + 11, ColorCode::new(Color::Green, Color::Black));
            plot(205u8 as char, x + 20, y + 11, ColorCode::new(Color::Green, Color::Black));

        } else {
            plot(196u8 as char, x + 19, y + 11, ColorCode::new(Color::Green, Color::Black));
            plot(196u8 as char, x + 20, y + 11, ColorCode::new(Color::Green, Color::Black));
        }
        for j in y + 1.. y + 11 {
            for i in [x, x + 39] {
                if focused {
                    plot(186u8 as char, i, j, ColorCode::new(Color::Green, Color::Black));

                } else {
                    plot(179u8 as char, i, j, ColorCode::new(Color::Green, Color::Black));
                }
            }
        }
        if focused {
            plot(201u8 as char, x, y, ColorCode::new(Color::Green, Color::Black));
            plot(187u8 as char, x + 39, y, ColorCode::new(Color::Green, Color::Black));
            plot(200u8 as char, x, y + 11, ColorCode::new(Color::Green, Color::Black));
            plot(188u8 as char, x + 39, y + 11, ColorCode::new(Color::Green, Color::Black));
        } else {
            plot(218u8 as char, x, y, ColorCode::new(Color::Green, Color::Black));
            plot(191u8 as char, x + 39, y, ColorCode::new(Color::Green, Color::Black));
            plot(192u8 as char, x, y + 11, ColorCode::new(Color::Green, Color::Black));
            plot(217u8 as char, x + 39, y + 11, ColorCode::new(Color::Green, Color::Black));
        }
    }

    pub fn key(&mut self, key: DecodedKey) {
        match key {
            DecodedKey::RawKey(code) => self.handle_raw(code),
            DecodedKey::Unicode(c) => self.handle_unicode(c),
        }
    }

    fn handle_raw(&mut self, key: KeyCode) {
        match key {
            KeyCode::F1 => {
                self.editors[self.focused_editor].focused = false;
                self.focused_editor = 0;
                self.editors[self.focused_editor].focused = true;
            },
            KeyCode::F2 => {
                self.editors[self.focused_editor].focused = false;
                self.focused_editor = 1;
                self.editors[self.focused_editor].focused = true;
            },
            KeyCode::F3 => {
                self.editors[self.focused_editor].focused = false;
                self.focused_editor = 2;
                self.editors[self.focused_editor].focused = true;
            },
            KeyCode::F4 => {
                self.editors[self.focused_editor].focused = false;
                self.focused_editor = 3;
                self.editors[self.focused_editor].focused = true;
            },
            KeyCode::ArrowUp => {
                self.editors[self.focused_editor].move_cursor_up();
            },
            KeyCode::ArrowRight => {
                self.editors[self.focused_editor].move_cursor_right();
            },
            KeyCode::ArrowDown => {
                self.editors[self.focused_editor].move_cursor_down();
            },
            KeyCode::ArrowLeft => {
                self.editors[self.focused_editor].move_cursor_left();
            },
            _ => {}
        }
    }

    fn handle_unicode(&mut self, key: char) {
        match key {
            '\n' => self.editors[self.focused_editor].newline(),
            '\u{0008}' => self.editors[self.focused_editor].backspace_char(),
            '\u{007F}' => self.editors[self.focused_editor].delete_char(),
            k => {
                if is_drawable(k) {
                    self.editors[self.focused_editor].push_char(key);
                }
            }

        }
    }
}
