#![warn(dead_code)]
#![allow(unused_braces)]
#![warn(unused)]

use crate::slice_from_start;

use std::collections::HashMap;
use std::io::{self};
use std::path::{PathBuf};
use std::time::Duration;
use std::sync::{Arc, Mutex};
use reqwest::header::{CONTENT_TYPE, CONTENT_DISPOSITION};
use tokio::{task::JoinHandle,  io::{AsyncWriteExt,},  fs::{File}};
use reqwest;
use is_url::is_url;
use indicatif::{ProgressBar, MultiProgress, ProgressStyle};
use directories::{UserDirs};

pub async fn download_chunk(
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

pub fn parse_filename_from_url(url_string: String) -> String {
    let url_vector = url_string.split('/').collect::<Vec<&str>>();
    let output_final_name = if url_string.chars().last().unwrap() == '/' {
        url_vector.len().checked_sub(2).map(|i| url_vector[i]).unwrap()
    } else {
        url_string.split('/').collect::<Vec<&str>>().last().copied().unwrap()
    };
    return String::from(output_final_name)
}

#[tokio::main] 
pub async fn download_file_in_pieces(url: &str, task_count: Arc<Mutex<usize>>)
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

    let final_filename_ = rename_index_filename(original_filename.as_str());
    
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

pub fn rename_index_filename(filename: &str) -> String {
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
