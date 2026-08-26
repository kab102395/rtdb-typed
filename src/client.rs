use rtdb_rs::RtdbClient;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::TypedError;

/// Strongly typed wrapper around [`rtdb_rs::RtdbClient`].
///
/// `TypedClient` deliberately delegates transport, authentication, query URL
/// construction, and Firebase REST behavior to `rtdb-rs`. This crate is only
/// responsible for converting between Firebase JSON and application types.
pub struct TypedClient {
    inner: RtdbClient,
}

impl TypedClient {
    /// Wrap an existing `rtdb-rs` client.
    pub fn new(inner: RtdbClient) -> Self {
        Self { inner }
    }

    /// Build a typed client directly from a Firebase RTDB base URL and token.
    pub fn from_parts(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self::new(RtdbClient::new(base_url, token))
    }

    /// Access the underlying low-level client when an operation is not yet
    /// exposed by `rtdb-typed`.
    pub fn inner(&self) -> &RtdbClient {
        &self.inner
    }

    /// Read and deserialize a value.
    ///
    /// A missing Firebase node is represented by JSON `null`; deserializing
    /// `null` into a non-optional application type will return `TypedError::Serde`.
    /// Use [`Self::get_optional`] when a node may legitimately be absent.
    pub async fn get<T>(&self, path: &str) -> Result<T, TypedError>
    where
        T: DeserializeOwned,
    {
        let value = self.inner.get(path).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Read and deserialize an optional value. Firebase JSON `null` maps to
    /// `None`; any non-null value is deserialized into `T`.
    pub async fn get_optional<T>(&self, path: &str) -> Result<Option<T>, TypedError>
    where
        T: DeserializeOwned,
    {
        let value = self.inner.get(path).await?;
        decode_optional(value)
    }

    /// Replace a node and deserialize Firebase's response into `R`.
    pub async fn put<T, R>(&self, path: &str, value: &T) -> Result<R, TypedError>
    where
        T: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let body = serde_json::to_value(value)?;
        let response = self.inner.put(path, &body).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Patch a node and deserialize Firebase's response into `R`.
    pub async fn patch<T, R>(&self, path: &str, value: &T) -> Result<R, TypedError>
    where
        T: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let body = serde_json::to_value(value)?;
        let response = self.inner.patch(path, &body).await?;
        Ok(serde_json::from_value(response)?)
    }

    /// Append a child with a Firebase push key and return that generated key.
    pub async fn post<T>(&self, path: &str, value: &T) -> Result<String, TypedError>
    where
        T: Serialize + ?Sized,
    {
        let body = serde_json::to_value(value)?;
        let response = self.inner.post(path, &body).await?;

        response
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or(TypedError::MissingPushKey)
    }

    /// Delete a node.
    pub async fn delete(&self, path: &str) -> Result<(), TypedError> {
        self.inner.delete(path).await?;
        Ok(())
    }
}

fn decode_optional<T>(value: Value) -> Result<Option<T>, TypedError>
where
    T: DeserializeOwned,
{
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(serde_json::from_value(value)?))
    }
}

#[cfg(test)]
mod tests {
    use super::decode_optional;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct User {
        name: String,
        score: u32,
    }

    #[test]
    fn optional_null_becomes_none() {
        let decoded: Option<User> = decode_optional(serde_json::Value::Null).unwrap();
        assert_eq!(decoded, None);
    }

    #[test]
    fn optional_object_becomes_typed_value() {
        let decoded: Option<User> = decode_optional(json!({
            "name": "Alice",
            "score": 95
        }))
        .unwrap();

        assert_eq!(
            decoded,
            Some(User {
                name: "Alice".to_string(),
                score: 95,
            })
        );
    }
}
