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
/// `Put` contains the complete model when the node exists, and `None` when
/// Firebase reports a deletion/null value. `Patch` contains only changed
/// fields through [`TypedPatch`]; it is never deserialized as a complete `T`.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedEvent<T> {
    Put {
        path: String,
        data: Option<T>,
    },
    /// A partial Firebase update.
    Patch {
        path: String,
        data: TypedPatch,
    },
    KeepAlive,
    Cancel,
}

/// A partial Firebase update that preserves the changed JSON object.
///
/// Patch fields are optional by definition: an omitted field was not changed,
/// and must not be treated as a default value. Use [`Self::deserialize_field`]
/// to decode only fields that are present, or [`Self::apply_to`] to merge the
/// update into an existing Serde model.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedPatch(Value);

impl TypedPatch {
    fn from_value(value: Value) -> Result<Self, TypedError> {
        if value.is_object() {
            Ok(Self(value))
        } else {
            Err(TypedError::InvalidPatch)
        }
    }

    /// Return the original patch JSON object.
    pub fn as_value(&self) -> &Value {
        &self.0
    }

    /// Return the original patch as a JSON object.
    pub fn as_object(&self) -> Option<&serde_json::Map<String, Value>> {
        self.0.as_object()
    }

    /// Return whether the patch changes `field`.
    pub fn contains_key(&self, field: &str) -> bool {
        self.0
            .as_object()
            .is_some_and(|object| object.contains_key(field))
    }

    /// Return the raw changed value for `field`, if present.
    pub fn get(&self, field: &str) -> Option<&Value> {
        self.0.as_object().and_then(|object| object.get(field))
    }

    /// Iterate over changed field names.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0
            .as_object()
            .into_iter()
            .flat_map(|object| object.keys().map(String::as_str))
    }

    /// Deserialize one changed field. Missing fields return `None`.
    pub fn deserialize_field<T>(&self, field: &str) -> Result<Option<T>, TypedError>
    where
        T: DeserializeOwned,
    {
        self.get(field)
            .map(|value| Ok(serde_json::from_value(value.clone())?))
            .transpose()
    }

    /// Apply this shallow patch to an existing JSON object.
    pub fn apply_to_value(&self, target: &mut Value) -> Result<(), TypedError> {
        let target = target.as_object_mut().ok_or(TypedError::InvalidPatch)?;
        for (key, value) in self.0.as_object().expect("validated patch object") {
            target.insert(key.clone(), value.clone());
        }
        Ok(())
    }

    /// Apply this patch to an existing Serde model and decode the result.
    pub fn apply_to<T>(&self, current: &T) -> Result<T, TypedError>
    where
        T: Serialize + DeserializeOwned,
    {
        let mut value = serde_json::to_value(current)?;
        self.apply_to_value(&mut value)?;
        Ok(serde_json::from_value(value)?)
    }
}

/// A Firebase object-map collection with a stable typed API.
///
/// Firebase represents collections as JSON objects keyed by Firebase child
/// keys. A missing collection is represented by JSON `null` and is converted
/// to an empty collection by [`TypedClient::get_collection`] and
/// [`TypedQuery::send_collection`]. Use the optional client method when the
/// distinction between a missing and an empty collection matters.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FirebaseCollection<T>(HashMap<String, T>);

impl<T> FirebaseCollection<T> {
    /// Construct a collection from its Firebase key/value representation.
    pub fn new(values: HashMap<String, T>) -> Self {
        Self(values)
    }

    /// Return the number of children in the collection.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return whether the collection contains no children.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return a child by Firebase key.
    pub fn get(&self, key: &str) -> Option<&T> {
        self.0.get(key)
    }

    /// Return whether a Firebase child key exists.
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Iterate over Firebase child keys.
    pub fn keys(&self) -> std::collections::hash_map::Keys<'_, String, T> {
        self.0.keys()
    }

    /// Iterate over collection values.
    pub fn values(&self) -> std::collections::hash_map::Values<'_, String, T> {
        self.0.values()
    }

    /// Iterate over `(key, value)` pairs.
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, String, T> {
        self.0.iter()
    }

    /// Consume the wrapper and return its key/value representation.
    pub fn into_inner(self) -> HashMap<String, T> {
        self.0
    }
}

impl<T> From<HashMap<String, T>> for FirebaseCollection<T> {
    fn from(values: HashMap<String, T>) -> Self {
        Self::new(values)
    }
}

impl<T> IntoIterator for FirebaseCollection<T> {
    type Item = (String, T);
    type IntoIter = std::collections::hash_map::IntoIter<String, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a FirebaseCollection<T> {
    type Item = (&'a String, &'a T);
    type IntoIter = std::collections::hash_map::Iter<'a, String, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// The stable result of creating a Firebase child with a push key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushResult {
    /// The generated Firebase child key.
    pub key: String,
    /// The created child path, when it can be derived from the request path.
    pub path: Option<String>,
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

    /// Execute the query and decode JSON `null` as `None`.
    pub async fn send_optional(self) -> Result<Option<T>, TypedError> {
        decode_optional(self.inner.send().await?)
    }

    /// Execute the query as a Firebase object-map collection.
    ///
    /// JSON `null` is treated as an empty collection.
    pub async fn send_collection(self) -> Result<FirebaseCollection<T>, TypedError> {
        let value = self.inner.send().await?;
        if value.is_null() {
            return Ok(FirebaseCollection::new(HashMap::new()));
        }
        Ok(FirebaseCollection::new(serde_json::from_value(value)?))
    }

    pub async fn stream(
        self,
    ) -> Result<impl futures_util::Stream<Item = Result<TypedEvent<T>, TypedError>>, TypedError>
    {
        Ok(convert_stream(self.inner.stream().await?))
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

    /// Open a typed realtime stream at `path`.
    pub async fn stream<T>(
        &self,
        path: &str,
    ) -> Result<impl futures_util::Stream<Item = Result<TypedEvent<T>, TypedError>>, TypedError>
    where
        T: DeserializeOwned + 'static,
    {
        Ok(convert_stream(self.inner.stream(path).await?))
    }

    /// Read a Firebase object map as typed `(key, value)` entries.
    ///
    /// A missing node (`null`) is treated as an empty map. Use
    /// [`Self::get_optional_collection`] when that distinction matters.
    pub async fn get_collection<T>(&self, path: &str) -> Result<FirebaseCollection<T>, TypedError>
    where
        T: DeserializeOwned,
    {
        let value = self.inner.get(path).await?;
        if value.is_null() {
            return Ok(FirebaseCollection::new(HashMap::new()));
        }
        Ok(FirebaseCollection::new(serde_json::from_value(value)?))
    }

    /// Read a Firebase object map while preserving whether the node was
    /// missing. A missing node is `None`; an existing empty object is `Some`.
    pub async fn get_optional_collection<T>(
        &self,
        path: &str,
    ) -> Result<Option<FirebaseCollection<T>>, TypedError>
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

    /// Append a child with a Firebase push key and return its stable result.
    pub async fn post<T>(&self, path: &str, value: &T) -> Result<PushResult, TypedError>
    where
        T: Serialize + ?Sized,
    {
        let body = serde_json::to_value(value)?;
        let response = self.inner.post(path, &body).await?;

        let key = response
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or(TypedError::MissingPushKey)?;
        Ok(PushResult {
            path: Some(format_push_path(path, &key)),
            key,
        })
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

fn convert_event<T>(event: rtdb_rs::RtdbEvent) -> Result<TypedEvent<T>, TypedError>
where
    T: DeserializeOwned,
{
    match event {
        rtdb_rs::RtdbEvent::Put { path, data } => Ok(TypedEvent::Put {
            path,
            data: decode_optional(data)?,
        }),
        rtdb_rs::RtdbEvent::Patch { path, data } => Ok(TypedEvent::Patch {
            path,
            data: TypedPatch::from_value(data)?,
        }),
        rtdb_rs::RtdbEvent::KeepAlive => Ok(TypedEvent::KeepAlive),
        rtdb_rs::RtdbEvent::Cancel => Ok(TypedEvent::Cancel),
    }
}

fn convert_stream<T, S>(
    stream: S,
) -> impl futures_util::Stream<Item = Result<TypedEvent<T>, TypedError>>
where
    T: DeserializeOwned,
    S: futures_util::Stream<Item = Result<rtdb_rs::RtdbEvent, rtdb_rs::RtdbError>>,
{
    futures_util::StreamExt::scan(stream, false, |cancelled, event| {
        if *cancelled {
            return futures_util::future::ready(None);
        }
        let converted = event.map_err(TypedError::from).and_then(convert_event);
        if matches!(converted, Ok(TypedEvent::Cancel)) {
            *cancelled = true;
        }
        futures_util::future::ready(Some(converted))
    })
}

fn format_push_path(parent: &str, key: &str) -> String {
    let parent = parent.trim_matches('/');
    if parent.is_empty() {
        format!("/{key}")
    } else {
        format!("/{parent}/{key}")
    }
}

#[cfg(test)]
mod tests {
    use super::{convert_event, decode_optional, format_push_path, FirebaseCollection, TypedEvent};
    use crate::TypedError;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::collections::HashMap;

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

    #[test]
    fn collection_deserializes_and_exposes_key_value_operations() {
        let collection: FirebaseCollection<User> = serde_json::from_value(serde_json::json!({
            "alice": {"name": "Alice", "score": 95},
            "bob": {"name": "Bob", "score": 80}
        }))
        .unwrap();

        assert_eq!(collection.len(), 2);
        assert!(!collection.is_empty());
        assert!(collection.contains_key("alice"));
        assert_eq!(collection.get("alice").unwrap().score, 95);
        assert_eq!(collection.keys().count(), 2);
        assert_eq!(collection.values().count(), 2);
        assert_eq!(collection.iter().count(), 2);
        assert_eq!(collection.clone().into_inner().len(), 2);
        assert_eq!(collection.into_iter().count(), 2);
    }

    #[test]
    fn collection_supports_empty_and_malformed_shapes_explicitly() {
        let empty: FirebaseCollection<User> =
            serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(empty.is_empty());

        let malformed = serde_json::from_value::<FirebaseCollection<User>>(serde_json::json!({
            "alice": {"name": "Alice"}
        }));
        assert!(malformed.is_err());

        let mut values = HashMap::new();
        values.insert(
            "alice".to_string(),
            User {
                name: "Alice".into(),
                score: 95,
            },
        );
        let from_map = FirebaseCollection::from(values);
        assert!(from_map.contains_key("alice"));
    }

    #[test]
    fn push_paths_are_derived_without_double_slashes() {
        assert_eq!(format_push_path("users", "-Nkey"), "/users/-Nkey");
        assert_eq!(format_push_path("/users/", "-Nkey"), "/users/-Nkey");
        assert_eq!(format_push_path("/", "-Nkey"), "/-Nkey");
    }

    #[test]
    fn representative_serde_values_round_trip() {
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        struct Profile {
            enabled: bool,
            tags: Vec<String>,
            scores: HashMap<String, u32>,
            nickname: Option<String>,
        }

        let profile = Profile {
            enabled: true,
            tags: vec!["rust".into(), "firebase".into()],
            scores: HashMap::from([(String::from("first"), 1)]),
            nickname: None,
        };
        let encoded = serde_json::to_value(&profile).unwrap();
        assert_eq!(serde_json::from_value::<Profile>(encoded).unwrap(), profile);
        assert!(serde_json::from_value::<Profile>(serde_json::json!({
            "enabled": true,
            "tags": []
        }))
        .is_err());
    }

    #[test]
    fn event_conversion_preserves_complete_null_partial_and_control_events() {
        let put = convert_event::<User>(rtdb_rs::RtdbEvent::Put {
            path: "/users/alice".into(),
            data: json!({"name": "Alice", "score": 95}),
        })
        .unwrap();
        assert!(matches!(
            put,
            TypedEvent::Put {
                data: Some(User { score: 95, .. }),
                ..
            }
        ));

        let null_put = convert_event::<User>(rtdb_rs::RtdbEvent::Put {
            path: "/users/alice".into(),
            data: serde_json::Value::Null,
        })
        .unwrap();
        assert!(matches!(null_put, TypedEvent::Put { data: None, .. }));

        let patch = convert_event::<User>(rtdb_rs::RtdbEvent::Patch {
            path: "/users/alice".into(),
            data: json!({"profile": {"score": 100}, "score": 100}),
        })
        .unwrap();
        let TypedEvent::Patch { data: patch, .. } = patch else {
            panic!("expected patch")
        };
        assert!(patch.contains_key("profile"));
        assert_eq!(patch.keys().collect::<Vec<_>>(), vec!["profile", "score"]);
        assert_eq!(patch.deserialize_field::<u32>("score").unwrap(), Some(100));
        assert_eq!(patch.deserialize_field::<u32>("missing").unwrap(), None);
        let mut current = User {
            name: "Alice".into(),
            score: 95,
        };
        current = patch.apply_to(&current).unwrap();
        assert_eq!(current.score, 100);
        assert_eq!(patch.as_value()["profile"]["score"], 100);

        assert!(matches!(
            convert_event::<User>(rtdb_rs::RtdbEvent::Patch {
                path: "/".into(),
                data: json!(42),
            }),
            Err(TypedError::InvalidPatch)
        ));
        assert!(matches!(
            convert_event::<User>(rtdb_rs::RtdbEvent::KeepAlive).unwrap(),
            TypedEvent::KeepAlive
        ));
        assert!(matches!(
            convert_event::<User>(rtdb_rs::RtdbEvent::Cancel).unwrap(),
            TypedEvent::Cancel
        ));
    }
}
