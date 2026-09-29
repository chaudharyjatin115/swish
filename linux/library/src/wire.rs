use std::io::{self, Read, Write};

pub const MAGIC: &[u8; 4] = b"SWSH";
pub const MAX_FRAME_PAYLOAD_SIZE: usize = 16 * 1024 * 1024; // 16 MB

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    HandshakeRequest = 0x01,
    HandshakeResponse = 0x02,
    TransferRequest = 0x10,
    TransferAccept = 0x11,
    TransferReject = 0x12,
    DataChunk = 0x20,
    TransferComplete = 0x21,
    Cancel = 0x30,
}

impl MessageType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x01 => Some(MessageType::HandshakeRequest),
            0x02 => Some(MessageType::HandshakeResponse),
            0x10 => Some(MessageType::TransferRequest),
            0x11 => Some(MessageType::TransferAccept),
            0x12 => Some(MessageType::TransferReject),
            0x20 => Some(MessageType::DataChunk),
            0x21 => Some(MessageType::TransferComplete),
            0x30 => Some(MessageType::Cancel),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireFrame {
    pub msg_type: MessageType,
    pub payload: Vec<u8>,
}

impl WireFrame {
    pub fn new(msg_type: MessageType, payload: Vec<u8>) -> Self {
        Self { msg_type, payload }
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(MAGIC)?;
        writer.write_all(&[self.msg_type as u8])?;
        let len = self.payload.len() as u32;
        writer.write_all(&len.to_be_bytes())?;
        if !self.payload.is_empty() {
            writer.write_all(&self.payload)?;
        }
        writer.flush()?;
        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid magic header in Swish frame",
            ));
        }

        let mut type_buf = [0u8; 1];
        reader.read_exact(&mut type_buf)?;
        let msg_type = MessageType::from_u8(type_buf[0]).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown message type: 0x{:02x}", type_buf[0]),
            )
        })?;

        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf)?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len > MAX_FRAME_PAYLOAD_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Frame payload exceeds maximum allowed size: {}", len),
            ));
        }

        let mut payload = vec![0u8; len];
        if len > 0 {
            reader.read_exact(&mut payload)?;
        }

        Ok(WireFrame { msg_type, payload })
    }
}
