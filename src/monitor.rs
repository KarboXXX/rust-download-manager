use std::{path::{Path, PathBuf}, sync::{Arc, Mutex}, thread};

use futures::{SinkExt, StreamExt, io};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::downloading::download_file_in_pieces;

macro_rules! vprintln {
    ($verbose:expr, $($arg:tt)*) => {
        if $verbose {
            println!($($arg)*);
        }
    };
}

pub async fn monitor(verbose: bool) -> io::Result<()> {
    let task_count = Arc::new(Mutex::new(0));
    let threads = thread::available_parallelism().unwrap().get();

    let mut download_folder = dirs::download_dir().unwrap();
    let mut max_threads = threads.div_ceil(2);
    let mut min_threads = threads.div_ceil(3);
    
    let address = "127.0.0.1:6969";
    let listener = TcpListener::bind(address).await.expect("failed to bind to localhost");
    
    let ws_address = format!("ws://{address}");
    println!("Waiting handshake on {}", ws_address);

    while let Ok((stream, address)) = listener.accept().await {
        vprintln!(verbose, "connection established with peer address {address}");
        let async_stream = accept_async(stream).await;
        
        if let Err(err) = async_stream {
            eprintln!("An error occurred on initial websocket handshake: {:?}", err);
            continue;
        }

        let websocket = async_stream.unwrap();
        println!("websocket connected. listening channel...");
        let (mut sink, mut stream) = websocket.split();
        
        while let Some(reading) = stream.next().await {
            if let Err(err) = reading {
                eprintln!("Failed to read packet. {:?}", err);
                continue;
            }

            let message = reading.unwrap();
            let message_text = message.into_text();
            
            if let Err(err) = message_text {
                eprintln!("A packet arrived, but not in any compatible text format. {err}");
                continue;
            }
            
            let packet = message_text.unwrap();

            let json: Result<Value, serde_json::Error> = serde_json::from_str(&packet);
            if let Err(err) = json {
                eprintln!("Malformed JSON packet. {err}");
                continue;
            }

            let parsed = json.unwrap();

            let Some(event) = parsed["event"].as_str() else { continue };
            if event != "download_created" {
                // sidebar events
                vprintln!(verbose, "extension dispatched event '{event}'.");
                if event == "GET_DEFAULT_CONFIG" {
                    let config = json!({
                        "event": event,
                        "download": download_folder,
                        "download_threads_max": max_threads,
                        "download_threads_min": min_threads,
                        "total_threads": threads
                    });
                    let _ = sink.send(
                        Message::Text(config.to_string())
                    ).await;
                    continue;
                }
                if event == "SET_CONFIG" {
                    let Some(config) = parsed["config"].as_object() else { continue };

                    let Some(th_max) = config["download_threads_max"].as_u64() else { continue };
                    let Some(th_min) = config["download_threads_min"].as_u64() else { continue };
                    let Some(download) = config["download"].as_str() else { continue };

                    max_threads = th_max as usize;
                    min_threads = th_min as usize;
                    
                    if max_threads <= 0 {max_threads = 1}
                    if min_threads <= 0 {min_threads = 1}

                    if !Path::new(&download).exists() {
                        eprintln!("Directory '{}' does not exist.", download);
                        continue;
                    }
                    download_folder = PathBuf::from(download);

                    vprintln!(verbose, "Saved settings. (max/min threads = {}/{})", max_threads, min_threads);
                    let _ = sink.send(Message::Text(format!("Saved"))).await;
                    continue;
                }
                let _ = sink.send(Message::text(format!("PONG! --> {}", event))).await;
                continue;
            }
            
            let Some(url) = parsed["url"].as_str() else { continue };
            let Some(id) = parsed["id"].as_u64() else { continue };
            let Some(mime) = parsed["mime"].as_str() else { continue };

            let Some(filename) = url.split("/").last() else { continue };
            
            println!("Received event '{event}' from '{url}' for '{filename}' as '{mime}' ({id})");

            let file_path = download_folder.join(filename);
            let task_c = Arc::clone(&task_count);
            thread::scope(|s| {
                s.spawn(|| {
                    let download_try = download_file_in_pieces(url, task_c, max_threads, min_threads);
                    match download_try {
                        Ok(final_filename) => {
                            println!("Downloaded finished. {final_filename}");
                            let downloaded = Path::new(&final_filename);
                            let destination = Path::new(&file_path);
                            if let Err(e) = std::fs::rename(downloaded, destination) {
                                eprintln!("An error occurred when moving file to default Downloads folder. {e} downloaded: {:?}, destination: {:?}", downloaded, destination);
                                let _ = sink.send(Message::text(format!("Error when joining parts. {}", e)));
                                return Err(())
                            }
                        },
                        Err(e) => {
                            eprintln!("ERROR: {e}");
                            return Err(())
                        }
                    }
                    return Ok(())
                });
            }); 
        }
    }
    return Ok(())
}
