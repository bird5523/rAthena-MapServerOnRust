use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, Duration};
use tracing::{info, error, warn};
use std::sync::Arc;
use crate::core::state::ServerState;

pub async fn connect_to_char_server(state: Arc<ServerState>) {
    let char_server_ip = &state.config.char_ip;
    let char_server_port = state.config.char_port;
    
    loop {
        info!("Attempting to connect to Char Server at {}:{}...", char_server_ip, char_server_port);
        
        match TcpStream::connect(format!("{}:{}", char_server_ip, char_server_port)).await {
            Ok(mut stream) => {
                info!("Successfully connected to Char Server!");
                
                // Construct the Map-to-Char registration packet (0x2af8)
                // Format: <cmd: u16> <userid: char[24]> <pass: char[24]> <ip: u32> <port: u16> <mac_addr: u8[18]>
                // Total length: 60 bytes (as per chrif.cpp)
                let mut dummy_packet = vec![0; 60];
                dummy_packet[0] = 0xf8;
                dummy_packet[1] = 0x2a; // 0x2af8
                
                // Set userid from config
                let userid_bytes = state.config.userid.as_bytes();
                for (i, &b) in userid_bytes.iter().take(24).enumerate() {
                    dummy_packet[2 + i] = b;
                }
                
                // Set password from config
                let passwd_bytes = state.config.passwd.as_bytes();
                for (i, &b) in passwd_bytes.iter().take(24).enumerate() {
                    dummy_packet[26 + i] = b;
                }
                
                // Bytes 50-53 are usually 0
                
                // IP Address (bytes 54-57, Big Endian) - Setting to 127.0.0.1
                dummy_packet[54] = 127;
                dummy_packet[55] = 0;
                dummy_packet[56] = 0;
                dummy_packet[57] = 1;
                
                // Port (bytes 58-59, Big Endian)
                let port = state.config.map_port;
                dummy_packet[58] = (port >> 8) as u8; // MSB
                dummy_packet[59] = (port & 0xFF) as u8; // LSB

                if let Err(e) = stream.write_all(&dummy_packet).await {
                    error!("Failed to send registration packet to Char Server: {}", e);
                } else {
                    info!("Sent registration packet (0x2af8) to Char Server.");
                }

                // Listen for Char server responses
                let mut buf = [0; 2048];
                loop {
                    match stream.read(&mut buf).await {
                        Ok(0) => {
                            warn!("Char Server closed the connection.");
                            break;
                        }
                        Ok(n) => {
                            if n >= 2 {
                                let cmd = buf[0] as u16 | ((buf[1] as u16) << 8);
                                info!("Received response from Char Server. Command ID: {:#06x}", cmd);
                                
                                if cmd == 0x2af9 {
                                    info!("Char Server accepted map server registration! Sending maps...");
                                    
                                    // 3. Prepare Map List Packet (0x2afa)
                                    let map_names = vec![
                                        "prontera", "morocc", "geffen", "payon", "alberta", "izlude", "aldebaran", "xmas",
                                        "comodo", "yuno", "amatsu", "gonryun", "umbala", "niao", "louyang", "ayothaya",
                                        "einbroch", "lighthalzen", "einbech", "hugel", "rachel", "veins", "moscovia",
                                        "mid_camp", "manuk", "splendide", "dicastes01", "mora", "dewata", "malangdo",
                                        "malaya", "eclage", "new_1-1", "new_1-2", "new_1-3", "new_1-4", "new_2-1", "new_2-2",
                                        "new_3-1", "new_3-2", "new_4-1", "new_4-2", "new_5-1", "new_5-2",
                                        "iz_int", "iz_int01", "iz_int02", "iz_int03", "iz_int04", "sec_pri", "sec_in01", "sec_in02"
                                    ];
                                    let mut map_packet = vec![0; 4 + map_names.len() * 16];
                                    map_packet[0] = 0xfa;
                                    map_packet[1] = 0x2a; // 0x2afa
                                    let len = (4 + map_names.len() * 16) as u16;
                                    map_packet[2] = (len & 0xFF) as u8;
                                    map_packet[3] = (len >> 8) as u8;

                                    for (i, name) in map_names.iter().enumerate() {
                                        let bytes = name.as_bytes();
                                        let start = 4 + (i * 16);
                                        let end = start + bytes.len().min(16);
                                        map_packet[start..end].copy_from_slice(&bytes[..bytes.len().min(16)]);
                                    }
                                    
                                    let _ = stream.write_all(&map_packet).await;
                                } else if cmd == 0x2b01 {
                                    // Char Server wants to send a player to this Map Server!
                                    // Format: <cmd: u16> <account_id: u32> <char_id: u32> ...
                                    if n >= 10 {
                                        let account_id = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]);
                                        let char_id = u32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]);
                                        info!("Char Server is sending Account {} (Char {}) to our Map Server!", account_id, char_id);
                                        
                                        // Acknowledge with 0x2b02 (Auth Ok)
                                        // Format: <cmd: u16> <account_id: u32> <char_id: u32> <status: u32> (0 = success)
                                        let mut ack = vec![0; 14];
                                        ack[0] = 0x02;
                                        ack[1] = 0x2b; // 0x2b02
                                        ack[2..6].copy_from_slice(&account_id.to_le_bytes());
                                        ack[6..10].copy_from_slice(&char_id.to_le_bytes());
                                        ack[10..14].copy_from_slice(&0u32.to_le_bytes()); // Status 0 (Success)
                                        
                                        if let Err(e) = stream.write_all(&ack).await {
                                            error!("Failed to send AuthAck (0x2b02) to Char Server: {}", e);
                                        } else {
                                            info!("Sent AuthAck (0x2b02) for Account {}. Waiting for client to connect to us...", account_id);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error reading from Char Server: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to connect to Char Server: {}. Retrying in 5 seconds...", e);
            }
        }
        
        // Wait before reconnecting
        sleep(Duration::from_secs(5)).await;
    }
}
