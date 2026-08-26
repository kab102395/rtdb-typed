use futures_util::StreamExt;
use rtdb_typed::{FirebaseCollection, PushResult, TypedClient, TypedError, TypedEvent};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct User {
    name: String,
    score: u32,
}

async fn server(response: &'static str) -> String {
    server_matching(response, "auth=test-token").await
}

async fn server_matching(response: &'static str, expected: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let size = socket.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..size]);
        assert!(request.contains(expected), "request was: {request}");
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    format!("http://{address}")
}

async fn chunked_sse_server(chunks: &'static [&'static str]) -> String {
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
        for chunk in chunks {
            socket.write_all(chunk.as_bytes()).await.unwrap();
            socket.flush().await.unwrap();
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
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

    let base = server(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 17\r\n\r\n{\"name\":\"-Npush\"}",
    )
    .await;
    let pushed: PushResult = client_from(base)
        .post("/users", &json!({"name":"Alice"}))
        .await
        .unwrap();
    assert_eq!(pushed.key, "-Npush");
    assert_eq!(pushed.path.as_deref(), Some("/users/-Npush"));
}

#[tokio::test]
async fn write_delete_and_upstream_status_boundaries_are_typed() {
    let response = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 27\r\n\r\n{\"name\":\"Alice\",\"score\":95}";
    let client = client_from(server(response).await);
    assert_eq!(
        client
            .put::<_, User>("users/alice", &json!({"name":"Alice","score":95}))
            .await
            .unwrap()
            .name,
        "Alice"
    );

    let client = client_from(server(response).await);
    assert_eq!(
        client
            .patch::<_, User>("users/alice", &json!({"score":100}))
            .await
            .unwrap()
            .score,
        95
    );

    let client = client_from(
        server(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 4\r\n\r\nnull",
        )
        .await,
    );
    client.delete("users/alice").await.unwrap();

    let client = client_from(
        server("HTTP/1.1 500 Internal Server Error\r\ncontent-type: application/json\r\ncontent-length: 5\r\n\r\nerror")
            .await,
    );
    assert!(matches!(
        client.get::<User>("users/alice").await,
        Err(TypedError::Rtdb(_))
    ));
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

#[tokio::test]
async fn query_filters_are_sent_and_decoded_into_a_collection() {
    let base = server_matching(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 37\r\n\r\n{\"alice\":{\"name\":\"Alice\",\"score\":95}}",
        "orderBy=%22active%22&limitToFirst=2&equalTo=true",
    )
    .await;
    let client = client_from(base);
    let results: FirebaseCollection<User> = client
        .query("users")
        .order_by_child("active")
        .equal_to(rtdb_typed::rtdb_rs::FilterValue::boolean(true))
        .limit_to_first(2)
        .send_collection()
        .await
        .unwrap();
    assert_eq!(results.get("alice").unwrap().score, 95);
}

#[tokio::test]
async fn query_range_and_key_ordering_are_supported() {
    let base = server_matching(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\n\r\n{}",
        "orderBy=%22%24key%22&startAt=10&endAt=20",
    )
    .await;
    let client = client_from(base);
    let results: FirebaseCollection<User> = client
        .query("users")
        .order_by_key()
        .start_at(rtdb_typed::rtdb_rs::FilterValue::number(10.0))
        .end_at(rtdb_typed::rtdb_rs::FilterValue::number(20.0))
        .send_collection()
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn invalid_and_malformed_queries_stay_in_the_typed_error_boundary() {
    let client = TypedClient::from_parts("http://127.0.0.1:1", "test-token");
    let error = client
        .query::<User>("users")
        .limit_to_first(2)
        .send()
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        TypedError::Rtdb(rtdb_typed::rtdb_rs::RtdbError::InvalidQuery(_))
    ));

    let base = server(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 15\r\n\r\n{\"score\":\"bad\"}",
    )
    .await;
    let error = client_from(base)
        .query::<User>("users/alice")
        .send()
        .await
        .unwrap_err();
    assert!(matches!(error, TypedError::Serde(_)));
}

#[tokio::test]
async fn collection_null_is_empty_but_optional_collection_preserves_missing() {
    let base = server(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 4\r\n\r\nnull",
    )
    .await;
    let client = client_from(base);
    let empty: FirebaseCollection<User> = client.get_collection("missing").await.unwrap();
    assert!(empty.is_empty());

    let base = server(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 4\r\n\r\nnull",
    )
    .await;
    let optional: Option<FirebaseCollection<User>> = client_from(base)
        .get_optional_collection("missing")
        .await
        .unwrap();
    assert_eq!(optional, None);

    let base = server(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 25\r\n\r\n{\"alice\":{\"score\":\"bad\"}}",
    )
    .await;
    assert!(matches!(
        client_from(base).get_collection::<User>("broken").await,
        Err(TypedError::Serde(_))
    ));
}

#[tokio::test]
async fn query_optional_and_collection_null_semantics_are_explicit() {
    let base = server(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 4\r\n\r\nnull",
    )
    .await;
    let client = client_from(base);
    assert_eq!(
        client
            .query::<User>("missing")
            .send_optional()
            .await
            .unwrap(),
        None
    );

    let base = server(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 4\r\n\r\nnull",
    )
    .await;
    let collection = client_from(base)
        .query::<User>("missing")
        .send_collection()
        .await
        .unwrap();
    assert!(collection.is_empty());
}

#[tokio::test]
async fn shallow_queries_and_filter_requirements_are_propagated() {
    let client = TypedClient::from_parts("http://127.0.0.1:1", "test-token");
    let error = client
        .query::<std::collections::HashMap<String, User>>("users")
        .shallow()
        .limit_to_first(1)
        .send()
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        TypedError::Rtdb(rtdb_typed::rtdb_rs::RtdbError::InvalidQuery(_))
    ));
}

#[tokio::test]
async fn sse_put_patch_keepalive_and_cancel_are_projected_correctly() {
    let base = server(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\nevent: put\ndata: {\"path\":\"/\",\"data\":{\"name\":\"Alice\",\"score\":95}}\n\nevent: patch\ndata: {\"path\":\"/\",\"data\":{\"score\":100}}\n\nevent: keep-alive\n\n\nevent: cancel\n\n",
    )
    .await;
    let client = client_from(base);
    let stream = client.query::<User>("users/alice").stream().await.unwrap();
    tokio::pin!(stream);

    let put = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(
        put,
        TypedEvent::Put {
            data: Some(User { score: 95, .. }),
            ..
        }
    ));
    let patch = stream.next().await.unwrap().unwrap();
    assert!(
        matches!(patch, TypedEvent::Patch { patch, .. } if patch.as_value() == &json!({"score": 100}))
    );
    assert!(matches!(
        stream.next().await.unwrap().unwrap(),
        TypedEvent::KeepAlive
    ));
    assert!(matches!(
        stream.next().await.unwrap().unwrap(),
        TypedEvent::Cancel
    ));
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn sse_malformed_payload_and_typed_put_failure_are_errors() {
    let base = server(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\nevent: put\ndata: {not-json}\n\n",
    )
    .await;
    let stream = client_from(base)
        .query::<User>("users/alice")
        .stream()
        .await
        .unwrap();
    tokio::pin!(stream);
    assert!(matches!(
        stream.next().await.unwrap(),
        Err(TypedError::Rtdb(rtdb_typed::rtdb_rs::RtdbError::Parse(_)))
    ));

    let base = server(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\nevent: put\ndata: {\"path\":\"/\",\"data\":{\"name\":\"Alice\"}}\n\n",
    )
    .await;
    let stream = client_from(base)
        .query::<User>("users/alice")
        .stream()
        .await
        .unwrap();
    tokio::pin!(stream);
    assert!(matches!(
        stream.next().await.unwrap(),
        Err(TypedError::Serde(_))
    ));
}

#[tokio::test]
async fn sse_preserves_events_across_chunk_boundaries_and_crlf() {
    let base = chunked_sse_server(&[
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\nevent: put\r\ndata: {\"path\":\"/\",\"data\":{\"name\":\"Alice\",",
        "\"score\":95}}\r\n\r\nevent: patch\r\ndata: {\"path\":\"/\",\"data\":{\"score\":100}}\r\n\r\nevent: keep-alive\r\n\r\n",
        "event: cancel\r\n\r\n",
    ])
    .await;
    let stream = client_from(base)
        .stream::<User>("users/alice")
        .await
        .unwrap();
    tokio::pin!(stream);
    assert!(matches!(
        stream.next().await.unwrap().unwrap(),
        TypedEvent::Put {
            data: Some(User { score: 95, .. }),
            ..
        }
    ));
    assert!(
        matches!(stream.next().await.unwrap().unwrap(), TypedEvent::Patch { patch, .. } if patch.as_value() == &json!({"score": 100}))
    );
    assert!(matches!(
        stream.next().await.unwrap().unwrap(),
        TypedEvent::KeepAlive
    ));
    assert!(matches!(
        stream.next().await.unwrap().unwrap(),
        TypedEvent::Cancel
    ));
    assert!(stream.next().await.is_none());
}
