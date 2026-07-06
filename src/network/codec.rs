use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};
use std::io;

/// Basic representation of an RO Packet
#[derive(Debug, Clone)]
pub struct RoPacket {
    pub cmd: u16,
    pub payload: Vec<u8>,
}

pub struct RoPacketCodec;

impl Decoder for RoPacketCodec {
    type Item = RoPacket;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 2 {
            // Not enough data to read command ID
            return Ok(None);
        }

        // Peek at the command ID (little endian)
        let cmd = src[0] as u16 | ((src[1] as u16) << 8);

        let packet_len = get_packet_length(cmd);
        
        if packet_len == -1 {
            // Dynamic length packet - next 2 bytes are usually length
            if src.len() < 4 {
                return Ok(None);
            }
            let dyn_len = (src[2] as u16 | ((src[3] as u16) << 8)) as usize;
            if src.len() < dyn_len {
                return Ok(None);
            }
            let data = src.split_to(dyn_len);
            return Ok(Some(RoPacket {
                cmd,
                payload: data[4..].to_vec(),
            }));
        } else {
            let fixed_len = packet_len as usize;
            if src.len() < fixed_len {
                return Ok(None);
            }
            let data = src.split_to(fixed_len);
            return Ok(Some(RoPacket {
                cmd,
                payload: data[2..].to_vec(),
            }));
        }
    }
}

impl Encoder<RoPacket> for RoPacketCodec {
    type Error = io::Error;

    fn encode(&mut self, item: RoPacket, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.put_u16_le(item.cmd);
        dst.put_slice(&item.payload);
        Ok(())
    }
}

// Dummy length lookup function (to be replaced with actual DB)
fn get_packet_length(cmd: u16) -> i32 {
    match cmd {
        0x0436 => 23, // Map Login Packet (newer clients)
        0x021d => 6,  // Ping / Time sync?
        0x0360 => 6,  // CZ_REQUEST_ACT (newer clients)
        0x035f => 5,  // CZ_REQUEST_MOVE (newer clients)
        // 0x0085 => 5, // Old clients used this for MOVE, but newer clients use it for CHAT!
        0x0089 => 7,  // CZ_REQUEST_ACT (old clients)
        0x0368 => 6,  // GetCharNameRequest / SolveCharName
        0x08c9 => 2,  // CashShop List
        0x0064 => 55, // e.g. login packet
        0x0065 => -1, // dynamic length example
        0x0085 => -1, // CZ_GLOBAL_CHAT (newer clients)
        0x00b2 => 3,  // CZ_RESTART (back to character select)
        0x0437 => 7,  // CZ_REQUEST_ACT2 (newer clients)
        0x008c => -1, // CZ_REQUEST_CHAT (dynamic length)
        0x00f3 => -1, // CZ_GLOBAL_CHAT (dynamic length)
        0x009f => -1, // CZ_GLOBAL_CHAT
        0x0096 => -1, // CZ_WHISPER (dynamic length)
        _ => 2,       // Default to just the header for unknown
    }
}
