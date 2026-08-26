use thiserror::Error;

#[derive(Debug, Error)]
pub enum TypedError {
    #[error(transparent)]
    Rtdb(#[from] rtdb_rs::RtdbError),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error("Firebase POST response did not contain a push key")]
    MissingPushKey,

    #[error("Firebase PATCH payload must be a JSON object")]
    InvalidPatch,
}
