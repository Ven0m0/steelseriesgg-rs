use steelseries_gg::gamesense::GameSenseServer;
use tokio::net::TcpListener;

#[tokio::test]
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
    let client = reqwest::Client::new();
    loop {
        if let Ok(_) = client.get(format!("http://127.0.0.1:{}/", port)).send().await {
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
    let response = client
        .post(format!("http://127.0.0.1:{}/game_metadata", port))
        .header("Origin", "http://evil.com")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();

    println!("Response from evil.com origin: {:?}", response);

    let allows_evil = response.headers().get("access-control-allow-origin").is_some();

    if allows_evil {
        panic!("SECURITY FAILURE: Server allowed CORS request from http://evil.com");
    }

    // 2. Test with Localhost Origin
    println!("Sending localhost request...");
    let response_local = client
        .post(format!("http://127.0.0.1:{}/game_metadata", port))
        .header("Origin", format!("http://localhost:{}", port))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();

    println!("Response from localhost origin: {:?}", response_local);

    let allows_local = response_local.headers().get("access-control-allow-origin").is_some();
    // Assert strictly
    assert!(allows_local, "Server should allow localhost origin");
}
