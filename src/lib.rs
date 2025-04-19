#![no_std]

use buffer::TextEditor;
use file_system_solution::FileSystem;
use num::Integer;
use pc_keyboard::{DecodedKey, KeyCode};
use pluggable_interrupt_os::vga_buffer::{is_drawable, plot, Color, ColorCode};
use ramdisk::RamDisk;
use simple_interp::Interpreter;

use core::prelude::rust_2024::derive;

mod buffer;

const WIN_WIDTH: usize = (WIN_REGION_WIDTH - 4) / 2;
const EDITOR_POSITION: [(usize, usize); 4] = [(0, 1), (WIN_REGION_WIDTH / 2, 1), (0, 13), (WIN_REGION_WIDTH / 2, 13)];
const TASK_MANAGER_WIDTH: usize = 10;
const WIN_REGION_WIDTH: usize = 80 - TASK_MANAGER_WIDTH;
const MAX_OPEN: usize = 16;
const BLOCK_SIZE: usize = 256;
const NUM_BLOCKS: usize = 255;
const MAX_FILE_BLOCKS: usize = 64;
const MAX_FILE_BYTES: usize = MAX_FILE_BLOCKS * BLOCK_SIZE;
const MAX_FILES_STORED: usize = 30;
const MAX_FILENAME_BYTES: usize = 10;
const DOCUMENT_LENGTH: usize = 40;

pub struct SwimInterface {
    windows: [Window; 4],
    filesystem: FileSystem<
        MAX_OPEN,
        BLOCK_SIZE,
        NUM_BLOCKS,
        MAX_FILE_BLOCKS,
        MAX_FILE_BYTES,
        MAX_FILES_STORED,
        MAX_FILENAME_BYTES,
    >,
    focused_editor: usize,
    num_files: usize,
}

impl Default for SwimInterface {
    fn default() -> Self {
        let mut filesystem = FileSystem::new(RamDisk::new());
        create_default("hello", r#"print("Hello, world!")"#, &mut filesystem);
        create_default("nums", r#"print(1)\nprint(257)"#, &mut filesystem);
        create_default(
            "average",
            r#"
sum := 0
count := 0
averaging := true
while averaging {
    num := input("Enter a number:")
    if (num == "quit") {
        averaging := false
    } else {
        sum := (sum + num)
        count := (count + 1)
    }
}
print((sum / count))
            "#,
            &mut filesystem,
        );
        create_default(
            "pi",
            r#"
sum := 0
i := 0
neg := false
terms := input("Num terms:")
while (i < terms) {
    term := (1.0 / ((2.0 * i) + 1.0))
    if neg {
        term := -term
    }
    sum := (sum + term)
    neg := not neg
    i := (i + 1)
}
print((4 * sum))
            "#,
            &mut filesystem,
        );
        let mut windows = [
            Window::default(),
            Window::default(),
            Window::default(),
            Window::default(),
        ];
        for i in 0..4 {
            windows[i].set_position(EDITOR_POSITION[i].0, EDITOR_POSITION[i].1);
        }
        Self {
            windows,
            filesystem,
            focused_editor: 0,
            num_files: 4,
        }
    }
}

fn create_default(
    filename: &str,
    contents: &str,
    filesystem: &mut FileSystem<
        MAX_OPEN,
        BLOCK_SIZE,
        NUM_BLOCKS,
        MAX_FILE_BLOCKS,
        MAX_FILE_BYTES,
        MAX_FILES_STORED,
        MAX_FILENAME_BYTES,
    >,
) {
    if let Ok(fd) = filesystem.open_create(filename) {
        if let Ok(()) = filesystem.write(fd, contents.as_bytes()) {
            filesystem.close(fd).unwrap_or(());
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
            plot(HEADER[i], i, 0, ColorCode::new(Color::Green, Color::Black));
        }
        for i in 0..4 {
            self.draw_outline(
                EDITOR_POSITION[i].0,
                EDITOR_POSITION[i].1,
                i == self.focused_editor,
            );
            plot(
                'F',
                EDITOR_POSITION[i].0 + WIN_REGION_WIDTH / 4 - 1,
                EDITOR_POSITION[i].1,
                ColorCode::new(Color::Green, Color::Black),
            );
            plot(
                (i + 49) as u8 as char,
                EDITOR_POSITION[i].0 + WIN_REGION_WIDTH / 4,
                EDITOR_POSITION[i].1,
                ColorCode::new(Color::Green, Color::Black),
            );
            self.windows[i].draw_window(&mut self.filesystem);
        }
    }

    fn draw_outline(&self, x: usize, y: usize, focused: bool) {
        for i in x + 1..x + WIN_REGION_WIDTH / 4 - 1 {
            for j in [y, y + 11] {
                if focused {
                    plot(
                        205u8 as char,
                        i,
                        j,
                        ColorCode::new(Color::Green, Color::Black),
                    );
                } else {
                    plot(
                        196u8 as char,
                        i,
                        j,
                        ColorCode::new(Color::Green, Color::Black),
                    );
                }
            }
        }
        for i in x + WIN_REGION_WIDTH / 4 + 1..x + WIN_REGION_WIDTH / 2 - 1 {
            for j in [y, y + 11] {
                if focused {
                    plot(
                        205u8 as char,
                        i,
                        j,
                        ColorCode::new(Color::Green, Color::Black),
                    );
                } else {
                    plot(
                        196u8 as char,
                        i,
                        j,
                        ColorCode::new(Color::Green, Color::Black),
                    );
                }
            }
        }
        if focused {
            plot(
                205u8 as char,
                x + WIN_REGION_WIDTH / 4 - 1,
                y + 11,
                ColorCode::new(Color::Green, Color::Black),
            );
            plot(
                205u8 as char,
                x + WIN_REGION_WIDTH / 4,
                y + 11,
                ColorCode::new(Color::Green, Color::Black),
            );
        } else {
            plot(
                196u8 as char,
                x + WIN_REGION_WIDTH / 4 - 1,
                y + 11,
                ColorCode::new(Color::Green, Color::Black),
            );
            plot(
                196u8 as char,
                x + WIN_REGION_WIDTH / 4,
                y + 11,
                ColorCode::new(Color::Green, Color::Black),
            );
        }
        for j in y + 1..y + 11 {
            for i in [x, x + WIN_REGION_WIDTH / 2 - 1] {
                if focused {
                    plot(
                        186u8 as char,
                        i,
                        j,
                        ColorCode::new(Color::Green, Color::Black),
                    );
                } else {
                    plot(
                        179u8 as char,
                        i,
                        j,
                        ColorCode::new(Color::Green, Color::Black),
                    );
                }
            }
        }
        if focused {
            plot(
                201u8 as char,
                x,
                y,
                ColorCode::new(Color::Green, Color::Black),
            );
            plot(
                187u8 as char,
                x + WIN_REGION_WIDTH / 2 - 1,
                y,
                ColorCode::new(Color::Green, Color::Black),
            );
            plot(
                200u8 as char,
                x,
                y + 11,
                ColorCode::new(Color::Green, Color::Black),
            );
            plot(
                188u8 as char,
                x + WIN_REGION_WIDTH / 2 - 1,
                y + 11,
                ColorCode::new(Color::Green, Color::Black),
            );
        } else {
            plot(
                218u8 as char,
                x,
                y,
                ColorCode::new(Color::Green, Color::Black),
            );
            plot(
                191u8 as char,
                x + WIN_REGION_WIDTH / 2 - 1,
                y,
                ColorCode::new(Color::Green, Color::Black),
            );
            plot(
                192u8 as char,
                x,
                y + 11,
                ColorCode::new(Color::Green, Color::Black),
            );
            plot(
                217u8 as char,
                x + WIN_REGION_WIDTH / 2 - 1,
                y + 11,
                ColorCode::new(Color::Green, Color::Black),
            );
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
                self.windows[self.focused_editor].set_focus(false);
                self.focused_editor = 0;
                self.windows[self.focused_editor].set_focus(true);
            }
            KeyCode::F2 => {
                self.windows[self.focused_editor].set_focus(false);
                self.focused_editor = 1;
                self.windows[self.focused_editor].set_focus(true);
            }
            KeyCode::F3 => {
                self.windows[self.focused_editor].set_focus(false);
                self.focused_editor = 2;
                self.windows[self.focused_editor].set_focus(true);
            }
            KeyCode::F4 => {
                self.windows[self.focused_editor].set_focus(false);
                self.focused_editor = 3;
                self.windows[self.focused_editor].set_focus(true);
            }
            KeyCode::ArrowUp => {
                //self.windows[self.focused_editor].move_cursor_up();
            }
            KeyCode::ArrowRight => {
                match self.windows[self.focused_editor].state {
                    WindowState::Editing => todo!(),
                    WindowState::Running => todo!(),
                    WindowState::Listing => {
                        self.windows[self.focused_editor].focused_file = (self.windows[self.focused_editor].focused_file + 1).mod_floor(&self.num_files);
                    },
                }
                //self.windows[self.focused_editor].move_cursor_right();
            }
            KeyCode::ArrowDown => {
                //self.windows[self.focused_editor].move_cursor_down();
            }
            KeyCode::ArrowLeft => {
                match self.windows[self.focused_editor].state {
                    WindowState::Editing => todo!(),
                    WindowState::Running => todo!(),
                    WindowState::Listing => {
                        if self.num_files > 0 {
                            self.windows[self.focused_editor].focused_file = (self.windows[self.focused_editor].focused_file + self.num_files - 1).mod_floor(&self.num_files);
                        }
                    },
                }
                //self.windows[self.focused_editor].move_cursor_left();
            }
            _ => {}
        }
    }

    fn handle_unicode(&mut self, key: char) {
        // match key {
        //     '\n' => self.windows[self.focused_editor].newline(),
        //     '\u{0008}' => self.windows[self.focused_editor].backspace_char(),
        //     '\u{007F}' => self.windows[self.focused_editor].delete_char(),
        //     k => {
        //         if is_drawable(k) {
        //             self.windows[self.focused_editor].push_char(key);
        //         }
        //     }
        // }
    }
}

#[derive(Default)]
enum WindowState {
    Editing,
    Running,
    #[default]
    Listing,
}

#[derive(Default)]
struct Window {
    editor: Option<TextEditor<WIN_WIDTH, DOCUMENT_LENGTH>>,
    //interpreter: Option<Interpreter<>>
    state: WindowState,
    window_x: usize,
    window_y: usize,
    focused: bool,
    focused_file: usize,
}

impl Window {
    pub fn set_position(&mut self, x: usize, y: usize) {
        self.window_x = x;
        self.window_y = y;
    }

    pub fn draw_window(
        &mut self,
        filesystem: &mut FileSystem<
            MAX_OPEN,
            BLOCK_SIZE,
            NUM_BLOCKS,
            MAX_FILE_BLOCKS,
            MAX_FILE_BYTES,
            MAX_FILES_STORED,
            MAX_FILENAME_BYTES,
        >,
    ) {
        match self.state {
            WindowState::Editing => todo!(),
            WindowState::Running => todo!(),
            WindowState::Listing => match filesystem.list_directory() {
                Ok((num_files, files)) => {
                    for i in 0..num_files {
                        for c in 0..MAX_FILENAME_BYTES {
                            if i == self.focused_file {
                                plot(
                                    files[i][c] as char,
                                    self.window_x + 1 + c + (i % 3 * MAX_FILENAME_BYTES),
                                    self.window_y + 1 + i / 3,
                                    ColorCode::new(Color::Black, Color::LightCyan),
                                );
                            } else {
                                plot(
                                    files[i][c] as char,
                                    self.window_x + 1 + c + (i % 3 * MAX_FILENAME_BYTES),
                                    self.window_y + 1 + i / 3,
                                    ColorCode::new(Color::LightCyan, Color::Black),
                                );
                            }
                        }
                    }
                }
                Err(e) => todo!(),
            },
        }
    }

    pub fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
        if let Some(mut editor) = self.editor {
            editor.focused = focused;
            self.editor = Some(editor);
        }
    }
}
