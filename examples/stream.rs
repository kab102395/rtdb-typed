use futures_util::StreamExt;
use rtdb_typed::{TypedClient, TypedEvent};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
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
    let stream = client.query::<User>("users/alice").stream().await?;
    tokio::pin!(stream);
    while let Some(event) = stream.next().await {
        match event? {
            TypedEvent::Put {
                data: Some(data), ..
            } => {
                println!("full user: {} ({})", data.name, data.score)
            }
            TypedEvent::Put { data: None, .. } => println!("user deleted"),
            TypedEvent::Patch { patch, .. } => {
                if let Some(score) = patch.deserialize_field::<u32>("score")? {
                    println!("partial score update: {score}");
                }
            }
            TypedEvent::KeepAlive => {}
            TypedEvent::Cancel => break,
        }
    }
    Ok(())
}
