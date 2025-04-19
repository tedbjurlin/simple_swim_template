use pluggable_interrupt_os::vga_buffer::{is_drawable, plot, Color, ColorCode};

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct TextEditor<const LINE_WIDTH: usize, const DOCUMENT_LENGTH: usize> {
    document: [[char; LINE_WIDTH]; DOCUMENT_LENGTH],
    cursor_col: usize,
    cursor_row: usize,
    target_col: usize,
    focus_x: usize,
    focus_y: usize,
    window_size_x: usize,
    window_size_y: usize,
    pub focused: bool,
}

impl<const LINE_WIDTH: usize, const DOCUMENT_LENGTH: usize> Default
    for TextEditor<LINE_WIDTH, DOCUMENT_LENGTH>
{
    fn default() -> Self {
        Self {
            window_size_x: LINE_WIDTH,
            window_size_y: DOCUMENT_LENGTH / 4,
            document: [[0u8 as char; LINE_WIDTH]; DOCUMENT_LENGTH],
            cursor_col: 0,
            cursor_row: 0,
            target_col: 0,
            focus_x: 0,
            focus_y: 0,
            focused: true,
        }
    }
}

impl<const LINE_WIDTH: usize, const DOCUMENT_LENGTH: usize>
    TextEditor<LINE_WIDTH, DOCUMENT_LENGTH>
{
    pub fn new(window_width: usize, window_height: usize, focused: bool) -> Self {
        Self {
            window_size_x: window_width,
            window_size_y: window_height,
            document: [[0u8 as char; LINE_WIDTH]; DOCUMENT_LENGTH],
            cursor_col: 0,
            cursor_row: 0,
            target_col: 0,
            focus_x: 0,
            focus_y: 0,
            focused,
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.document[self.cursor_row][self.cursor_col] = c;
        if self.cursor_col < self.window_size_x - 1 {
            self.cursor_col += 1;
        } else if self.cursor_row < self.window_size_y * 4 - 1 {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
        self.target_col = self.cursor_col;
    }

    pub fn backspace_char(&mut self) {
        if self.cursor_col != 0 || self.cursor_row != 0 {
            if self.cursor_col > 0 {
                self.cursor_col -= 1;
            } else if self.cursor_row > 0 {
                self.cursor_row -= 1;
                self.cursor_col = self.window_size_x - 1;
            }
            if self.document[self.cursor_row][self.cursor_col] == 0u8 as char {
                for i in 0..self.window_size_x {
                    if self.document[self.cursor_row][i] == 0u8 as char {
                        self.cursor_col = i;
                        break;
                    }
                }
            }
            self.shift();
        }
        self.target_col = self.cursor_col;
    }

    pub fn delete_char(&mut self) {
        if self.document[self.cursor_row][0] == 0u8 as char {
            self.delete_line();
        } else {
            self.shift();
        }
        self.target_col = self.cursor_col;
    }

    pub fn shift(&mut self) {
        for i in self.cursor_col..self.window_size_x {
            if self.document[self.cursor_row][i] == 0u8 as char {
                break;
            } else {
                if i + 1 == self.window_size_x {
                    self.document[self.cursor_row][i] = 0u8 as char;
                } else {
                    self.document[self.cursor_row][i] = self.document[self.cursor_row][i + 1];
                }
            }
        }
    }

    pub fn newline(&mut self) {
        if self.cursor_row + 1 != self.window_size_y * 4 {
            self.cursor_row += 1;
            self.cursor_col = 0;
            for i in self.window_size_y * 4..self.cursor_row {
                self.document[i] = self.document[i - 1];
            }
            self.document[self.cursor_row] = [0u8 as char; LINE_WIDTH];
        }
    }

    pub fn delete_line(&mut self) {
        for i in self.cursor_row..self.window_size_y * 4 - 1 {
            self.document[i] = self.document[i + 1]
        }
        self.document[self.window_size_y * 4 - 1] = [0u8 as char; LINE_WIDTH];
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            if self.target_col != self.cursor_col {
                self.cursor_col = self.target_col;
            }
            if self.document[self.cursor_row][self.cursor_col] == 0u8 as char {
                for i in 0..self.window_size_x {
                    if self.document[self.cursor_row][i] == 0u8 as char {
                        self.cursor_col = i;
                        break;
                    }
                }
            }
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_row < self.window_size_y * 4 - 1 {
            self.cursor_row += 1;
            if self.target_col != self.cursor_col {
                self.cursor_col = self.target_col;
            }
            if self.document[self.cursor_row][self.cursor_col] == 0u8 as char {
                for i in 0..self.window_size_x {
                    if self.document[self.cursor_row][i] == 0u8 as char {
                        self.cursor_col = i;
                        break;
                    }
                }
            }
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_col = self.window_size_x - 1;
            self.cursor_row -= 1;
            if self.document[self.cursor_row][self.cursor_col] == 0u8 as char {
                for i in 0..self.window_size_x {
                    if self.document[self.cursor_row][i] == 0u8 as char {
                        self.cursor_col = i;
                        break;
                    }
                }
            }
        }
        self.target_col = self.cursor_col;
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_col < self.window_size_x - 1
            && self.document[self.cursor_row][self.cursor_col] != 0u8 as char
        {
            self.cursor_col += 1;
        } else if self.cursor_row < self.window_size_y * 4 - 1 {
            self.cursor_col = 0;
            self.cursor_row += 1;
        }
        self.target_col = self.cursor_col;
    }

    pub fn draw_window(&mut self, window_x: usize, window_y: usize) {
        if self.cursor_row < self.focus_y && self.focus_y != 0 {
            self.focus_y = self.cursor_row;
        } else if self.cursor_row >= self.focus_y + self.window_size_y
            && self.focus_y + self.window_size_y < self.window_size_y * 4
        {
            self.focus_y = self.cursor_row - self.window_size_y + 1;
        }
        for y in 0..self.window_size_y {
            for x in 0..self.window_size_x {
                if self.cursor_col == x && self.cursor_row == y + self.focus_y && self.focused {
                    if is_drawable(self.document[y + self.focus_y][x]) {
                        plot(
                            self.document[y + self.focus_y][x],
                            window_x + x + 1,
                            window_y + y + 1,
                            ColorCode::new(Color::Black, Color::LightCyan),
                        );
                    } else {
                        plot(
                            ' ',
                            window_x + x + 1,
                            window_y + y + 1,
                            ColorCode::new(Color::Black, Color::LightCyan),
                        );
                    }
                } else if is_drawable(self.document[y + self.focus_y][x]) {
                    plot(
                        self.document[y + self.focus_y][x],
                        window_x + x + 1,
                        window_y + y + 1,
                        ColorCode::new(Color::LightCyan, Color::Black),
                    );
                } else {
                    plot(
                        ' ',
                        window_x + x + 1,
                        window_y + y + 1,
                        ColorCode::new(Color::LightCyan, Color::Black),
                    );
                }
            }
        }
    }
}
