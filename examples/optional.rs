use rtdb_typed::TypedClient;
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
    let user: Option<User> = client.get_optional("users/missing").await?;
    println!(
        "missing user: {}",
        user.map(|user| user.name).unwrap_or_else(|| "none".into())
    );
    let _: Option<std::collections::HashMap<String, User>> =
        client.get_optional_collection("users/missing").await?;
    Ok(())
}
