use crate::messages::*;
use crate::wire::*;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

pub struct TransferClient {
    ephemeral_id: [u8; 8],
    device_name: String,
    timeout: Duration,
    chunk_size: usize,
}

impl TransferClient {
    pub fn new(ephemeral_id: [u8; 8], device_name: String) -> Self {
        Self {
            ephemeral_id,
            device_name,
            timeout: Duration::from_secs(15),
            chunk_size: 64 * 1024,
        }
    }

    pub fn send_file<A: ToSocketAddrs, P: AsRef<Path>, F>(
        &self,
        addr: A,
        file_path: P,
        mime_type: &str,
        mut on_progress: F,
    ) -> io::Result<()>
    where
        F: FnMut(u64, u64),
    {
        let path = file_path.as_ref();
        let file = File::open(path)?;
        let file_len = file.metadata()?.len();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file.bin")
            .to_string();

        let mut stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;

        let req = HandshakeRequest {
            ephemeral_id: self.ephemeral_id,
            device_name: self.device_name.clone(),
        };
        WireFrame::new(
            MessageType::HandshakeRequest,
            encode_handshake_request(&req),
        )
        .write_to(&mut stream)?;

        let frame = WireFrame::read_from(&mut stream)?;
        if frame.msg_type != MessageType::HandshakeResponse {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected HandshakeResponse, got {:?}", frame.msg_type),
            ));
        }
        let resp = decode_handshake_response(&frame.payload)?;
        if !resp.accepted {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "Receiver rejected connection handshake",
            ));
        }
        if resp.device_name.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Receiver returned an empty device name",
            ));
        }

        let metadata = PayloadMetadata {
            id: Uuid::new_v4().simple().to_string(),
            payload_type: PayloadType::File,
            name: file_name,
            mime_type: mime_type.to_string(),
            size_bytes: file_len,
        };
        WireFrame::new(
            MessageType::TransferRequest,
            encode_transfer_request(&metadata),
        )
        .write_to(&mut stream)?;

        let frame = WireFrame::read_from(&mut stream)?;
        match frame.msg_type {
            MessageType::TransferAccept => {}
            MessageType::TransferReject => {
                let reject = decode_transfer_reject(&frame.payload)?;
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("Transfer rejected: {}", reject.reason),
                ));
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Expected TransferAccept or TransferReject, got {:?}", other),
                ));
            }
        }

        let mut reader = BufReader::new(file);
        let mut buffer = vec![0u8; self.chunk_size];
        let mut hasher = Sha256::new();
        let mut bytes_sent = 0u64;

        while bytes_sent < file_len {
            let bytes_to_read = std::cmp::min(buffer.len() as u64, file_len - bytes_sent) as usize;
            reader.read_exact(&mut buffer[..bytes_to_read])?;

            hasher.update(&buffer[..bytes_to_read]);
            WireFrame::new(MessageType::DataChunk, buffer[..bytes_to_read].to_vec())
                .write_to(&mut stream)?;

            bytes_sent += bytes_to_read as u64;
            on_progress(bytes_sent, file_len);
        }

        let sha256_hex = format!("{:x}", hasher.finalize());
        let complete = TransferComplete {
            transfer_id: metadata.id,
            sha256_hex,
        };
        WireFrame::new(
            MessageType::TransferComplete,
            encode_transfer_complete(&complete),
        )
        .write_to(&mut stream)?;

        Ok(())
    }
}
