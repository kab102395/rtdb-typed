use rtdb_rs::RtdbClient;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::marker::PhantomData;

use crate::TypedError;

/// Strongly typed wrapper around [`rtdb_rs::RtdbClient`].
///
/// `TypedClient` deliberately delegates transport, authentication, query URL
/// construction, and Firebase REST behavior to `rtdb-rs`. This crate is only
/// responsible for converting between Firebase JSON and application types.
pub struct TypedClient {
    inner: RtdbClient,
}

/// A result from a Firebase realtime stream.
///
/// `Put` is deserialized into the complete model `T`. `Patch` intentionally
/// remains raw JSON because Firebase sends only changed fields for patches.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedEvent<T> {
    Put {
        path: String,
        data: T,
    },
    /// A partial Firebase update. The payload is intentionally raw JSON
    /// because omitted fields are valid and cannot be deserialized as a full
    /// `T` without inventing application state.
    Patch {
        path: String,
        data: Value,
    },
    KeepAlive,
    Cancel,
}

/// A typed wrapper around [`rtdb_rs::GetBuilder`].
pub struct TypedQuery<'a, T> {
    inner: rtdb_rs::GetBuilder<'a>,
    marker: PhantomData<fn() -> T>,
}

impl<'a, T> TypedQuery<'a, T>
where
    T: DeserializeOwned + 'static,
{
    fn new(inner: rtdb_rs::GetBuilder<'a>) -> Self {
        Self {
            inner,
            marker: PhantomData,
        }
    }

    pub fn order_by_child(mut self, field: &str) -> Self {
        self.inner = self.inner.order_by_child(field);
        self
    }

    pub fn order_by_key(mut self) -> Self {
        self.inner = self.inner.order_by_key();
        self
    }

    pub fn order_by_value(mut self) -> Self {
        self.inner = self.inner.order_by_value();
        self
    }

    pub fn order_by(mut self, order: rtdb_rs::OrderBy) -> Self {
        self.inner = self.inner.order_by(order);
        self
    }

    pub fn limit_to_first(mut self, n: u32) -> Self {
        self.inner = self.inner.limit_to_first(n);
        self
    }

    pub fn limit_to_last(mut self, n: u32) -> Self {
        self.inner = self.inner.limit_to_last(n);
        self
    }

    pub fn start_at(mut self, value: rtdb_rs::FilterValue) -> Self {
        self.inner = self.inner.start_at(value);
        self
    }

    pub fn end_at(mut self, value: rtdb_rs::FilterValue) -> Self {
        self.inner = self.inner.end_at(value);
        self
    }

    pub fn equal_to(mut self, value: rtdb_rs::FilterValue) -> Self {
        self.inner = self.inner.equal_to(value);
        self
    }

    pub fn shallow(mut self) -> Self {
        self.inner = self.inner.shallow();
        self
    }

    pub async fn send(self) -> Result<T, TypedError> {
        Ok(serde_json::from_value(self.inner.send().await?)?)
    }

    pub async fn stream(
        self,
    ) -> Result<impl futures_util::Stream<Item = Result<TypedEvent<T>, TypedError>>, TypedError>
    {
        let stream = self.inner.stream().await?;
        Ok(futures_util::StreamExt::map(stream, |event| {
            event
                .map_err(TypedError::from)
                .and_then(|event| match event {
                    rtdb_rs::RtdbEvent::Put { path, data } => Ok(TypedEvent::Put {
                        path,
                        data: serde_json::from_value(data)?,
                    }),
                    rtdb_rs::RtdbEvent::Patch { path, data } => {
                        Ok(TypedEvent::Patch { path, data })
                    }
                    rtdb_rs::RtdbEvent::KeepAlive => Ok(TypedEvent::KeepAlive),
                    rtdb_rs::RtdbEvent::Cancel => Ok(TypedEvent::Cancel),
                })
        }))
    }
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

    /// Start a typed one-shot query or realtime stream.
    pub fn query<T>(&self, path: &str) -> TypedQuery<'_, T>
    where
        T: DeserializeOwned + 'static,
    {
        TypedQuery::new(self.inner.query(path))
    }

    /// Read a Firebase object map as typed `(key, value)` entries.
    ///
    /// A missing node (`null`) is treated as an empty map. Use
    /// [`Self::get_optional_collection`] when that distinction matters.
    pub async fn get_collection<T>(
        &self,
        path: &str,
    ) -> Result<std::collections::HashMap<String, T>, TypedError>
    where
        T: DeserializeOwned,
    {
        let value = self.inner.get(path).await?;
        if value.is_null() {
            return Ok(HashMap::new());
        }
        Ok(serde_json::from_value(value)?)
    }

    /// Read a Firebase object map while preserving whether the node was
    /// missing. A missing node is `None`; an existing empty object is `Some`.
    pub async fn get_optional_collection<T>(
        &self,
        path: &str,
    ) -> Result<Option<HashMap<String, T>>, TypedError>
    where
        T: DeserializeOwned,
    {
        let value = self.inner.get(path).await?;
        decode_optional(value)
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
