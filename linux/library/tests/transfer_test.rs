use std::fs::{self, File};
use std::io::Write;
use std::sync::mpsc;
use std::time::Duration;
use swish_core::*;

#[test]
fn test_wire_frame_roundtrip() {
    let payload = b"Hello Swish Wire!".to_vec();
    let frame = WireFrame::new(MessageType::DataChunk, payload.clone());

    let mut buf = Vec::new();
    frame.write_to(&mut buf).unwrap();

    let mut cursor = std::io::Cursor::new(buf);
    let decoded = WireFrame::read_from(&mut cursor).unwrap();

    assert_eq!(decoded.msg_type, MessageType::DataChunk);
    assert_eq!(decoded.payload, payload);
}

#[test]
fn test_unknown_payload_type_is_rejected() {
    let metadata = PayloadMetadata {
        id: "transfer-1".to_string(),
        payload_type: PayloadType::File,
        name: "sample.txt".to_string(),
        mime_type: "text/plain".to_string(),
        size_bytes: 0,
    };
    let mut encoded = swish_core::messages::encode_transfer_request(&metadata);
    encoded[2 + metadata.id.len()] = 99;

    let error = swish_core::messages::decode_transfer_request(&encoded).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn test_empty_handshake_device_name_is_rejected() {
    let request = swish_core::messages::HandshakeRequest {
        ephemeral_id: [0; 8],
        device_name: "   ".to_string(),
    };
    let encoded = swish_core::messages::encode_handshake_request(&request);

    let error = swish_core::messages::decode_handshake_request(&encoded).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn test_client_server_transfer() {
    let temp_dir = std::env::temp_dir().join(format!("swish_test_{}", rand_id()));
    let storage_dir = temp_dir.join("storage");
    let source_dir = temp_dir.join("source");
    fs::create_dir_all(&storage_dir).unwrap();
    fs::create_dir_all(&source_dir).unwrap();

    // Create test file
    let file_path = source_dir.join("sample.txt");
    let test_data = b"Swish Linux to Linux / Android test content 12345";
    {
        let mut f = File::create(&file_path).unwrap();
        f.write_all(test_data).unwrap();
    }

    let (tx, rx) = mpsc::channel();

    let server = TransferServer::start(
        "127.0.0.1:0",
        storage_dir.clone(),
        [1, 2, 3, 4, 5, 6, 7, 8],
        "Linux Receiver".to_string(),
        |_meta| true,
        move |transfer| {
            tx.send(transfer).unwrap();
        },
    )
    .unwrap();

    let port = server.local_port().unwrap();
    let client = TransferClient::new([8, 7, 6, 5, 4, 3, 2, 1], "Linux Sender".to_string());

    client
        .send_file(
            format!("127.0.0.1:{}", port),
            &file_path,
            "text/plain",
            |_sent, _total| {},
        )
        .unwrap();

    let received = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(received.metadata.name, "sample.txt");
    let content = fs::read(&received.file_path).unwrap();
    assert_eq!(content, test_data);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_receiver_rejection() {
    let temp_dir = std::env::temp_dir().join(format!("swish_reject_{}", rand_id()));
    let storage_dir = temp_dir.join("storage");
    let source_dir = temp_dir.join("source");
    fs::create_dir_all(&storage_dir).unwrap();
    fs::create_dir_all(&source_dir).unwrap();

    let file_path = source_dir.join("reject_me.txt");
    File::create(&file_path)
        .unwrap()
        .write_all(b"reject me")
        .unwrap();

    let server = TransferServer::start(
        "127.0.0.1:0",
        storage_dir.clone(),
        [1, 2, 3, 4, 5, 6, 7, 8],
        "Strict Receiver".to_string(),
        |_meta| false, // Reject all
        |_transfer| {},
    )
    .unwrap();

    let port = server.local_port().unwrap();
    let client = TransferClient::new([8, 7, 6, 5, 4, 3, 2, 1], "Linux Sender".to_string());

    let result = client.send_file(
        format!("127.0.0.1:{}", port),
        &file_path,
        "text/plain",
        |_s, _t| {},
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

fn rand_id() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
}
