use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, BufReader};
use flate2::read::ZlibDecoder;
use tracing::{info, error, warn};
use anyhow::{Result, Context};
use std::path::Path;
use super::map_instance::MapInstance;

#[repr(C, packed)]
struct MapCacheMainHeader {
    file_size: u32,
    map_count: u16,
}

#[repr(C, packed)]
struct MapCacheMapInfo {
    name: [u8; 12],
    xs: i16,
    ys: i16,
    len: i32,
}

pub fn load_map_cache<P: AsRef<Path>>(path: P) -> Result<HashMap<String, MapInstance>> {
    let file = File::open(&path).context("Failed to open map_cache.dat")?;
    let mut reader = BufReader::new(file);

    // C++ struct map_cache_main_header is 8 bytes due to alignment padding (u32 + u16 + 2 bytes padding)
    let mut header_buf = [0u8; 8];
    reader.read_exact(&mut header_buf)?;

    let file_size = u32::from_le_bytes(header_buf[0..4].try_into().unwrap());
    let map_count = u16::from_le_bytes(header_buf[4..6].try_into().unwrap());

    info!("Found map_cache.dat (Size: {}, Maps: {})", file_size, map_count);

    let mut maps = HashMap::new();

    for i in 0..map_count {
        let mut info_buf = [0u8; 20];
        if reader.read_exact(&mut info_buf).is_err() {
            warn!("Failed to read map info header for map index {}", i);
            break;
        }

        let mut name_bytes = [0u8; 12];
        name_bytes.copy_from_slice(&info_buf[0..12]);
        let xs = i16::from_le_bytes(info_buf[12..14].try_into().unwrap());
        let ys = i16::from_le_bytes(info_buf[14..16].try_into().unwrap());
        let len = i32::from_le_bytes(info_buf[16..20].try_into().unwrap());

        // Read null-terminated name
        let name_end = name_bytes.iter().position(|&c| c == 0).unwrap_or(12);
        let name = String::from_utf8_lossy(&name_bytes[0..name_end]).into_owned();

        // Read compressed data
        let mut compressed_data = vec![0u8; len as usize];
        if reader.read_exact(&mut compressed_data).is_err() {
            error!("Failed to read compressed data for map {}", name);
            break;
        }

        // Decompress zlib
        let mut decoder = ZlibDecoder::new(&compressed_data[..]);
        let mut decompressed_data = Vec::with_capacity((xs as usize) * (ys as usize));
        
        if decoder.read_to_end(&mut decompressed_data).is_err() {
            error!("Failed to decompress map data for {}", name);
            continue;
        }
        
        let expected_size = (xs as usize) * (ys as usize);
        if decompressed_data.len() != expected_size {
            warn!("Map {} size mismatch: expected {}, got {}", name, expected_size, decompressed_data.len());
        }

        let map_instance = MapInstance::new(name.clone(), xs, ys, decompressed_data);
        maps.insert(name.clone(), map_instance);
    }

    info!("Successfully loaded {} maps from cache.", maps.len());
    Ok(maps)
}
