#![warn(dead_code)]
#![allow(unused_braces)]
#![warn(unused)]            

use std::io::{self, Write, Stdout};
use clipboard::{self, ClipboardContext, ClipboardProvider};
use crossterm::cursor::{MoveTo};
use crossterm::{QueueableCommand};

#[derive(Default, Clone)]
pub struct Prompt {
    pub buffer: Vec<char>,
    pub cursor: usize,
}

impl Prompt {
    pub fn sync_terminal_cursor(&mut self, qc: &mut impl Write, x: usize, y: usize, w: usize) -> io::Result<()> {
        if let Some(_) = w.checked_sub(2) {
            let _ = qc.queue(MoveTo(x as u16 + self.cursor as u16, y as u16))?;
        }
        Ok(())
    }

    pub fn read_clipboard(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let mut ctx: ClipboardContext = ClipboardProvider::new()?;
        ctx.get_contents().map(|s| s.to_string())
    }
    
    pub fn insert(&mut self, x: char) {
        if self.cursor > self.buffer.len() {
            self.cursor = self.buffer.len()
        }
        self.buffer.insert(self.cursor, x);
        self.cursor += 1;
    }

    pub fn insert_str(&mut self, text: &str) {
        // println!("Clipboard content: {:?}", text);
        for x in text.chars() {
            if x != '\r' && x != '\n' {
                // println!("Inserting character: {:?}", x);
                self.insert(x)
            }
        }
    }

    pub fn left_char(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn right_char(&mut self) {
        if self.cursor < self.buffer.len() {
            self.cursor += 1;
        }
    }

    pub fn at_cursor(&self) -> char {
        self.buffer.get(self.cursor).cloned().unwrap_or('\n')
    }

    pub fn left_word(&mut self) {
        while self.cursor > 0 && self.at_cursor().is_whitespace() {
            self.cursor -= 1;
        }
        while self.cursor > 0 && !self.at_cursor().is_whitespace() {
            self.cursor -= 1;
        }
    }

    pub fn right_word(&mut self) {
        while self.cursor < self.buffer.len() && self.at_cursor().is_whitespace() {
            self.cursor += 1;
        }
        while self.cursor < self.buffer.len() && !self.at_cursor().is_whitespace() {
            self.cursor += 1;
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.buffer.remove(self.cursor);
        }
    }

    fn _before_cursor(&self) -> &[char] {
        &self.buffer[..self.cursor]
    }

    fn _after_cursor(&self) -> &[char] {
        &self.buffer[self.cursor..]
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
    }

    fn _delete_until_end(&mut self) {
        while self.cursor < self.buffer.len() {
            self.buffer.pop();
        }
    }
}

#[derive(Default, Clone)]
pub struct DownloadResults {
    dr: Vec<String>
}

impl DownloadResults {
    pub fn push(&mut self, string: String) {
        self.dr.push(string);
    }
    
    pub fn render(&mut self, height_bound: usize, stdout: &mut Stdout) {
        let n = self.dr.len();
        let m = n.checked_sub(height_bound).unwrap_or(0);
        for (index, dr) in self.dr.iter().skip(m).enumerate() {
            stdout.queue(MoveTo(0, index as u16)).unwrap();
            stdout.write(dr.as_bytes()).unwrap();
        }
    }
}
