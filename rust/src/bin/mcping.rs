use anyhow::{anyhow, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Semaphore, Mutex};
use tokio::time::timeout;
use std::io::Write;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_PROTOCOL_VERSION: i32 = 800;

fn encode_varint(mut value: i32) -> Vec<u8> {
    let mut result = Vec::new();
    loop {
        let mut temp = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            temp |= 0x80;
        }
        result.push(temp);
        if value == 0 {
            break;
        }
    }
    result
}

async fn read_varint(stream: &mut TcpStream) -> Result<i32> {
    let mut result = 0i32;
    let mut shift = 0;
    
    for _ in 0..5 {
        let byte = stream.read_u8().await?;
        result |= ((byte & 0x7F) as i32) << shift;
        
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        
        shift += 7;
    }
    
    Err(anyhow!("VarInt too long"))
}

fn encode_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut result = encode_varint(bytes.len() as i32);
    result.extend_from_slice(bytes);
    result
}

fn create_handshake_packet(host: &str, port: u16) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&encode_varint(0x00));
    data.extend_from_slice(&encode_varint(MAX_PROTOCOL_VERSION));
    data.extend_from_slice(&encode_string(host));
    data.extend_from_slice(&port.to_be_bytes());
    data.extend_from_slice(&encode_varint(1));
    
    let mut packet = encode_varint(data.len() as i32);
    packet.extend_from_slice(&data);
    packet
}

fn create_status_request() -> Vec<u8> {
    vec![0x01, 0x00]
}

async fn is_minecraft_server(host: &str, port: u16) -> bool {
    let addr_result: Result<SocketAddr, _> = format!("{}:{}", host, port).parse();
    if addr_result.is_err() {
        return false;
    }
    
    let addr = addr_result.unwrap();
    
    let stream_result = timeout(DEFAULT_TIMEOUT, TcpStream::connect(addr)).await;
    if stream_result.is_err() {
        return false;
    }
    
    let mut stream = match stream_result.unwrap() {
        Ok(s) => s,
        Err(_) => return false,
    };
    
    let handshake = create_handshake_packet(host, port);
    if stream.write_all(&handshake).await.is_err() {
        return false;
    }
    if stream.flush().await.is_err() {
        return false;
    }
    
    let status_request = create_status_request();
    if stream.write_all(&status_request).await.is_err() {
        return false;
    }
    if stream.flush().await.is_err() {
        return false;
    }
    
    let packet_length_result = timeout(DEFAULT_TIMEOUT, read_varint(&mut stream)).await;
    if packet_length_result.is_err() {
        return false;
    }
    
    let packet_length = match packet_length_result.unwrap() {
        Ok(len) => len,
        Err(_) => return false,
    };
    
    packet_length > 0
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("📖 Reading check.txt...");
    let input = tokio::fs::read_to_string("check.txt").await?;
    let lines: Vec<String> = input
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .collect();
    
    println!("🔍 Minecraft Server Fast Ping Scanner");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 Total IPs to scan: {}", lines.len());
    println!();
    
    let concurrent_scans = 6000;
    
    let pb = ProgressBar::new(lines.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) | Found: {msg}")
            .unwrap()
            .progress_chars("█▓▒░"),
    );
    
    let semaphore = Arc::new(Semaphore::new(concurrent_scans));
    let found_count = Arc::new(Mutex::new(0u64));
    let output_file = Arc::new(Mutex::new(
        std::fs::File::create("mc.txt")?
    ));
    
    let mut handles = Vec::new();
    
    for line in lines {
        let (ip, port) = if let Some((host, port_str)) = line.split_once(':') {
            (host.to_string(), port_str.parse().unwrap_or(25565))
        } else {
            (line.clone(), 25565)
        };
        
        let semaphore = Arc::clone(&semaphore);
        let pb = pb.clone();
        let found_count = Arc::clone(&found_count);
        let output_file = Arc::clone(&output_file);
        
        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            
            let is_mc = is_minecraft_server(&ip, port).await;
            
            if is_mc {
                let server_addr = if port == 25565 {
                    format!("{}\n", ip)
                } else {
                    format!("{}:{}\n", ip, port)
                };
                
                let mut file = output_file.lock().await;
                let _ = file.write_all(server_addr.as_bytes());
                let _ = file.flush();
                drop(file);
                
                let mut count = found_count.lock().await;
                *count += 1;
                pb.set_message(format!("{}", *count));
            }
            
            pb.inc(1);
        });
        
        handles.push(handle);
    }
    
    for handle in handles {
        let _ = handle.await;
    }
    
    pb.finish_with_message("Done!");
    
    let final_count = *found_count.lock().await;
    
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📈 Scan Complete:");
    println!("   🎮 Minecraft servers found: {}", final_count);
    println!("   💾 Saved to: mc.txt");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    Ok(())
}