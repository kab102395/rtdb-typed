use rtdb_typed::{FirebaseCollection, TypedClient};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct User {
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), rtdb_typed::TypedError> {
    let client = TypedClient::from_parts(
        std::env::var("RTDB_URL").expect("RTDB_URL must be set"),
        std::env::var("RTDB_TOKEN").unwrap_or_default(),
    );
    let users: FirebaseCollection<User> = client.get_collection("users").await?;
    println!("{} users", users.len());
    if let Some(user) = users.get("alice") {
        println!("Alice is {}", user.name);
    }
    Ok(())
}
