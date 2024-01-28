#![warn(dead_code)]
#![allow(unused_braces)]
#![warn(unused)]            

mod downloading;
mod interaction;
mod rendering;
use downloading::{download_file_in_pieces};
use interaction::{Prompt, DownloadResults};
use rendering::{RenderingManager};

use std::io::{self, Write, ErrorKind, Error};
use std::path::{Path, PathBuf};
use std::{thread};
use std::sync::{Arc, Mutex};
use std::io::{stdout};
use std::time::Duration;
use tokio::{net::{TcpListener}};
use tokio_tungstenite::{self, accept_async};
use futures::{StreamExt};
use is_url::is_url;
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::cursor::{MoveTo};
use crossterm::event::{self, poll, read, Event, KeyCode, KeyModifiers, EnableBracketedPaste,
                       DisableBracketedPaste, EnableMouseCapture, DisableMouseCapture};
use crossterm::{QueueableCommand, ExecutableCommand};
use directories::{UserDirs};


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
                        let (_sink, mut stream) = websocket.split();

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
