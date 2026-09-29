use std::io::{self, Cursor, Read, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PayloadType {
    Text = 1,
    Url = 2,
    Image = 3,
    File = 4,
}

impl PayloadType {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => PayloadType::Text,
            2 => PayloadType::Url,
            3 => PayloadType::Image,
            _ => PayloadType::File,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadMetadata {
    pub id: String,
    pub payload_type: PayloadType,
    pub name: String,
    pub mime_type: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeRequest {
    pub ephemeral_id: [u8; 8],
    pub device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeResponse {
    pub accepted: bool,
    pub ephemeral_id: [u8; 8],
    pub device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferReject {
    pub transfer_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferComplete {
    pub transfer_id: String,
    pub sha256_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cancel {
    pub transfer_id: String,
    pub reason: String,
}

fn write_utf<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    let len = bytes.len() as u16;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(bytes)?;
    Ok(())
}

fn read_utf<R: Read>(r: &mut R) -> io::Result<String> {
    let mut len_buf = [0u8; 2];
    r.read_exact(&mut len_buf)?;
    let len = u16::from_be_bytes(len_buf) as usize;
    let mut str_bytes = vec![0u8; len];
    r.read_exact(&mut str_bytes)?;
    String::from_utf8(str_bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

pub fn sanitize_filename(raw_name: &str) -> String {
    let path = Path::new(raw_name);
    let base = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("payload.bin");

    let sanitized: String = base
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.trim().is_empty() {
        "payload.bin".to_string()
    } else {
        sanitized
    }
}

pub fn encode_handshake_request(req: &HandshakeRequest) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&req.ephemeral_id);
    let _ = write_utf(&mut buf, &req.device_name);
    buf
}

pub fn decode_handshake_request(bytes: &[u8]) -> io::Result<HandshakeRequest> {
    let mut cursor = Cursor::new(bytes);
    let mut id = [0u8; 8];
    cursor.read_exact(&mut id)?;
    let device_name = read_utf(&mut cursor)?;
    if device_name.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Device name cannot be empty",
        ));
    }
    Ok(HandshakeRequest {
        ephemeral_id: id,
        device_name,
    })
}

pub fn encode_handshake_response(resp: &HandshakeResponse) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(if resp.accepted { 1 } else { 0 });
    buf.extend_from_slice(&resp.ephemeral_id);
    let _ = write_utf(&mut buf, &resp.device_name);
    buf
}

pub fn decode_handshake_response(bytes: &[u8]) -> io::Result<HandshakeResponse> {
    let mut cursor = Cursor::new(bytes);
    let mut accepted_byte = [0u8; 1];
    cursor.read_exact(&mut accepted_byte)?;
    let mut id = [0u8; 8];
    cursor.read_exact(&mut id)?;
    let device_name = read_utf(&mut cursor)?;
    Ok(HandshakeResponse {
        accepted: accepted_byte[0] != 0,
        ephemeral_id: id,
        device_name,
    })
}

pub fn encode_transfer_request(meta: &PayloadMetadata) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = write_utf(&mut buf, &meta.id);
    buf.push(meta.payload_type as u8);
    let _ = write_utf(&mut buf, &sanitize_filename(&meta.name));
    let _ = write_utf(&mut buf, &meta.mime_type);
    buf.extend_from_slice(&meta.size_bytes.to_be_bytes());
    buf
}

pub fn decode_transfer_request(bytes: &[u8]) -> io::Result<PayloadMetadata> {
    let mut cursor = Cursor::new(bytes);
    let id = read_utf(&mut cursor)?;
    let mut type_buf = [0u8; 1];
    cursor.read_exact(&mut type_buf)?;
    if !(1..=4).contains(&type_buf[0]) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unknown payload type: {}", type_buf[0]),
        ));
    }
    let payload_type = PayloadType::from_u8(type_buf[0]);
    let name = sanitize_filename(&read_utf(&mut cursor)?);
    let mime_type = read_utf(&mut cursor)?;
    let mut size_buf = [0u8; 8];
    cursor.read_exact(&mut size_buf)?;
    let size_bytes = u64::from_be_bytes(size_buf);

    Ok(PayloadMetadata {
        id,
        payload_type,
        name,
        mime_type,
        size_bytes,
    })
}

pub fn encode_transfer_accept(transfer_id: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = write_utf(&mut buf, transfer_id);
    buf
}

pub fn decode_transfer_accept(bytes: &[u8]) -> io::Result<String> {
    let mut cursor = Cursor::new(bytes);
    read_utf(&mut cursor)
}

pub fn encode_transfer_reject(reject: &TransferReject) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = write_utf(&mut buf, &reject.transfer_id);
    let _ = write_utf(&mut buf, &reject.reason);
    buf
}

pub fn decode_transfer_reject(bytes: &[u8]) -> io::Result<TransferReject> {
    let mut cursor = Cursor::new(bytes);
    let transfer_id = read_utf(&mut cursor)?;
    let reason = read_utf(&mut cursor)?;
    Ok(TransferReject {
        transfer_id,
        reason,
    })
}

pub fn encode_transfer_complete(complete: &TransferComplete) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = write_utf(&mut buf, &complete.transfer_id);
    let _ = write_utf(&mut buf, &complete.sha256_hex);
    buf
}

pub fn decode_transfer_complete(bytes: &[u8]) -> io::Result<TransferComplete> {
    let mut cursor = Cursor::new(bytes);
    let transfer_id = read_utf(&mut cursor)?;
    let sha256_hex = read_utf(&mut cursor)?;
    Ok(TransferComplete {
        transfer_id,
        sha256_hex,
    })
}

pub fn encode_cancel(cancel: &Cancel) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = write_utf(&mut buf, &cancel.transfer_id);
    let _ = write_utf(&mut buf, &cancel.reason);
    buf
}

pub fn decode_cancel(bytes: &[u8]) -> io::Result<Cancel> {
    let mut cursor = Cursor::new(bytes);
    let transfer_id = read_utf(&mut cursor)?;
    let reason = read_utf(&mut cursor)?;
    Ok(Cancel {
        transfer_id,
        reason,
    })
}
