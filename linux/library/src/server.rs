use crate::messages::*;
use crate::wire::*;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub struct ReceivedTransfer {
    pub file_path: PathBuf,
    pub metadata: PayloadMetadata,
}

pub struct TransferServer {
    listener: TcpListener,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TransferServer {
    pub fn start<F, C>(
        bind_addr: &str,
        storage_dir: PathBuf,
        ephemeral_id: [u8; 8],
        device_name: String,
        should_accept: F,
        on_complete: C,
    ) -> io::Result<Self>
    where
        F: Fn(&PayloadMetadata) -> bool + Send + Sync + 'static,
        C: Fn(ReceivedTransfer) + Send + Sync + 'static,
    {
        fs::create_dir_all(&storage_dir)?;
        let listener = TcpListener::bind(bind_addr)?;
        let listener_clone = listener.try_clone()?;
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        let handle = thread::spawn(move || {
            let should_accept = Arc::new(should_accept);
            let on_complete = Arc::new(on_complete);

            while running_clone.load(Ordering::Relaxed) {
                match listener_clone.accept() {
                    Ok((mut stream, _addr)) => {
                        let storage_dir = storage_dir.clone();
                        let device_name = device_name.clone();
                        let should_accept = Arc::clone(&should_accept);
                        let on_complete = Arc::clone(&on_complete);

                        thread::spawn(move || {
                            if let Err(e) = Self::handle_connection(
                                &mut stream,
                                &storage_dir,
                                ephemeral_id,
                                &device_name,
                                &*should_accept,
                                &*on_complete,
                            ) {
                                eprintln!("swish: connection failed: {}", e);
                            }
                        });
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            listener,
            running,
            handle: Some(handle),
        })
    }

    pub fn local_port(&self) -> io::Result<u16> {
        self.listener.local_addr().map(|a| a.port())
    }

    fn handle_connection<F, C>(
        stream: &mut TcpStream,
        storage_dir: &Path,
        ephemeral_id: [u8; 8],
        device_name: &str,
        should_accept: &F,
        on_complete: &C,
    ) -> io::Result<()>
    where
        F: Fn(&PayloadMetadata) -> bool,
        C: Fn(ReceivedTransfer),
    {
        let frame = WireFrame::read_from(stream)?;
        if frame.msg_type != MessageType::HandshakeRequest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected HandshakeRequest, got {:?}", frame.msg_type),
            ));
        }
        let _req = decode_handshake_request(&frame.payload)?;

        let resp = HandshakeResponse {
            accepted: true,
            ephemeral_id,
            device_name: device_name.to_string(),
        };
        WireFrame::new(
            MessageType::HandshakeResponse,
            encode_handshake_response(&resp),
        )
        .write_to(stream)?;

        let frame = WireFrame::read_from(stream)?;
        if frame.msg_type != MessageType::TransferRequest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected TransferRequest, got {:?}", frame.msg_type),
            ));
        }
        let metadata = decode_transfer_request(&frame.payload)?;

        if !should_accept(&metadata) {
            let reject = TransferReject {
                transfer_id: metadata.id.clone(),
                reason: "Rejected by receiver policy".to_string(),
            };
            WireFrame::new(MessageType::TransferReject, encode_transfer_reject(&reject))
                .write_to(stream)?;
            return Ok(());
        }

        let accept = encode_transfer_accept(&metadata.id);
        WireFrame::new(MessageType::TransferAccept, accept).write_to(stream)?;

        let target_file_path = storage_dir.join(&metadata.name);
        let mut hasher = Sha256::new();
        let mut bytes_received = 0u64;

        // RAII cleanup guard: removes partial or corrupted downloads from disk if
        // the client disconnects, aborts, or fails the final SHA-256 checksum.
        // Defused only upon verified completion.
        struct CleanupGuard(Option<PathBuf>);
        impl Drop for CleanupGuard {
            fn drop(&mut self) {
                if let Some(ref path) = self.0 {
                    if path.exists() {
                        let _ = fs::remove_file(path);
                    }
                }
            }
        }
        impl CleanupGuard {
            fn defuse(&mut self) {
                self.0 = None;
            }
        }

        let mut guard = CleanupGuard(Some(target_file_path.clone()));

        {
            let file = File::create(&target_file_path)?;
            let mut writer = BufWriter::new(file);

            while bytes_received < metadata.size_bytes {
                let chunk_frame = WireFrame::read_from(stream)?;
                match chunk_frame.msg_type {
                    MessageType::DataChunk => {
                        let remaining = metadata.size_bytes - bytes_received;
                        if chunk_frame.payload.len() as u64 > remaining {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "Data chunk exceeds declared transfer size",
                            ));
                        }
                        writer.write_all(&chunk_frame.payload)?;
                        hasher.update(&chunk_frame.payload);
                        bytes_received += chunk_frame.payload.len() as u64;
                    }
                    MessageType::Cancel => {
                        return Err(io::Error::new(
                            io::ErrorKind::Interrupted,
                            "Transfer cancelled by sender",
                        ));
                    }
                    other => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Unexpected message type during transfer: {:?}", other),
                        ));
                    }
                }
            }
            writer.flush()?;
        }

        let complete_frame = WireFrame::read_from(stream)?;
        if complete_frame.msg_type != MessageType::TransferComplete {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Expected TransferComplete, got {:?}",
                    complete_frame.msg_type
                ),
            ));
        }

        let complete = decode_transfer_complete(&complete_frame.payload)?;
        if complete.transfer_id != metadata.id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Transfer completion ID does not match the request",
            ));
        }
        let computed_sha256 = format!("{:x}", hasher.finalize());

        if computed_sha256 != complete.sha256_hex {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "SHA-256 verification failed: expected {}, computed {}",
                    complete.sha256_hex, computed_sha256
                ),
            ));
        }

        guard.defuse();

        on_complete(ReceivedTransfer {
            file_path: target_file_path,
            metadata,
        });

        Ok(())
    }
}

impl Drop for TransferServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        // Connect a loopback socket to wake the acceptor thread from blocking in accept()
        if let Ok(addr) = self.listener.local_addr() {
            let _ = TcpStream::connect(addr);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
