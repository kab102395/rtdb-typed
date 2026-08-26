use rtdb_typed::TypedClient;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct User {
    name: String,
    score: u32,
}

#[tokio::main]
async fn main() -> Result<(), rtdb_typed::TypedError> {
    let client = TypedClient::from_parts(
        std::env::var("RTDB_URL").expect("RTDB_URL must be set"),
        std::env::var("RTDB_TOKEN").unwrap_or_default(),
    );
    let user = User {
        name: "Alice".into(),
        score: 95,
    };
    let _: User = client.put("examples/users/alice", &user).await?;
    let loaded: User = client.get("examples/users/alice").await?;
    println!("{loaded:?}");
    client.delete("examples/users/alice").await?;
    Ok(())
}
