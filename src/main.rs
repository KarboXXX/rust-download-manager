#![warn(dead_code)]
#![allow(unused_braces)]
#![warn(unused)]            

use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::io::{self, Write, Stdout, ErrorKind, Error};
use std::path::{Path, PathBuf};
use std::{thread};
use std::sync::{Arc, Mutex};
use std::io::{stdout};
use std::time::Duration;
use reqwest::header::{HeaderMap, CONTENT_TYPE, CONTENT_DISPOSITION};
use tokio::{task::JoinHandle,  io::{AsyncWriteExt, AsyncRead, AsyncWrite},  fs::{File, self, remove_file},
            net::{TcpListener, TcpStream, TcpSocket}
};
use tokio_tungstenite::{self, accept_async};
use futures::{StreamExt, TryStreamExt};
use clipboard::{self, ClipboardContext, ClipboardProvider};
use reqwest;
use is_url::is_url;
use indicatif::{ProgressBar, MultiProgress, ProgressStyle};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::cursor::{MoveTo};
use crossterm::event::{self, poll, read, Event, KeyCode, KeyModifiers, EnableBracketedPaste,
                       DisableBracketedPaste, EnableMouseCapture, DisableMouseCapture};
use crossterm::{QueueableCommand, ExecutableCommand};
use directories::{UserDirs};

#[derive(Default, Clone)]
struct Prompt {
    buffer: Vec<char>,
    cursor: usize,
}

impl Prompt {
    fn sync_terminal_cursor(&mut self, qc: &mut impl Write, x: usize, y: usize, w: usize) -> io::Result<()> {
        if let Some(_) = w.checked_sub(2) {
            let _ = qc.queue(MoveTo(x as u16 + self.cursor as u16, y as u16))?;
        }
        Ok(())
    }

    fn read_clipboard(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let mut ctx: ClipboardContext = ClipboardProvider::new()?;
        ctx.get_contents().map(|s| s.to_string())
    }
    
    fn insert(&mut self, x: char) {
        if self.cursor > self.buffer.len() {
            self.cursor = self.buffer.len()
        }
        self.buffer.insert(self.cursor, x);
        self.cursor += 1;
    }

    fn insert_str(&mut self, text: &str) {
        // println!("Clipboard content: {:?}", text);
        for x in text.chars() {
            if x != '\r' && x != '\n' {
                // println!("Inserting character: {:?}", x);
                self.insert(x)
            }
        }
    }

    fn left_char(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn right_char(&mut self) {
        if self.cursor < self.buffer.len() {
            self.cursor += 1;
        }
    }

    fn at_cursor(&self) -> char {
        self.buffer.get(self.cursor).cloned().unwrap_or('\n')
    }

    fn left_word(&mut self) {
        while self.cursor > 0 && self.at_cursor().is_whitespace() {
            self.cursor -= 1;
        }
        while self.cursor > 0 && !self.at_cursor().is_whitespace() {
            self.cursor -= 1;
        }
    }

    fn right_word(&mut self) {
        while self.cursor < self.buffer.len() && self.at_cursor().is_whitespace() {
            self.cursor += 1;
        }
        while self.cursor < self.buffer.len() && !self.at_cursor().is_whitespace() {
            self.cursor += 1;
        }
    }

    fn backspace(&mut self) {
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

    fn clear(&mut self) {
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
struct DownloadResults {
    dr: Vec<String>
}

impl DownloadResults {
    fn push(&mut self, string: String) {
        self.dr.push(string);
    }
    
    fn render(&mut self, height_bound: usize, stdout: &mut Stdout) {
        let n = self.dr.len();
        let m = n.checked_sub(height_bound).unwrap_or(0);
        for (index, dr) in self.dr.iter().skip(m).enumerate() {
            stdout.queue(MoveTo(0, index as u16)).unwrap();
            stdout.write(dr.as_bytes()).unwrap();
        }
    }
}

#[derive(Default, Clone, Copy)]
struct RenderingManager {
    rendering: bool,
    w: u16,
    h: u16,
    fps: i16,
    interval: u64
}

impl RenderingManager {
    pub fn new(w: u16, h: u16, fps: i16) -> Self {
        RenderingManager {
            rendering: RenderingManager::check_from(w, h),
            w,
            h,
            fps,
            interval: RenderingManager::calculate_interval(fps)
        }
    }

    pub fn calculate_interval(fps: i16) -> u64 {
        (1000 / fps) as u64
    }

    pub fn new_fps(&mut self, fps: i16) -> u64 {
        self.fps = fps;
        self.interval = RenderingManager::calculate_interval(fps);
        self.interval
    }
    
    pub fn get_fps_interval(&mut self) -> u64 {
        self.interval
    }
    
    pub fn resize(&mut self, w: u16, h: u16) -> (u16, u16) {
        self.w = w;
        self.h = h;
        (w, h)
    }

    pub fn check(&mut self) -> bool {
        self.rendering = RenderingManager::check_from(self.w, self.h);
        self.rendering
    }
    
    pub fn check_from(w: u16, h: u16) -> bool {
        w > 20 && h > 20
    }

    pub fn is_enabled(&mut self) -> bool {
        self.rendering
    }
    
    // pub fn toggle(&mut self) -> bool {
    //     self.rendering = !self.rendering;
    //     self.rendering
    // }

    pub fn set(&mut self, new: bool) -> bool {
        self.rendering = new;
        new
    }
}

// fn slice_from_end(s: &str, n: usize) -> Option<&str> {
//     s.char_indices().rev().nth(n).map(|(i, _)| &s[i..])
// }

fn slice_from_start(s: String, n: usize) -> String {
    s.chars().into_iter().take(n).collect()
}

fn usage_message(error: bool) -> io::Result<()> {
    let command = std::env::args().into_iter().next().unwrap();
    println!("{} help - shows this message", command);
    println!("{} monitor - enter monitor mode and wait for downloads from your browser.\n", command);

    if error {return Err(std::io::Error::new(ErrorKind::Other, "invalid command"))} else {return Ok(())};
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let task_count = Arc::new(Mutex::new(0));

    if std::env::args().into_iter().len() > 1 {
        print!("\n");
        let argument = std::env::args().into_iter().nth(1).unwrap();
        if argument.contains("help") {
            return usage_message(false);
        } else if argument.contains("monitor") {
            let address = "127.0.0.1:6969";
            let listener = TcpListener::bind(address).await.expect("failed to bind to localhost");
            
            let ws_address = format!("ws://{address}");
            println!("Server listening on {}", ws_address);

            while let Ok((stream, address)) = listener.accept().await {
                println!("connection established with peer address {address}");
                match accept_async(stream).await {
                    Ok(websocket) => {
                        println!("websocket connected. listening for packets");
                        let (sink, mut stream) = websocket.split();

                        while let Some(reading) = stream.next().await {
                            match reading {
                                Ok(message) => {
                                    let msg = message.clone();
                                    if let Ok(packet) = message.into_text() {
                                        if packet.contains("{|}") {
                                            let splitted: Vec<&str> = packet.split("{|}").collect();
                                            if splitted.len() == 2 {
                                                let filename = splitted[1];
                                                let url = splitted[0];
                                                if is_url(url) {
                                                    // println!("URL: {url}, Filename: {filename}");
                                                    let file_path = filename;

                                                    let task_c = Arc::clone(&task_count);
                                                    thread::scope(|s| {
                                                        s.spawn(|| {
                                                            let download_try = download_file_in_pieces(url, task_c);
                                                            match download_try {
                                                                Ok(final_filename) => {
                                                                    println!("Downloaded finished. {final_filename}");
                                                                    let downloaded = Path::new(&final_filename);
                                                                    let destination = Path::new(&file_path);
                                                                    if let Err(e) = std::fs::rename(downloaded, destination) {
                                                                        eprintln!("An error occurred when moving file to default Downloads folder. {e} downloaded: {:?}, destination: {:?}", downloaded, destination);
                                                                    }
                                                                },
                                                                Err(e) => {
                                                                    eprintln!("ERROR: {e}");
                                                                }
                                                            }   
                                                        });
                                                    });
                                                    
                                                } else {
                                                    println!("New well-formatted packet without valid url: {filename} in {url}");
                                                }
                                            } else {
                                                println!("New packet with corret separator: {packet}");
                                            }
                                        } else {
                                            println!("New packet: {msg}");
                                        }
                                    } else {
                                        println!("A packet was received, but not on a text format.");
                                    }
                                },
                                Err(e) => {
                                    eprintln!("Failed to read message: {:?}", e);
                                }
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("An error occurred: {:?}", e);
                    }
                }
            }
            
            return Ok(())
        } else {
            return usage_message(true);
        }
    };
    
    let mut stdout = stdout();

    let mut prompt = Prompt::default();
    
    let _ = terminal::enable_raw_mode().unwrap();
    if let Err(r) = stdout.execute(EnableBracketedPaste) {
        eprintln!("Your terminal does not support bracket paste. {:?}", r);
        return Err(Error::new(ErrorKind::Other, format!("{:?}", r)));
    }

    let is_mouse_capture_enabled: bool = if let Err(_) = stdout.execute(EnableMouseCapture) { false } else { true };
    
    let mut rendering = {
        let (w, h) = terminal::size().unwrap();
        RenderingManager::new(w, h, 60)
    };
    
    let bar_char = "─";
    
    let message_start = if is_mouse_capture_enabled {
        "Type in the URL, and hit enter to start the download! ESC or Ctrl+C to exit and Ctrl+K to clear the prompt and Left click to paste"
    } else {
        "Type in the URL, and hit enter to start the download! ESC or Ctrl+C to exit and Ctrl+K to clear the prompt"
    };

    
    let mut download_requested: Vec<String> = Vec::new();
    let mut download_results: DownloadResults = DownloadResults::default();

    let mut quit = false;

    while !quit {
        let mut bar = bar_char.repeat(rendering.w as usize);
        
        while poll(Duration::from_millis(rendering.get_fps_interval() / 2))? {
            match read()? {
                Event::Resize(nw, nh) => {
                    rendering.resize(nw, nh);
                    bar = bar_char.repeat(rendering.w as usize / bar_char.len());
                    rendering.check();
                },
                Event::FocusGained => {
                    rendering.set(true);
                },
                Event::FocusLost => {
                    rendering.set(false);
                },
                Event::Paste(data) => prompt.insert_str(&data),
                Event::Mouse(event) => {
                    match event.kind {
                        event::MouseEventKind::Down(key) => {
                            match key {
                                event::MouseButton::Left => {
                                    if let Ok(data) = prompt.read_clipboard() {
                                        prompt.insert_str(data.as_str());
                                    }                                    
                                },
                                _ => {}
                            }
                            
                        },
                        _ => {}
                    };
                    // prompt.insert_str(&data)
                    // return Ok(())
                },
                Event::Key(event) => {
                    let key = event.code;
                    
                    match key {
                        KeyCode::Esc => {quit = true},
                        KeyCode::Backspace => prompt.backspace(),
                        KeyCode::Char(x) => {
                            if event.modifiers.contains(KeyModifiers::CONTROL) {
                                match x {
                                    'c' => quit = true,
                                    'k' => prompt.clear(),
                                    _ => {}
                                }
                            } else {
                                // url.push(x);
                                prompt.insert(x);
                            }
                        },
                        // #[cfg(feature = "bracketed-paste")]
                        KeyCode::Enter => {
                            if prompt.buffer.is_empty() {
                                let message = "URL vazio. Insira um URL válido";
                                stdout.queue(MoveTo(rendering.w/2 - (message.len() / 2) as u16, rendering.h/2)).unwrap();
                                stdout.write(message.as_bytes()).unwrap();
                                stdout.flush().unwrap();

                                thread::sleep(Duration::from_secs(5));
                                break;
                            }
                            
                            let temp_url = String::from_iter(&prompt.buffer);
                            let this_url = temp_url.clone();
                            
                            download_requested.push(temp_url);
                            let task_count = Arc::clone(&task_count);
                            
                            prompt.clear();
                            
                            if is_mouse_capture_enabled {
                                let _ = stdout.execute(DisableMouseCapture).unwrap();
                            }
                            let _ = terminal::disable_raw_mode().unwrap();
                            // thread::scope(|s| {
                            //     s.spawn(|| {
                            // dbg!("thread started");
                            // let rt = runtime::Runtime::new().unwrap();

                            // dbg!(this_url.to_string(), this_url.to_string().len(), this_url.len());

                            let display_name = if this_url.len() > (rendering.w/2 + 20) as usize {
                                let slice = slice_from_start(this_url.to_string(), ((rendering.w/2) - 20) as usize);
                                format!("{}...", slice)
                            } else {
                                this_url.to_string()
                            };

                            rendering.new_fps(20);
                            
                            stdout.queue(Clear(ClearType::All)).unwrap();
                            let loading_message = String::from("Loading, please wait...");
                            stdout.queue(MoveTo(rendering.w/2 - (loading_message.len() / 2) as u16, rendering.h/2)).unwrap();
                            stdout.write(loading_message.as_bytes()).unwrap();
                            stdout.queue(MoveTo(0,rendering.h-1)).unwrap();
                            stdout.flush().unwrap();
                            
                            thread::scope(|s| {
                                s.spawn(|| {
                                    let download_try = download_file_in_pieces(
                                        &this_url,
                                        task_count
                                    );
                                    if let Ok(filename) = download_try {
                                        download_results.push(format!("{} finished", display_name));
                                        stdout.queue(Clear(ClearType::All)).unwrap();

                                        if let Some(user_space) = UserDirs::new() {
                                            if let Some(download_dir) = user_space.download_dir() {
                                                let filename_ = filename.clone();
                                                let file_final = Path::new(filename.as_str());
                                                let mut destination = PathBuf::from(download_dir);
                                                destination.push(filename_);

                                                match std::fs::rename(file_final, destination.as_path()) {
                                                    Ok(_) => {
                                                        println!("Moved to default Downloads folder.");
                                                    },
                                                    Err(e) => {
                                                        eprintln!("Error moving file to default Downloads folder. ERR: {e}");
                                                    }
                                                }
                                            }                                            
                                        }
                                        
                                        let message = String::from("Sucess. {} is complete.");
                                        stdout.queue(MoveTo(rendering.w/2 - (message.len() / 2) as u16, rendering.h/2)).unwrap();
                                        stdout.write(loading_message.as_bytes()).unwrap();
                                        stdout.queue(MoveTo(0,rendering.h-1)).unwrap();
                                    } else if let Err(reason) = download_try {
                                        download_results.push(format!("{} failed. {}", display_name, reason));
                                        stdout.queue(Clear(ClearType::All)).unwrap();
                                        
                                        let message = String::from(format!("{} Failed.", display_name));
                                        stdout.queue(MoveTo(rendering.w/2 - (message.len() / 2) as u16, rendering.h/2)).unwrap();
                                        stdout.write(loading_message.as_bytes()).unwrap();
                                        stdout.queue(MoveTo(0,rendering.h-1)).unwrap();
                                    }

                                    stdout.flush().unwrap();
                                    let _ = terminal::enable_raw_mode().unwrap();
                                    rendering.new_fps(40);
                                    thread::sleep(Duration::from_secs(3));
                                });
                            });
                        },
                        KeyCode::Right => {
                            if event.modifiers.contains(KeyModifiers::CONTROL) {
                                prompt.right_word();
                            } else {
                                prompt.right_char();
                            }
                        },
                        KeyCode::Left => {
                            if event.modifiers.contains(KeyModifiers::SHIFT) {
                                prompt.left_word();
                            } else {
                                prompt.left_char();
                            }
                        },
                        _ => {}
                    }
                }
            }
        }

        if !rendering.is_enabled() {
            thread::sleep(Duration::from_millis(rendering.get_fps_interval() + 100));
            continue;
        };
        stdout.queue(Clear(ClearType::All)).unwrap();

        let offset = if message_start.len() > rendering.w as usize {
            (message_start.len() as f32 / rendering.w as f32).floor()
        } else {
            0f32
        };

        stdout.queue(MoveTo(0, rendering.h-(4 + offset as u16))).unwrap();
        stdout.write(bar.as_bytes()).unwrap();
        
        stdout.queue(MoveTo(0, rendering.h-(3 + offset as u16))).unwrap();
        
        stdout.write(message_start.as_bytes()).unwrap();

        stdout.queue(MoveTo(0, rendering.h-2)).unwrap();
        stdout.write(bar.as_bytes()).unwrap();

        stdout.queue(MoveTo(0, rendering.h-1)).unwrap();
        stdout.write(String::from_iter(&prompt.buffer).as_bytes()).unwrap();
        
        download_results.render((rendering.h-(4 + offset as u16)) as usize, &mut stdout);        
        
        // stdout.queue(MoveTo(terminal_cursor.x, terminal_cursor.y)).unwrap();
        if let Some(y) = rendering.h.checked_sub(1) {
            let x = 0;
            if let Some(w) = rendering.w.checked_sub(1) {
                prompt.sync_terminal_cursor(&mut stdout, x, y as usize, w as usize)?;
            }
        }
        
        stdout.flush().unwrap();
        thread::sleep(Duration::from_millis(rendering.get_fps_interval()));
    };  

    if is_mouse_capture_enabled {
        let _ = stdout.execute(DisableMouseCapture).unwrap();
    }
    
    let _ = terminal::disable_raw_mode().unwrap();
    let _ = stdout.execute(DisableBracketedPaste).unwrap();
    Ok(())
}

async fn download_chunk(
    client: reqwest::Client,
    url: String,
    content_type: String,
    start: usize,
    end: usize,
    filename: String,
    pb: ProgressBar,
    task_count: Arc<Mutex<usize>>
) -> () {
    let response = client
        .get(&url)
        .header("Range", format!("bytes={}-{}", start, end))
        .send()
        .await;

    match response {
        Ok(mut response) => {
            let mut file = File::create(filename).await.unwrap();
            
            let mut progress: u64 = 0;
            pb.set_position(progress);
            
            while let Some(b) = response.chunk().await.unwrap() {
                progress += b.len() as u64;
                
                file.write(&b).await.unwrap();
                file.flush().await.unwrap();
                pb.set_position(progress);
            };

            drop(file);
            let mut count = task_count.lock().unwrap();
            *count -= 1;

            drop(count);
            
            pb.finish_and_clear();
            return;
        }

        Err(_) => {
            eprintln!("Error downloading chunk: {:?} Trying again...", filename);
            let _ = download_chunk(client, url, content_type, start, end, filename, pb, task_count);
        }
    }

}

fn parse_filename_from_url(url_string: String) -> String {
    let url_vector = url_string.split('/').collect::<Vec<&str>>();
    let output_final_name = if url_string.chars().last().unwrap() == '/' {
        url_vector.len().checked_sub(2).map(|i| url_vector[i]).unwrap()
    } else {
        url_string.split('/').collect::<Vec<&str>>().last().copied().unwrap()
    };
    return String::from(output_final_name)
}

#[tokio::main] 
async fn download_file_in_pieces(url: &str, task_count: Arc<Mutex<usize>>)
                                 -> Result<String, String> {
    if !is_url(url) {
        return Err(format!("Invalid URL."));
    }
    
    let clientbuilder = reqwest::ClientBuilder::new()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/42.0.2311.135 Safari/537.36 Edge/12.246")
        .redirect(reqwest::redirect::Policy::limited(10))
        .connect_timeout(Duration::from_secs(30))
        .build();

    if let Err(e) = clientbuilder {
        return Err(format!("Could not create ClientBuilder object for connection initialization. {}", e));
    }

    let client = clientbuilder.unwrap();

    let response_future = client
        .get(url)
        .send()
        .await;

    if let Err(_) = response_future {
        return Err(format!("Connection refused."))
    };

    let parsed = parse_filename_from_url(String::from(url));
    
    let response = response_future.unwrap();
    if !response.status().is_success() {
        return Err(format!("Response status of request is not sucess."));
    }

    let original_filename = if let Some(content_disposition) = response.headers().get(CONTENT_DISPOSITION) {
        if let Ok(cd_str) = content_disposition.to_str() {
            if let Some(filename) = cd_str.split("filename=").nth(1) {
                println!("{filename}");
                let filename = filename.trim().trim_matches('"');
                String::from(filename)
            } else {
                if parsed.len() >= 20 {
                    let pars = parsed.clone();
                    let result = slice_from_start(pars, 20 - parsed.len());
                    result
                } else {
                    parsed
                }
            }
        } else {
            if parsed.len() >= 20 {
                let pars = parsed.clone();
                slice_from_start(pars, 20 - parsed.len())
            } else {
                parsed
            }
        }
    } else {
        if parsed.len() >= 20 {
            let pars = parsed.clone();
            slice_from_start(pars, 20 - parsed.len())
        } else {
            parsed
        }
    };
    
    let content_type = if let Some(content_type) = response.headers().get(CONTENT_TYPE) {
        if let Ok(content_type) = content_type.to_str() {
            println!("{content_type}");
            String::from(content_type)
        } else {
            String::from("")
        }
    } else {
        String::from("")
    };
    
    let total_size: u64 = response.content_length().unwrap_or(0);

    let chunk_size: usize = if total_size > 1024 * 1024 * 40 {
        if total_size >= 1024 * 1024 * 1024 {
            (total_size / 6) as usize
        } else {
            (total_size / 3) as usize
        }
    } else {
        total_size as usize
    };

    // if &content_type == "application/octet-stream" {
    //     let original_filename_ = original_filename.clone();
    //     let output_file = File::create(original_filename_).await;
    //     match output_file {
    //         Ok(mut output) => {
    //             match response.bytes().await {
    //                 Ok(bytes) => {
    //                     match tokio::io::copy(&mut bytes.as_ref(), &mut output).await {
    //                         Ok(_bytes_read) => {
    //                             return Ok(original_filename)
    //                         },
    //                         Err(e) => {
    //                             return Err(format!("Could not read byte stream from 'octet-stream'. {e}"))
    //                         }
    //                     }
    //                 },
    //                 Err(e) => {
    //                     return Err(format!("No bytes received from byte-stream (octet-stream). {e}"))
    //                 }
    //             }
    //         },
    //         Err(e) => {
    //             return Err(format!("Could not create output file. Application has permission? Free space? {e}"))
    //         }
    //     }
    // }

    let mpb = MultiProgress::new();

    let mut tasks: HashMap<(u64, usize), JoinHandle<()>> = HashMap::new();
    let mut pieces: HashMap<u16, String> = HashMap::new();
    
    let mut count = task_count.lock().unwrap();
    *count += ((total_size + (chunk_size as u64) - 1) / chunk_size as u64) as usize;
    drop(count);

    let mut index : u16 = 0;
    
    for start in (0..total_size).step_by(chunk_size) {
        let client_ = client.clone();
        let url_ = url.to_string();
        // let file_ = file.clone();
        // let pb_ = pb.clone();
        let start_ = start.clone() as usize;
        let task_count_ = Arc::clone(&task_count);

        let end = usize::min((start as usize) + chunk_size, total_size as usize) - 1;

        // println!("progressbar size={} for thread start={}", chunk_size, start);

        let pb = ProgressBar::new(chunk_size as u64);
        let pb_styletry = ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})");
        if let Err(_) = pb_styletry {
            return Err(format!("Could not set progress bar style."));
        };
        
        pb.set_style(pb_styletry.unwrap().progress_chars("#>-"));

        let pb_ = pb.clone();
        mpb.add(pb);

        let this_piece_filename_string = format!("{original_filename}.part{start}");
        
        let this_piece_filename = this_piece_filename_string.clone();
        pieces.insert(index, this_piece_filename_string);
        
        let ct = content_type.clone();
        
        let task = tokio::spawn(
            download_chunk(client_, url_, ct, start_, end, this_piece_filename, pb_, task_count_)
        );

        tasks.insert((start, end), task);
        index += 1;
    }

    for (_, task) in tasks {
        let _running_task = task.await;
    }

    let pb = ProgressBar::new(pieces.len() as u64);

    println!("Finishing up");
    let pb_c = pb.clone();
    mpb.add(pb);

    let mut final_filename_ = rename_index_filename(original_filename.as_str());
    
    let final_filename = final_filename_.clone();
    let output_file_try = std::fs::File::create(final_filename_);
    
    if let Err(e) = output_file_try {
        return Err(format!("Could not create final output file, does the running program has permission? {}", e));
    };
    
    let mut output_file = output_file_try.unwrap();
    
    for (index, (_, file_piece)) in pieces.into_iter().enumerate() {
        let file_piece_clone = file_piece.clone();
        let file_piece_ = file_piece_clone.clone();

        let piece_file_future = std::fs::File::open(&file_piece_);
        if let Err(e) = piece_file_future {
            return Err(format!("Could not open file part. {}", e));
        };
        
        let mut input = piece_file_future.unwrap();
        let status = io::copy(&mut input, &mut output_file);
        
        let pos = index as u64;
        pb_c.set_position(pos);
        match status {
            Ok(_bytes) => {
                let this = file_piece_clone.clone();
                if let Err(e) = std::fs::remove_file(this) {
                    return Err(
                        format!("Could not remove file piece \"{}\" after successful output file finish step. {}",
                                file_piece_clone,
                                e
                        )
                    );
                }
            },
            Err(e) => {
                return Err(format!("error: {}", e));
            }
        }
    };

    pb_c.finish_and_clear();
    Ok(final_filename)
}

fn rename_index_filename(filename: &str) -> String {
    // let mut new_filename = filename.to_string();
    let mut new_filename = if let Some(user_space) = UserDirs::new() {
        if let Some(download_dir) = user_space.download_dir() {
            let mut p = PathBuf::from(download_dir);
            p.push(filename);
            p.as_path().to_str().unwrap().to_string()
        } else {
            filename.to_string()
        }
    } else {
        filename.to_string()
    };
    
    let mut index = 1;

    while std::fs::metadata(&new_filename).is_ok() {
        let parts: Vec<&str> = filename.rsplitn(2, '.').collect();
        // let p = parts.clone();
        // dbg!(p);
        let (base_name, extension) = if parts.len() == 2 {
            (parts[1], parts[0])
        } else {
            ("", parts[0])
        };
        
        // new_filename = format!("{}({}).{}", base_name, index, extension);
        new_filename = if let Some(user_space) = UserDirs::new() {
            if let Some(download_dir) = user_space.download_dir() {
                let mut p = PathBuf::from(download_dir);
                let n = format!("{}({}).{}", base_name, index, extension);
                p.push(n);
                p.as_path().to_str().unwrap().to_string()
            } else {
                format!("{}({}).{}", base_name, index, extension)
            }
        } else {
            format!("{}({}).{}", base_name, index, extension)
        };
        
        index += 1;
        // let n = new_filename.clone();
        // dbg!(index, base_name, extension, n);
    }

    new_filename
}
