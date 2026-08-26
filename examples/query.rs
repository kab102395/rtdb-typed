use rtdb_typed::TypedClient;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct User {
    name: String,
    active: bool,
}

#[tokio::main]
async fn main() -> Result<(), rtdb_typed::TypedError> {
    let client = TypedClient::from_parts(
        std::env::var("RTDB_URL").expect("RTDB_URL must be set"),
        std::env::var("RTDB_TOKEN").unwrap_or_default(),
    );
    let users: HashMap<String, User> = client
        .query("users")
        .order_by_child("active")
        .equal_to(rtdb_typed::rtdb_rs::FilterValue::boolean(true))
        .limit_to_first(25)
        .send()
        .await?;
    for (key, user) in users {
        println!("{key}: {} ({})", user.name, user.active);
    }
    Ok(())
}
