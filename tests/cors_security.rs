use std::io::{Read, Write};
use std::net::TcpStream;
use steelseries_gg::gamesense::GameSenseServer;
use tokio::net::TcpListener;

#[tokio::test]
#[ignore]
async fn test_cors_vulnerability() {
    println!("Starting test_cors_vulnerability");
    // Bind to port 0 to get a free port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    println!("Selected port: {}", port);

    let server = GameSenseServer::new("127.0.0.1", port).unwrap();

    tokio::spawn(async move {
        println!("Server task started");
        if let Err(e) = server.run().await {
            eprintln!("Server error: {}", e);
        }
    });

    // Wait for server to start
    let mut attempts = 0;
    loop {
        if TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok() {
            println!("Connected to server!");
            break;
        }
        attempts += 1;
        if attempts > 50 {
            panic!("Server failed to start");
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // 1. Test with Evil Origin
    println!("Sending evil request...");
    let request = format!(
        "POST /game_metadata HTTP/1.1\r\n         Host: 127.0.0.1:{}\r\n         Origin: http://evil.com\r\n         Content-Type: application/json\r\n         Content-Length: 2\r\n         Connection: close\r\n         \r\n         {{}}",
        port
    );

    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream.write_all(request.as_bytes()).unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    println!("Response from evil.com origin:\n{}", response);

    let allows_evil = response
        .to_lowercase()
        .contains("access-control-allow-origin: http://evil.com")
        || response.to_lowercase().contains("access-control-allow-origin: *");

    if allows_evil {
        panic!("SECURITY FAILURE: Server allowed CORS request from http://evil.com");
    }

    // 2. Test with Localhost Origin
    println!("Sending localhost request...");
    let request_local = format!(
        "POST /game_metadata HTTP/1.1\r\n         Host: 127.0.0.1:{}\r\n         Origin: http://localhost:{}\r\n         Content-Type: application/json\r\n         Content-Length: 2\r\n         Connection: close\r\n         \r\n         {{}}",
        port, port
    );

    let mut stream_local = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream_local.write_all(request_local.as_bytes()).unwrap();

    let mut response_local = String::new();
    stream_local.read_to_string(&mut response_local).unwrap();

    println!("Response from localhost origin:\n{}", response_local);

    let allows_local = response_local.to_lowercase().contains("access-control-allow-origin");
    // Assert strictly
    assert!(allows_local, "Server should allow localhost origin");
}
