#![warn(dead_code)]
#![allow(unused_braces)]
#![warn(unused)]            

use std::collections::HashMap;
use std::io::{self, Write, Stdout};
// use std::fs::{self, File, OpenOptions};
use std::{thread};
use std::sync::{Arc, Mutex};
use std::io::{stdout};
use std::time::Duration;

use tokio::{self,
            task::JoinHandle,
            io::{AsyncWriteExt, AsyncReadExt},
            fs::File
};

use reqwest;
use is_url::is_url;

use indicatif::{ProgressBar, MultiProgress, ProgressStyle};

use crossterm::terminal::{self, Clear, ClearType};
use crossterm::cursor::{MoveTo};
use crossterm::event::{poll, read, Event, KeyCode, KeyModifiers};
use crossterm::{QueueableCommand, ExecutableCommand};

#[derive(Default)]
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
    
    fn insert(&mut self, x: char) {
        if self.cursor > self.buffer.len() {
            self.cursor = self.buffer.len()
        }
        self.buffer.insert(self.cursor, x);
        self.cursor += 1;
    }

    fn insert_str(&mut self, text: &str) {
        for x in text.chars() {
            self.insert(x)
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

#[derive(Default)]
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

fn slice_from_end(s: &str, n: usize) -> Option<&str> {
    s.char_indices().rev().nth(n).map(|(i, _)| &s[i..])
}

fn slice_from_start(s: String, n: usize) -> String {
    s.chars().into_iter().take(n).collect()
}

fn main() -> io::Result<()> {
    let mut stdout = stdout();

    let mut prompt = Prompt::default();
    
    let _ = terminal::enable_raw_mode().unwrap();

    let (mut w, mut h) = terminal::size().unwrap();
    let bar_char = "─";
    let mut bar = bar_char.repeat(w as usize);

    let mut download_requested: Vec<String> = Vec::new();
    let mut download_results = DownloadResults::default();

    let mut quit = false;

    let task_count = Arc::new(Mutex::new(0));

    while !quit {
        while poll(Duration::ZERO)? {
            match read()? {
                Event::Resize(nw, nh) => {
                    w = nw;
                    h = nh;
                    bar = bar_char.repeat(w as usize / bar_char.len());
                },
                Event::Paste(data) => prompt.insert_str(&data),
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
                        KeyCode::Enter => {
                            let temp_url = String::from_iter(&prompt.buffer);
                            let this_url = temp_url.clone();
                            
                            download_requested.push(temp_url);
                            let task_count = Arc::clone(&task_count);
                            
                            prompt.clear();

                            let _ = terminal::disable_raw_mode().unwrap();
                            // thread::scope(|s| {
                            //     s.spawn(|| {
                            // dbg!("thread started");
                            // let rt = runtime::Runtime::new().unwrap();

                            let display_name = format!("{}...", slice_from_start(this_url.to_string(), &this_url.len() - 15));

                            // download_file_in_pieces(
                            //     download_requested.get(download_requested.len() - 1).unwrap().as_str(),
                            //     format!("output{}", download_requested.len()).as_str(),
                            //     task_count
                            // );

                            
                            // let url = this_url.clone();
                            // let db = download_requested.len().clone();
                            
                            // download_file_in_pieces(
                            //     &url,
                            //     format!("output{}", db).as_str(),
                            //     task_count
                            // )

                            stdout.execute(Clear(ClearType::All)).unwrap();

                            thread::scope(|s| {
                                s.spawn(|| {
                                    let download_try = download_file_in_pieces(
                                        &this_url,
                                        task_count
                                    );
                                    if let Ok(_) = download_try {
                                        // println!("download thread finished");
                                        download_results.push(format!("{} finished", display_name));
                                        // terminal::enable_raw_mode().unwrap();
                                        // rt.shutdown_background()
                                    } else if let Err(reason) = download_try {
                                        // eprintln!("an error occurred on the download thread.");
                                        download_results.push(format!("{} failed. {}", display_name, reason));
                                        // rt.shutdown_background()
                                    }
                                    
                                    let _ = terminal::enable_raw_mode().unwrap();
                                    thread::sleep(Duration::from_secs(5));
                                });
                            });
                            
                            // println!("current concurrent download thread name={}",
                            //          current_download.thread().name().unwrap_or("no_name"));
                            // while !current_download.is_finished() {};

                            // download_results.push(String::from(format!("finished {}", display_name)));
                            
                            // terminal::enable_raw_mode().unwrap();
                            //     });
                            // });
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
                },
                _ => {}
            }
        }

        stdout.queue(Clear(ClearType::All)).unwrap();
        download_results.render(1, &mut stdout);

        stdout.queue(MoveTo(0, h-3)).unwrap();
        stdout.write(b"Type in the URL, and hit enter to start the download! ESC to exit and Ctrl+K to clear the prompt").unwrap();
        
        stdout.queue(MoveTo(0, h-2)).unwrap();
        stdout.write(bar.as_bytes()).unwrap();

        stdout.queue(MoveTo(0, h-1)).unwrap();
        stdout.write(String::from_iter(&prompt.buffer).as_bytes()).unwrap();

        // stdout.queue(MoveTo(terminal_cursor.x, terminal_cursor.y)).unwrap();
        if let Some(y) = h.checked_sub(1) {
            let x = 0;
            if let Some(w) = w.checked_sub(1) {
                prompt.sync_terminal_cursor(&mut stdout, x, y as usize, w as usize)?;
            }
        }
        
        stdout.flush().unwrap();
        thread::sleep(Duration::from_millis(33));
    };
    
    let _ = terminal::disable_raw_mode().unwrap();
    Ok(())
}

async fn download_chunk(
    client: reqwest::Client,
    url: String,
    start: usize,
    end: usize,
    // file: Arc<tokio::sync::Mutex<File>>,
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
            // dbg!(start, end);
            
            // pb.set_position(0 as u64);
            // let mut content = response.bytes().await.unwrap();
            // let mut content = Bytes::new().chunks(end - start);
            
            // println!("mutex lock for file unwraped on thread start={}", start);
            // let mut file = file.lock().await;  
            // let mut content: Vec<&[u8]> = vec!(&Bytes::new());
            
            let mut file = File::create(filename).await.unwrap();
            
            let mut progress: u64 = 0;
            pb.set_position(progress);
            
            while let Some(b) = response.chunk().await.unwrap() {    
                // let mut file = file.lock().await;  
                // let progress = (end - start) as u64 / b.len() as u64;

                progress += b.len() as u64;
                
                // println!("writing on thr start={} (chunk.length={}, progress={} of {})",
                //          start, b.len(), progress, (end - start));
                
                file.write(&b).await.unwrap();
                file.flush().await.unwrap();
                // drop(file);
                pb.set_position(progress);
            };

            // file.write_all(&mut content).await.unwrap();
            
            // println!("mutex lock for file dropped on thread start={}", start);

            drop(file);
            let mut count = task_count.lock().unwrap();
            *count -= 1;

            drop(count);
            
            pb.finish_and_clear();
            
            // while response_chunk.is_some() {
            //     // println!("Chunk: {:?}", chunk);
            //     file.write_all(response_chunk.unwrap().chunks()).await;
            //     pb.set_position(chunk);
            // }

            // pb.set_position(end as u64);
            return;
        }

        Err(err) => {
            eprintln!("Error downloading chunk: {:?}", err);
            return;
        }
    }

    // let mut count = task_count.lock().unwrap();
    // *count -= 1;

    // drop(count);
    // pb.finish();

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
                                 -> Result<(), String> {
    if !is_url(url) {
        return Err(format!("Invalid URL."));
    }
    
    let client: reqwest::Client = reqwest::Client::new();

    let response_future = client
        .get(url)
        .send()
        .await;

    if let Err(e) = response_future {
        return Err(format!("Connection refused."))
    };

    let response = response_future.unwrap();

    let total_size: u64 = response.content_length().unwrap_or(0);

    // let chunk_size: usize = (total_size / 3) as usize;
    let chunk_size: usize = if total_size > 1024 * 1024 * 40 {
        if total_size >= 1024 * 1024 * 1024 {
            (total_size / 6) as usize
        } else {
            (total_size / 3) as usize
        }
    } else {
        total_size as usize
    };
    
    // dbg!(total_size);

    // let file = Arc::new(
    //     tokio::sync::Mutex::new(
    //         File::create(output)
    //         .await
    //         .expect("Error creating output file.")
    //     )
    // );

    let mpb = MultiProgress::new();

    // let pb = ProgressBar::new(total_size);
    // pb.set_style(ProgressStyle::default_bar()
    //              .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
    //              .progress_chars("#>-"));

    let mut tasks: HashMap<(u64, usize), JoinHandle<()>> = HashMap::new();
    let mut pieces: HashMap<u16, String> = HashMap::new();
    
    let mut count = task_count.lock().unwrap();
    *count += ((total_size + (chunk_size as u64) - 1) / chunk_size as u64) as usize;
    drop(count);

    let mut index : u16 = 0;
    let original_filename = parse_filename_from_url(String::from(url));
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
        if let Err(e) = pb_styletry {
            return Err(format!("Could not set progress bar style"));
        };
        
        pb.set_style(pb_styletry.unwrap().progress_chars("#>-"));

        let pb_ = pb.clone();
        mpb.add(pb);

        let this_piece_filename_string = format!("{}.part{}", original_filename, start);
        let this_piece_filename = this_piece_filename_string.clone();
        pieces.insert(index, this_piece_filename_string);
        
        let task = tokio::spawn(
            download_chunk(client_, url_, start_, end, this_piece_filename, pb_, task_count_)
        );
            // download_chunk(client_, url_, start_, end, file_, task_count_)
        // );

        tasks.insert((start, end), task);
        index += 1;

        // let response = client
        //     .get(url)
        //     .header("Range", format!("bytes={}-{}", start, end))
        //     .send()
        //     .await?;

        // let mut content = response.bytes().await?;
        // file.write_all(&mut content)
        //     .await
        //     .or(Err(format!("Could not write data stream to output file.")))?;

        // pb.set_position(end as u64);
    }

    for ((start, end), task) in tasks {
        // println!("awaiting for task start={} end={}", start, end);
        let _running_task = task.await;
        // println!("task start={} finished", start);
    }

    let pb = ProgressBar::new(pieces.len() as u64);
    let pb_styletry = ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})");
    if let Err(e) = pb_styletry {
        return Err(format!("Could not set style for finishing progress bar. {}", e));
    }
    pb_styletry.unwrap().progress_chars("#>-");

    // let pb_ = pb.clone();
    pb.set_message("Finishing up");
    let pb_c = pb.clone();
    mpb.add(pb);

    // let url_string = String::from(url);
    // let url_vector = url_string.split('/').collect::<Vec<&str>>();
    // let output_final_name = if url_string.chars().last().unwrap() == '/' {
    //     url_vector.len().checked_sub(2).map(|i| url_vector[i]).unwrap()
    // } else {
    //     url_string.split('/').collect::<Vec<&str>>().last().copied().unwrap()
    // };
    
    let output_file_try = std::fs::File::create(
        parse_filename_from_url(String::from(url))
    );
    if let Err(e) = output_file_try {
        return Err(format!("Could not create final output file, does the running program has permission?"));
    };
    let mut output_file = output_file_try.unwrap();

    // let file = File::open(file_piece).await
    //     .expect("File part not found, did you remove or delete them? Finishing process could not complete.");

    let p__ = pieces.clone();
    dbg!(p__);
    
    for (index, (start_pos, file_piece)) in pieces.into_iter().enumerate() {
        // let mut buf = Vec::with_capacity(64);

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
                // _println!("bytes copied: {}", bytes);
                let this = file_piece_clone.clone();
                if let Err(e) = std::fs::remove_file(this) {
                    eprintln!("Could not remove file piece \"{}\" after successful output file finish step", file_piece_clone);
                }
            },
            Err(_e) => {
                // eprintln!("error: {}", e);
            }
        }
        
        // if let Ok(piece_file) = &mut piece_file_future {
        //     // piece.read -> write(output)
            
        //     match piece_file.read(&mut buf).await {
        //         Ok(bytes) => {
        //             while bytes == buf.len() {
        //                 match output_file.write(&buf).await {
        //                     Ok(bytes) => {
        //                         println!("wrote {} bytes to final file.", bytes);
        //                     },
        //                     Err(e) => {
        //                         eprintln!("could not stream bytes to final file.");
        //                     }
        //                 }
        //             }
        //         },
        //         Err(e) => {
        //             eprintln!("Could not parse file piece/part data. {}", e);
        //         }
        //     }
        // }
        
        // while let Ok(n) = output_file.read(&mut buf).await {
        //     let piece_file = File::open(&file_piece_);
        //     match &mut piece_file.await {
        //         Ok(piece) => {
        //             match piece.write(&buf).await {
        //                 Ok(_) => {
        //                     let pos = file_index as u64;
        //                     pb_c.set_position(pos);
        //                 },
        //                 Err(e) => {
        //                     eprintln!("Could not write file piece data to buffer.");
        //                     eprintln!("{}", e);
        //                 }
        //             }
        //         },
        //         Err(e) => {
        //             eprintln!("Could not retrieve data for file piece.");
        //             eprintln!("{}", e);
        //         }
                
        //     }
        // }

    };

    pb_c.finish_and_clear();
    // pb.finish();

    // println!("File download completed.");
    Ok(())
}
