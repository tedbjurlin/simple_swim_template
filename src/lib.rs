#![no_std]

use buffer::TextEditor;
use core::{fmt::Write, usize};
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
use simple_interp::{ArrayString, Interpreter, InterpreterOutput};

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
const MAX_TOKENS: usize = 100;
const MAX_LITERAL_CHARS: usize = 15;
const STACK_DEPTH: usize = 20;
const MAX_LOCAL_VARS: usize = 10;
const HEAP_SIZE: usize = 256;
const MAX_HEAP_BLOCKS: usize = HEAP_SIZE;
const SCHED_LATENCY: usize = 24;

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
    filename_input: ArrayString<MAX_FILENAME_BYTES>,
    creating_file: bool,
}

impl Default for SwimInterface {
    fn default() -> Self {
        let mut filesystem = FileSystem::new(RamDisk::new());
        create_default("hello", r#"print("Hello, world!")"#, &mut filesystem);
        create_default(
            "nums",
            r#"print(1)
print(257)
            "#,
            &mut filesystem,
        );
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
            filename_input: ArrayString::default(),
            creating_file: false,
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
            let (_, p, program_count) = self.min_vruntime();
            program_to_tick = p;
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
                    simple_interp::TickStatus::Finished => {}
                    simple_interp::TickStatus::AwaitInput => {
                        self.windows[program_to_tick].input_buffer = Default::default();
                        self.windows[program_to_tick].taking_input = true;
                        if self.windows[program_to_tick].interpreter_print_loc == 10 {
                            for row in self.windows[program_to_tick].window_y + 1
                                ..self.windows[program_to_tick].interpreter_print_loc
                                    + self.windows[program_to_tick].window_y
                            {
                                for col in self.windows[program_to_tick].window_x + 1
                                    ..WIN_WIDTH + self.windows[program_to_tick].window_x
                                {
                                    let (c, color) = peek(col, row + 1);
                                    plot(c, col, row, color);
                                }
                            }
                            for col in self.windows[program_to_tick].window_x + 1
                                ..WIN_WIDTH + self.windows[program_to_tick].window_x - 1
                            {
                                plot(
                                    ' ',
                                    col,
                                    self.windows[program_to_tick].interpreter_print_loc
                                        + self.windows[program_to_tick].window_y,
                                    ColorCode::new(Color::Black, Color::Black),
                                );
                            }
                            self.windows[program_to_tick].interpreter_print_loc -= 1;
                        }
                    }
                }
                self.windows[program_to_tick].vruntime += 1;
                self.windows[program_to_tick].interpreter = Some(interpreter);
            }
        }
    }

    fn min_vruntime(&mut self) -> (usize, usize, usize) {
        let mut min_vruntime = usize::MAX;
        let mut program_to_tick = 4;
        let mut num_programs = 0;
        for i in 0..4 {
            if self.windows[i].state == WindowState::Running {
                if let Some(interpreter) = &self.windows[i].interpreter {
                    if !interpreter.blocked_on_input() && !interpreter.completed() {
                        if self.windows[i].vruntime < min_vruntime {
                            min_vruntime = self.windows[i].vruntime;
                            program_to_tick = i;
                        }
                        num_programs += 1;
                    }
                }
            }
        }
        if min_vruntime == usize::MAX {
            (0, 4, 0)
        } else {
            (min_vruntime, program_to_tick, num_programs)
        }
    }

    fn draw_current(&mut self) {
        match self.windows[self.focused_editor].state {
            WindowState::Editing => {
                plot_str(
                    "Editing ",
                    0,
                    0,
                    ColorCode::new(Color::LightCyan, Color::Black),
                );
                plot_str(
                    core::str::from_utf8(&self.windows[self.focused_editor].current_file).unwrap(),
                    8,
                    0,
                    ColorCode::new(Color::LightCyan, Color::Black),
                );
            }
            WindowState::Running => {
                if self.windows[self.focused_editor].taking_input {
                    plot_str(
                        "Awaiting Input    ",
                        0,
                        0,
                        ColorCode::new(Color::LightCyan, Color::Black),
                    );
                } else {
                    plot_str(
                        "Running ",
                        0,
                        0,
                        ColorCode::new(Color::LightCyan, Color::Black),
                    );
                    plot_str(
                        core::str::from_utf8(&self.windows[self.focused_editor].current_file)
                            .unwrap(),
                        8,
                        0,
                        ColorCode::new(Color::LightCyan, Color::Black),
                    );
                }
            }
            WindowState::Listing => {
                plot_str(
                    "F5 - Filename: ",
                    0,
                    0,
                    ColorCode::new(Color::LightCyan, Color::Black),
                );
            }
        }
        for i in 0..4 {
            self.draw_outline(
                EDITOR_POSITION[i].0,
                EDITOR_POSITION[i].1,
                i == self.focused_editor,
            );
            plot(
                'F',
                EDITOR_POSITION[i].0 + 3,
                EDITOR_POSITION[i].1,
                ColorCode::new(Color::Green, Color::Black),
            );
            plot_num(
                (i + 1) as isize,
                EDITOR_POSITION[i].0 + 4,
                EDITOR_POSITION[i].1,
                ColorCode::new(Color::Green, Color::Black),
            );
            if i == self.focused_editor {
                match self.windows[i].state {
                    WindowState::Listing => {
                        plot_str(
                            " (e)dit (r)unÍÍÍÍÍÍÍÍÍÍÍÍÍÍÍÍ",
                            EDITOR_POSITION[i].0 + 5,
                            EDITOR_POSITION[i].1,
                            ColorCode::new(Color::Green, Color::Black),
                        );
                    }
                    _ => {
                        for j in 0..10 {
                            plot(
                                self.windows[self.focused_editor].current_file[j] as char,
                                EDITOR_POSITION[i].0 + 6 + j,
                                EDITOR_POSITION[i].1,
                                ColorCode::new(Color::Green, Color::Black),
                            );
                        }
                        plot_str(
                            " (F6 to exit)ÍÍÍÍÍ",
                            EDITOR_POSITION[i].0 + 16,
                            EDITOR_POSITION[i].1,
                            ColorCode::new(Color::Green, Color::Black),
                        );
                    }
                }
            } else {
                match self.windows[i].state {
                    WindowState::Listing => {
                        plot_str(
                            " (e)dit (r)unÄÄÄÄÄÄÄÄÄÄÄÄÄÄÄÄ",
                            EDITOR_POSITION[i].0 + 5,
                            EDITOR_POSITION[i].1,
                            ColorCode::new(Color::Green, Color::Black),
                        );
                    }
                    _ => {
                        for j in 0..10 {
                            plot(
                                self.windows[self.focused_editor].current_file[j] as char,
                                EDITOR_POSITION[i].0 + 6 + j,
                                EDITOR_POSITION[i].1,
                                ColorCode::new(Color::Green, Color::Black),
                            );
                        }
                        plot_str(
                            " (F6 to exit)ÄÄÄÄÄ",
                            EDITOR_POSITION[i].0 + 16,
                            EDITOR_POSITION[i].1,
                            ColorCode::new(Color::Green, Color::Black),
                        );
                    }
                }
            }
            self.windows[i].draw_window(&mut self.filesystem);
        }
        self.draw_processes();
    }

    fn draw_outline(&self, x: usize, y: usize, focused: bool) {
        for i in x + 1..x + 3 {
            if focused {
                plot(
                    205u8 as char,
                    i,
                    y,
                    ColorCode::new(Color::Green, Color::Black),
                );
            } else {
                plot(
                    196u8 as char,
                    i,
                    y,
                    ColorCode::new(Color::Green, Color::Black),
                );
            }
        }
        for i in x + 1..x + WIN_REGION_WIDTH / 2 - 1 {
            if focused {
                plot(
                    205u8 as char,
                    i,
                    y + 11,
                    ColorCode::new(Color::Green, Color::Black),
                );
            } else {
                plot(
                    196u8 as char,
                    i,
                    y + 11,
                    ColorCode::new(Color::Green, Color::Black),
                );
            }
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
            KeyCode::F6 => {
                self.windows[self.focused_editor].interpreter = None;
                self.windows[self.focused_editor].interpreter_print_loc = 0;
                self.windows[self.focused_editor].vruntime = 0;
                self.windows[self.focused_editor].state = WindowState::Listing;
                self.windows[self.focused_editor].clear_window();
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
            WindowState::Running => {
                if self.windows[self.focused_editor].taking_input {
                    if let Some(mut interpreter) = self.windows[self.focused_editor].interpreter {
                        match key {
                            '\n' => {
                                self.windows[self.focused_editor].vruntime = self.min_vruntime().0;
                                interpreter
                                    .provide_input(
                                        self.windows[self.focused_editor]
                                            .input_buffer
                                            .as_str()
                                            .unwrap(),
                                    )
                                    .unwrap_or_else(|e| {
                                        let mut err: ArrayString<80> = Default::default();
                                        write!(err, "{}", e).unwrap();
                                        self.windows[self.focused_editor]
                                            .print(err.as_str().unwrap().as_bytes());
                                    });
                                self.windows[self.focused_editor].interpreter_print_loc += 1;
                                self.windows[self.focused_editor].taking_input = false;
                            }
                            '\u{0008}' => self.windows[self.focused_editor].input_buffer.push_char('\u{0008}'),
                            k => {
                                if is_drawable(k) {
                                    self.windows[self.focused_editor].input_buffer.push_char(k);
                                }
                            }
                        }
                        self.windows[self.focused_editor].interpreter = Some(interpreter);
                    }
                }
            }
            WindowState::Listing => match key {
                'r' => {
                    self.windows[self.focused_editor].clear_window();
                    self.windows[self.focused_editor].vruntime = self.min_vruntime().0;
                    self.windows[self.focused_editor].state = WindowState::Running;
                    let mut filesystem_operations = || -> Result<(), FileSystemError> {
                        let (_, files) = self.filesystem.list_directory()?;
                        let filename = files[self.windows[self.focused_editor].focused_file];
                        let fd = self
                            .filesystem
                            .open_read(core::str::from_utf8(&filename).unwrap())?;
                        let mut buffer = [0; MAX_FILE_BYTES];
                        let num_bytes = self.filesystem.read(fd, &mut buffer)?;
                        let program = core::str::from_utf8(&buffer[0..num_bytes]).unwrap();
                        self.windows[self.focused_editor].run_program(program, filename);
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
            GenerationalHeap<HEAP_SIZE, MAX_HEAP_BLOCKS, 2>,
        >,
    >,
    interpreter_print_loc: usize,
    current_file: [u8; 10],
    state: WindowState,
    window_x: usize,
    window_y: usize,
    focused: bool,
    focused_file: usize,
    vruntime: usize,
    taking_input: bool,
    input_buffer: ArrayString<10>,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            editor: None,
            interpreter: None,
            interpreter_print_loc: Default::default(),
            current_file: Default::default(),
            state: Default::default(),
            window_x: Default::default(),
            window_y: Default::default(),
            focused: Default::default(),
            focused_file: Default::default(),
            vruntime: Default::default(),
            taking_input: false,
            input_buffer: Default::default(),
        }
    }
}

impl Window {
    pub fn make(x: usize, y: usize) -> Self {
        Self {
            editor: None,
            interpreter: None,
            window_x: x,
            window_y: y,
            ..Default::default()
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
            WindowState::Running => {
                if self.taking_input {
                    plot_str(
                        self.input_buffer.as_str().unwrap(),
                        self.window_x + 1,
                        self.window_y + 1 + self.interpreter_print_loc,
                        ColorCode::new(Color::LightCyan, Color::Black),
                    );
                    for i in self.input_buffer.len()..10 {
                        plot(
                            ' ',
                            self.window_x + 1 + i,
                            self.window_y + 1 + self.interpreter_print_loc,
                            ColorCode::new(Color::LightCyan, Color::Black),
                        );
                    }
                }
            }
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

    pub fn run_program(&mut self, program: &str, filename: [u8; 10]) {
        let interpreter = Interpreter::new(program);
        self.interpreter = Some(interpreter);
        self.current_file = filename;
    }

    pub fn clear_window(&mut self) {
        for col in self.window_x + 1..self.window_x + WIN_WIDTH {
            for row in self.window_y + 1..self.window_y + 11 {
                plot(' ', col, row, ColorCode::new(Color::Black, Color::Black));
            }
        }
    }
}

impl InterpreterOutput for Window {
    fn print(&mut self, chars: &[u8]) {
        if self.interpreter_print_loc == 10 {
            for row in self.window_y + 1..self.interpreter_print_loc + self.window_y {
                for col in self.window_x + 1..WIN_WIDTH + self.window_x {
                    let (c, color) = peek(col, row + 1);
                    plot(c, col, row, color);
                }
            }
            for col in self.window_x + 1..WIN_WIDTH + self.window_x - 1 {
                plot(
                    ' ',
                    col,
                    self.interpreter_print_loc + self.window_y,
                    ColorCode::new(Color::Black, Color::Black),
                );
            }
            self.interpreter_print_loc -= 1;
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
        } else if chars.len() != 0 {
            for i in 0..chars.len() - 1 {
                plot(
                    chars[i] as char,
                    i + self.window_x + 1,
                    self.interpreter_print_loc + self.window_y + 1,
                    ColorCode::new(Color::LightCyan, Color::Black),
                );
            }
        }
        self.interpreter_print_loc += 1;
    }
}
