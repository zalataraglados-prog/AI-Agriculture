use std::net::UdpSocket;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

fn find_free_udp_port() -> u16 {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind ephemeral port");
    socket.local_addr().expect("read local addr").port()
}

#[test]
fn cloud_help_flag_exits_success() {
    let output = Command::new(env!("CARGO_BIN_EXE_cloud"))
        .arg("--help")
        .output()
        .expect("execute cloud --help");

    assert!(output.status.success(), "help command should succeed");
}

#[test]
fn cloud_receiver_ack_success_packet() {
    let port = find_free_udp_port();
    let bind_addr = format!("127.0.0.1:{port}");

    let args = vec![
        "--config".to_string(),
        "config/sensors.toml".to_string(),
        "--bind".to_string(),
        bind_addr.clone(),
        //"--once".to_string(),
        "--timeout-ms".to_string(),
        "0".to_string(),
    ];

    let child = Command::new(env!("CARGO_BIN_EXE_cloud"))
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start cloud receiver");

    thread::sleep(Duration::from_millis(200));

    let sender = UdpSocket::bind("127.0.0.1:0").expect("bind sender socket");
    sender
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set sender read timeout");

    sender
        .send_to(b"success", &bind_addr)
        .expect("send success payload");

    let mut buf = [0_u8; 128];
    let (size, _) = sender.recv_from(&mut buf).expect("receive ack");
    assert_eq!(&buf[..size], b"ack:success");

    let output = child.wait_with_output().expect("wait cloud process");
    assert!(
        output.status.success(),
        "cloud process should exit success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}