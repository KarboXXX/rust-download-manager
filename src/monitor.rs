use std::{path::Path, sync::{Arc, Mutex}, thread};

use futures::{StreamExt, io};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;

use crate::downloading::download_file_in_pieces;

pub async fn monitor() -> io::Result<()> {
    let task_count = Arc::new(Mutex::new(0));
    
    let address = "127.0.0.1:6969";
    let listener = TcpListener::bind(address).await.expect("failed to bind to localhost");
    
    let ws_address = format!("ws://{address}");
    println!("Server listening on {}", ws_address);

    while let Ok((stream, address)) = listener.accept().await {
        println!("connection established with peer address {address}");
        let async_stream = accept_async(stream).await;
        
        if let Err(err) = async_stream {
            eprintln!("An error occurred: {:?}", err);
            continue;
        }

        let websocket = async_stream.unwrap();
        println!("websocket connected. listening for packets");
        let (_sink, mut stream) = websocket.split();
        
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
            let Some(url) = parsed["url"].as_str() else { continue };
            let Some(id) = parsed["id"].as_u64() else { continue };
            let Some(mime) = parsed["mime"].as_str() else { continue };

            let Some(filename) = url.split("/").last() else { continue };
            
            println!("Received event '{event}' from '{url}' for '{filename}' as '{mime}' ({id})");
            let Some(downloads_folder) = dirs::download_dir() else {
                eprintln!("Could not find default 'Downloads' folder.");
                continue;
            };

            let file_path = downloads_folder.join(filename);
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
