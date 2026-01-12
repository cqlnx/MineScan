use anyhow::{anyhow, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use flate2::read::ZlibDecoder;
use std::io::Read;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);
const AUTH_TIMEOUT: Duration = Duration::from_secs(3);
const PROTOCOL_VERSION: i32 = 763;
const MAX_PROTOCOL_VERSION: i32 = 800;
const MIN_PROTOCOL_VERSION: i32 = 47;

#[derive(Debug, Serialize, Deserialize)]
struct ScanResult {
    ip: String,
    port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    motd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_players: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    online_players: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    players: Option<Vec<Player>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    favicon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_mode: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Player {
    name: String,
    uuid: String,
}

#[derive(Debug, Deserialize)]
struct ServerResponse {
    #[serde(default)]
    version: Option<VersionInfo>,
    #[serde(default)]
    players: Option<PlayersInfo>,
    #[serde(default)]
    description: Option<serde_json::Value>,
    #[serde(default)]
    favicon: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VersionInfo {
    name: String,
    protocol: i32,
}

#[derive(Debug, Deserialize)]
struct PlayersInfo {
    max: i32,
    online: i32,
    #[serde(default)]
    sample: Option<Vec<PlayerSample>>,
}

#[derive(Debug, Deserialize)]
struct PlayerSample {
    name: String,
    id: String,
}

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

fn create_handshake_packet(host: &str, port: u16, next_state: i32, protocol: i32) -> Vec<u8> {
    let mut data = Vec::new();
    
    data.extend_from_slice(&encode_varint(0x00));
    
    data.extend_from_slice(&encode_varint(protocol));
    
    data.extend_from_slice(&encode_string(host));

    data.extend_from_slice(&port.to_be_bytes());
    
    data.extend_from_slice(&encode_varint(next_state));
    
    let mut packet = encode_varint(data.len() as i32);
    packet.extend_from_slice(&data);
    
    packet
}

fn create_status_request() -> Vec<u8> {
    vec![0x01, 0x00]
}

fn create_login_start(username: &str, uuid: &str, protocol: i32) -> Vec<u8> {
    let mut data = Vec::new();
    
    data.extend_from_slice(&encode_varint(0x00));

    data.extend_from_slice(&encode_string(username));
    
    if protocol >= 47 && protocol <= 758 {
    } else if protocol == 759 {
        data.push(0x00);
    } else if protocol == 760 {
        data.push(0x00);
        data.push(0x01);
        let uuid_bytes = parse_uuid(uuid);
        data.extend_from_slice(&uuid_bytes);
    } else if protocol >= 761 && protocol <= 763 {
        data.push(0x01);
        let uuid_bytes = parse_uuid(uuid);
        data.extend_from_slice(&uuid_bytes);
    } else if protocol >= 764 {
        let uuid_bytes = parse_uuid(uuid);
        data.extend_from_slice(&uuid_bytes);
    } else {
    }
    
    let mut packet = encode_varint(data.len() as i32);
    packet.extend_from_slice(&data);
    
    packet
}

fn parse_uuid(uuid: &str) -> Vec<u8> {
    let clean = uuid.replace("-", "");
    (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).unwrap_or(0))
        .collect()
}

fn parse_motd(description: &serde_json::Value) -> String {
    match description {
        serde_json::Value::String(s) => strip_color_codes(s),
        serde_json::Value::Object(obj) => {
            let mut motd = String::new();
            
            if let Some(serde_json::Value::String(text)) = obj.get("text") {
                motd.push_str(&strip_color_codes(text));
            }
            
            if let Some(extra) = obj.get("extra") {
                motd.push_str(&parse_extra(extra));
            }
            
            motd
        }
        serde_json::Value::Array(arr) => {
            arr.iter()
                .filter_map(|v| {
                    if let serde_json::Value::Object(obj) = v {
                        obj.get("text")
                            .and_then(|t| t.as_str())
                            .map(|s| strip_color_codes(s))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("")
        }
        _ => String::new(),
    }
}

fn parse_extra(extra: &serde_json::Value) -> String {
    match extra {
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(|item| {
                if let serde_json::Value::Object(obj) = item {
                    obj.get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| strip_color_codes(s))
                        .unwrap_or_default()
                } else if let serde_json::Value::String(s) = item {
                    strip_color_codes(s)
                } else {
                    String::new()
                }
            })
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn strip_color_codes(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            result.push(c);
        }
    }
    
    result
}

async fn get_server_status(host: &str, port: u16) -> Result<ServerResponse> {
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    let mut stream = timeout(DEFAULT_TIMEOUT, TcpStream::connect(addr)).await??;
    
    let handshake = create_handshake_packet(host, port, 1, MAX_PROTOCOL_VERSION);
    stream.write_all(&handshake).await?;
    stream.flush().await?;
    
    let status_request = create_status_request();
    stream.write_all(&status_request).await?;
    stream.flush().await?;
    
    let _packet_length = read_varint(&mut stream).await?;
    let _packet_id = read_varint(&mut stream).await?;
    let json_length = read_varint(&mut stream).await?;
    
    let mut json_data = vec![0u8; json_length as usize];
    stream.read_exact(&mut json_data).await?;
    
    let response: ServerResponse = serde_json::from_slice(&json_data)?;
    
    Ok(response)
}

async fn get_auth_mode(host: &str, port: u16, protocol: i32) -> Result<i32> {
    if protocol < MIN_PROTOCOL_VERSION {
        return Err(anyhow!("Protocol version {} too old (< 1.8)", protocol));
    }
    
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    let mut stream = timeout(DEFAULT_TIMEOUT, TcpStream::connect(addr)).await??;
    
    let handshake = create_handshake_packet(host, port, 2, protocol);
    stream.write_all(&handshake).await?;
    stream.flush().await?;
    
    let login_start = create_login_start("popiiumaa", "00000000-0000-0000-0000-000000000000", protocol);
    stream.write_all(&login_start).await?;
    stream.flush().await?;
    
    let mut compression_threshold: i32 = -1;
    
    let result = timeout(AUTH_TIMEOUT, async {
        loop {
            let packet_length = read_varint(&mut stream).await?;
            if packet_length <= 0 {
                continue;
            }
            
            let mut packet_data = vec![0u8; packet_length as usize];
            stream.read_exact(&mut packet_data).await?;
            
            let packet_bytes = if compression_threshold >= 0 {
                let mut cursor = 0;
                let mut data_length = 0i32;
                let mut shift = 0;
                
                for i in 0..5 {
                    if cursor >= packet_data.len() {
                        return Err(anyhow!("Incomplete compressed packet"));
                    }
                    let byte = packet_data[cursor];
                    cursor += 1;
                    data_length |= ((byte & 0x7F) as i32) << shift;
                    if byte & 0x80 == 0 {
                        break;
                    }
                    shift += 7;
                    if i == 4 {
                        return Err(anyhow!("Data length varint too long"));
                    }
                }
                
                if data_length == 0 {
                    packet_data[cursor..].to_vec()
                } else {
                    let compressed_data = &packet_data[cursor..];
                    let mut decoder = ZlibDecoder::new(compressed_data);
                    let mut decompressed = Vec::new();
                    decoder.read_to_end(&mut decompressed)
                        .map_err(|e| anyhow!("Decompression failed: {}", e))?;
                    decompressed
                }
            } else {
                packet_data
            };
            
            if packet_bytes.is_empty() {
                continue;
            }
            
            let mut cursor = 0;
            let mut packet_id = 0i32;
            let mut shift = 0;
            
            for i in 0..5 {
                if cursor >= packet_bytes.len() {
                    return Err(anyhow!("Incomplete packet"));
                }
                let byte = packet_bytes[cursor];
                cursor += 1;
                packet_id |= ((byte & 0x7F) as i32) << shift;
                if byte & 0x80 == 0 {
                    break;
                }
                shift += 7;
                if i == 4 {
                    return Err(anyhow!("Packet ID varint too long"));
                }
            }
            
            match packet_id {
                0x00 => {
                    if cursor < packet_bytes.len() {
                        let mut string_len = 0i32;
                        let mut shift = 0;
                        for i in 0..5 {
                            if cursor >= packet_bytes.len() {
                                break;
                            }
                            let byte = packet_bytes[cursor];
                            cursor += 1;
                            string_len |= ((byte & 0x7F) as i32) << shift;
                            if byte & 0x80 == 0 {
                                break;
                            }
                            shift += 7;
                            if i == 4 {
                                break;
                            }
                        }
                        
                        if string_len > 0 && cursor + string_len as usize <= packet_bytes.len() {
                            let json_bytes = &packet_bytes[cursor..cursor + string_len as usize];
                            if let Ok(json_str) = std::str::from_utf8(json_bytes) {
                                let lower = json_str.to_lowercase();
                                if lower.contains("whitelist") || lower.contains("not whitelisted") {
                                    return Ok(2);
                                }
                            }
                        }
                    }
                    return Ok(2);
                }
                0x01 => {
                    return Ok(1);
                }
                0x02 => {
                    return Ok(0);
                }
                0x03 => {
                    let mut threshold = 0i32;
                    let mut shift = 0;
                    for i in 0..5 {
                        if cursor >= packet_bytes.len() {
                            break;
                        }
                        let byte = packet_bytes[cursor];
                        cursor += 1;
                        threshold |= ((byte & 0x7F) as i32) << shift;
                        if byte & 0x80 == 0 {
                            break;
                        }
                        shift += 7;
                        if i == 4 {
                            break;
                        }
                    }
                    compression_threshold = threshold;
                    continue;
                }
                0x04 => {
                    continue;
                }
                _ => {
                    continue;
                }
            }
        }
    })
    .await;
    
    match result {
        Ok(mode) => mode,
        Err(_) => Ok(-1),
    }
}

async fn scan_server(ip: String, port: u16, check_auth: bool) -> ScanResult {
    let scan_result = timeout(Duration::from_secs(10), async {
        let mut result = ScanResult {
            ip: ip.clone(),
            port,
            motd: None,
            version: None,
            protocol: None,
            max_players: None,
            online_players: None,
            players: None,
            favicon: None,
            auth_mode: None,
            error: None,
        };
        
        match get_server_status(&ip, port).await {
            Ok(response) => {
                if let Some(version) = response.version {
                    result.version = Some(version.name);
                    result.protocol = Some(version.protocol);
                }
                
                if let Some(players) = response.players {
                    result.max_players = Some(players.max);
                    result.online_players = Some(players.online);
                    
                    if let Some(sample) = players.sample {
                        result.players = Some(
                            sample
                                .into_iter()
                                .map(|p| Player {
                                    name: p.name,
                                    uuid: p.id,
                                })
                                .collect(),
                        );
                    }
                }
                
                if let Some(description) = response.description {
                    result.motd = Some(parse_motd(&description));
                }
                
                result.favicon = response.favicon;
                
                if check_auth {
                    if let Some(protocol) = result.protocol {
                        if protocol >= MIN_PROTOCOL_VERSION {
                            match get_auth_mode(&ip, port, protocol).await {
                                Ok(auth_mode) => result.auth_mode = Some(auth_mode),
                                Err(_) => {
                                    result.auth_mode = Some(-1);
                                }
                            }
                        } else {
                            result.auth_mode = Some(-1);
                        }
                    } else {
                        result.auth_mode = Some(-1);
                    }
                }
            }
            Err(e) => {
                result.error = Some(e.to_string());
            }
        }
        result
    })
    .await;
    
    match scan_result {
        Ok(r) => r,
        Err(_) => ScanResult {
            ip,
            port,
            motd: None,
            version: None,
            protocol: None,
            max_players: None,
            online_players: None,
            players: None,
            favicon: None,
            auth_mode: None,
            error: Some("Scan timeout".to_string()),
        },
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let input = tokio::fs::read_to_string("input.txt").await?;
    let lines: Vec<String> = input
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .collect();
    
    println!("🔍 Minecraft Server Scanner");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 Total servers to scan: {}", lines.len());
    println!();
    
    let check_auth = false;
    let concurrent_scans = 500;
    
    let multi_progress = MultiProgress::new();
    let main_pb = multi_progress.add(ProgressBar::new(lines.len() as u64));
    main_pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
            .unwrap()
            .progress_chars("█▓▒░"),
    );
    
    let stats_pb = multi_progress.add(ProgressBar::new(0));
    stats_pb.set_style(
        ProgressStyle::default_bar()
            .template("   ✓ {msg}")
            .unwrap()
    );
    
    let semaphore = Arc::new(Semaphore::new(concurrent_scans));
    
    let mut handles = Vec::new();
    let mut results = Vec::new();
    
    for line in lines {
        let (ip, port) = if let Some((host, port_str)) = line.split_once(':') {
            (host.to_string(), port_str.parse().unwrap_or(25565))
        } else {
            (line.clone(), 25565)
        };
        
        let semaphore = Arc::clone(&semaphore);
        let pb = main_pb.clone();
        let stats = stats_pb.clone();
        
        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            let result = scan_server(ip.clone(), port, check_auth).await;

            let success = result.error.is_none();
            pb.inc(1);
            
            if success {
                if let Some(version) = &result.version {
                    stats.set_message(format!(
                        "Success: {} | {} | Players: {}/{}",
                        ip,
                        version,
                        result.online_players.unwrap_or(0),
                        result.max_players.unwrap_or(0)
                    ));
                } else {
                    stats.set_message(format!("Success: {}", ip));
                }
            }
            
            result
        });
        
        handles.push(handle);
    }
    
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }
    
    main_pb.finish_with_message("Scan complete!");
    stats_pb.finish_and_clear();
    
    let total = results.len();
    let successful = results.iter().filter(|r| r.error.is_none()).count();
    let failed = total - successful;
    let online_mode = results.iter().filter(|r| r.auth_mode == Some(1)).count();
    let offline_mode = results.iter().filter(|r| r.auth_mode == Some(0)).count();
    let whitelist = results.iter().filter(|r| r.auth_mode == Some(2)).count();
    
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📈 Scan Results:");
    println!("   Total scanned:    {}", total);
    println!("   ✓ Successful:     {} ({:.1}%)", successful, (successful as f32 / total as f32) * 100.0);
    println!("   ✗ Failed:         {} ({:.1}%)", failed, (failed as f32 / total as f32) * 100.0);
    
    if check_auth {
        println!();
        println!("🔐 Authentication Modes:");
        println!("   🟢 Online:        {}", online_mode);
        println!("   🟡 Offline:       {}", offline_mode);
        println!("   🔴 Whitelist:     {}", whitelist);
    }
    
    let json = serde_json::to_string_pretty(&results)?;
    tokio::fs::write("results.json", json).await?;
    
    println!();
    println!("💾 Results saved to: results.json");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    Ok(())
}
