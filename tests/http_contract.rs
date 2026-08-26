use rtdb_typed::{TypedClient, TypedError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct User {
    name: String,
    score: u32,
}

async fn server(response: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let size = socket.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..size]);
        assert!(
            request.contains("auth=test-token"),
            "request was: {request}"
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    format!("http://{address}")
}

#[tokio::test]
async fn crud_boundary_decodes_typed_values_and_push_keys() {
    let base = server("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 27\r\n\r\n{\"name\":\"Alice\",\"score\":95}").await;
    let client = TypedClient::from_parts(base, "test-token");
    assert_eq!(
        client.get::<User>("users/alice").await.unwrap(),
        User {
            name: "Alice".into(),
            score: 95
        }
    );
}

#[tokio::test]
async fn null_is_optional_and_missing_push_key_is_an_error() {
    let base = server(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 4\r\n\r\nnull",
    )
    .await;
    let client = TypedClient::from_parts(base, "test-token");
    assert_eq!(client.get_optional::<User>("missing").await.unwrap(), None);

    let base =
        server("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\n\r\n{}")
            .await;
    let error = client_from(base)
        .post("users", &json!({"name":"Alice"}))
        .await
        .unwrap_err();
    assert!(matches!(error, TypedError::MissingPushKey));
}

fn client_from(base: String) -> TypedClient {
    TypedClient::from_parts(base, "test-token")
}
