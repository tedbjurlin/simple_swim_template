#![no_std]

use buffer::TextEditor;
use file_system_solution::{FileSystem, FileSystemError};
use gc_heap_template::{CopyingHeap, GenerationalHeap, OnceAndDoneHeap};
use num::Integer;
use pc_keyboard::{DecodedKey, KeyCode};
use pluggable_interrupt_os::{
    print, println,
    vga_buffer::{
        is_drawable, peek, plot, plot_num, plot_num_right_justified, plot_str, Color, ColorCode,
    },
};
use ramdisk::RamDisk;
use simple_interp::{Interpreter, InterpreterOutput};

use core::prelude::rust_2024::derive;

mod buffer;

const WIN_WIDTH: usize = (WIN_REGION_WIDTH - 4) / 2;
const EDITOR_POSITION: [(usize, usize); 4] = [
    (0, 1),
    (WIN_REGION_WIDTH / 2, 1),
    (0, 13),
    (WIN_REGION_WIDTH / 2, 13),
];
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
const MAX_TOKENS: usize = 500;
const MAX_LITERAL_CHARS: usize = 30;
const STACK_DEPTH: usize = 50;
const MAX_LOCAL_VARS: usize = 20;
const HEAP_SIZE: usize = 1024;
const MAX_HEAP_BLOCKS: usize = HEAP_SIZE;
const SCHED_LATENCY: usize = 48;

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
    running_countdown: usize,
    current_process: usize,
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
        let windows = [
            Window::make(EDITOR_POSITION[0].0, EDITOR_POSITION[0].1),
            Window::make(EDITOR_POSITION[1].0, EDITOR_POSITION[1].1),
            Window::make(EDITOR_POSITION[2].0, EDITOR_POSITION[2].1),
            Window::make(EDITOR_POSITION[3].0, EDITOR_POSITION[3].1),
        ];
        Self {
            windows,
            filesystem,
            focused_editor: 0,
            num_files: 4,
            running_countdown: 0,
            current_process: 0,
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
        let mut program_to_tick = 4;
        if self.running_countdown > 0 {
            if self.windows[self.current_process].state == WindowState::Running {
                if let Some(interpreter) = &self.windows[self.current_process].interpreter {
                    if !interpreter.blocked_on_input() && !interpreter.completed() {
                        program_to_tick = self.current_process;
                    }
                }
            }
            self.running_countdown -= 1;
        } else {
            let mut min_vruntime = usize::MAX;
            let mut program_count = 0;
            for i in 0..4 {
                if self.windows[i].state == WindowState::Running {
                    if let Some(interpreter) = &self.windows[i].interpreter {
                        if !interpreter.blocked_on_input() && !interpreter.completed() {
                            if self.windows[i].vruntime < min_vruntime {
                                min_vruntime = self.windows[i].vruntime;
                                program_to_tick = i;
                            }
                            program_count += 1;
                        }
                    }
                }
            }
            if program_to_tick != 4 {
                self.current_process = program_to_tick;
                self.running_countdown = SCHED_LATENCY / program_count;
            }
        }
        if program_to_tick != 4 {
            if let Some(mut interpreter) = self.windows[program_to_tick].interpreter {
                //print!("{}", interpreter.completed);
                match interpreter.tick(&mut self.windows[program_to_tick]) {
                    simple_interp::TickStatus::Continuing => (),
                    simple_interp::TickStatus::Finished => self.windows[self.focused_editor].print("completed".as_bytes()),
                    simple_interp::TickStatus::AwaitInput => (),
                }
                self.windows[program_to_tick].vruntime += 1;
            }
        }
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
        self.draw_processes();
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

    pub fn draw_processes(&mut self) {
        for i in 0..4 {
            plot(
                'F',
                WIN_REGION_WIDTH,
                i * 2,
                ColorCode::new(Color::LightCyan, Color::Black),
            );
            plot(
                (i + 49) as u8 as char,
                WIN_REGION_WIDTH + 1,
                i * 2,
                ColorCode::new(Color::LightCyan, Color::Black),
            );

            plot_num_right_justified(
                10,
                self.windows[i].vruntime as isize,
                WIN_REGION_WIDTH,
                i * 2 + 1,
                ColorCode::new(Color::LightCyan, Color::Black),
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
                    WindowState::Running => (),
                    WindowState::Listing => {
                        self.windows[self.focused_editor].focused_file =
                            (self.windows[self.focused_editor].focused_file + 1)
                                .mod_floor(&self.num_files);
                    }
                }
                //self.windows[self.focused_editor].move_cursor_right();
            }
            KeyCode::ArrowDown => {
                //self.windows[self.focused_editor].move_cursor_down();
            }
            KeyCode::ArrowLeft => {
                match self.windows[self.focused_editor].state {
                    WindowState::Editing => todo!(),
                    WindowState::Running => (),
                    WindowState::Listing => {
                        if self.num_files > 0 {
                            self.windows[self.focused_editor].focused_file =
                                (self.windows[self.focused_editor].focused_file + self.num_files
                                    - 1)
                                .mod_floor(&self.num_files);
                        }
                    }
                }
                //self.windows[self.focused_editor].move_cursor_left();
            }
            _ => {}
        }
    }

    fn handle_unicode(&mut self, key: char) {
        match self.windows[self.focused_editor].state {
            WindowState::Editing => todo!(),
            WindowState::Running => todo!(),
            WindowState::Listing => match key {
                'r' => {
                    for col in self.windows[self.focused_editor].window_x + 1..self.windows[self.focused_editor].window_x + WIN_WIDTH - 1 {
                        for row in self.windows[self.focused_editor].window_y + 1..self.windows[self.focused_editor].window_y + 9 {
                            plot(' ', col, row, ColorCode::new(Color::Black, Color::Black));
                        }
                    }
                    self.windows[self.focused_editor].state = WindowState::Running;
                    let mut filesystem_operations = || -> Result<(), FileSystemError> {
                        let (_, files) = self.filesystem.list_directory()?;
                        let filename = files[self.windows[self.focused_editor].focused_file];
                        let fd = self.filesystem.open_read(core::str::from_utf8(&filename).unwrap())?;
                        let mut buffer = [0; MAX_FILE_BYTES];
                        let num_bytes = self.filesystem.read(fd, &mut buffer)?;
                        let program = core::str::from_utf8(&buffer[0..num_bytes]).unwrap();
                        self.windows[self.focused_editor].run_program(program);
                        self.filesystem.close(fd)?;
                        Ok(())
                    };
                    if let Err(_e) = filesystem_operations() {
                        self.windows[self.focused_editor].print("filesystem error".as_bytes());
                    }
                }
                _ => (),
            },
        }
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

#[derive(Default, Eq, PartialEq)]
enum WindowState {
    Editing,
    Running,
    #[default]
    Listing,
}

struct Window {
    editor: Option<TextEditor<WIN_WIDTH, DOCUMENT_LENGTH>>,
    interpreter: Option<
        Interpreter<
            MAX_TOKENS,
            MAX_LITERAL_CHARS,
            STACK_DEPTH,
            MAX_LOCAL_VARS,
            WIN_WIDTH,
            CopyingHeap<HEAP_SIZE, MAX_HEAP_BLOCKS>,
        >,
    >,
    interpreter_print_loc: usize,
    state: WindowState,
    window_x: usize,
    window_y: usize,
    focused: bool,
    focused_file: usize,
    vruntime: usize,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            editor: None,
            interpreter: None,
            interpreter_print_loc: Default::default(),
            state: Default::default(),
            window_x: Default::default(),
            window_y: Default::default(),
            focused: Default::default(),
            focused_file: Default::default(),
            vruntime: Default::default(),
        }
    }
}

impl Window {
    pub fn make(x: usize, y: usize) -> Self {
        Self {
            editor: None,
            interpreter: None,
            interpreter_print_loc: Default::default(),
            state: Default::default(),
            window_x: x,
            window_y: y,
            focused: Default::default(),
            focused_file: Default::default(),
            vruntime: Default::default(),
        }
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
            WindowState::Running => (),
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
                Err(e) => {
                    todo!()
                }
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

    pub fn run_program(&mut self, program: &str) {
        let interpreter = Interpreter::new(program);
        self.interpreter = Some(interpreter);
    }
}

impl InterpreterOutput for Window {
    fn print(&mut self, chars: &[u8]) {
        // if let Some(mut buffer) =
        // for i in 0..chars.len() {
        //     i / WIN_WIDTH;
        //     i % WIN_WIDTH;
        //     if let Some(buffer) = self.interpreter_buffer {
        //         buffer[self.interpreter_print_loc + i / WIN_WIDTH] = chars[i];
        //     }
        // }
        for row in self.window_y + 1..self.interpreter_print_loc + self.window_y + 1 {
            for col in self.window_x + 1..WIN_WIDTH + self.window_x - 1 {
                let (c, color) = peek(col, row);
                plot(c, col, row, color);
            }
        }
        if chars.len() > WIN_WIDTH - 2 {
            for i in 0..WIN_WIDTH - 2 {
                plot(
                    chars[i] as char,
                    i + self.window_x + 1,
                    self.interpreter_print_loc + self.window_y + 1,
                    ColorCode::new(Color::LightCyan, Color::Black),
                );
            }
            self.print(&chars[WIN_WIDTH - 2..]);
        } else {
            for i in 0..chars.len() {
                plot(
                    chars[i] as char,
                    i + self.window_x + 1,
                    self.interpreter_print_loc + self.window_y + 1,
                    ColorCode::new(Color::LightCyan, Color::Black),
                );
            }
        }
        if self.interpreter_print_loc <= 8 {
            self.interpreter_print_loc += 1;
        }
    }
}
