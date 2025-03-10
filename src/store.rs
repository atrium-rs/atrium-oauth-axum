use atrium_api::types::string::Did;
use atrium_common::store::Store;
use atrium_oauth_client::store::{
    session::{Session, SessionStore},
    state::{InternalStateData, StateStore},
};
use serde::{de::DeserializeOwned, Serialize};
use std::hash::Hash;
use tower_sessions_redis_store::fred::{
    bytes::Bytes,
    clients::Pool,
    error::{Error, ErrorKind},
    prelude::KeysInterface,
    types::{FromValue, Value},
};

struct FredValue<T>(Option<T>);

impl<T> FromValue for FredValue<T>
where
    T: DeserializeOwned,
{
    fn from_value(value: Value) -> Result<Self, Error> {
        value
            .as_bytes()
            .map(|bytes| {
                serde_json::from_slice::<T>(bytes)
                    .map_err(|e| Error::new(ErrorKind::Unknown, e.to_string()))
            })
            .transpose()
            .map(FredValue)
    }
}

impl<T> TryFrom<FredValue<T>> for Value
where
    T: Serialize,
{
    type Error = Error;

    fn try_from(value: FredValue<T>) -> Result<Self, Self::Error> {
        match value.0 {
            Some(value) => serde_json::to_vec(&value)
                .map(|data| Value::Bytes(Bytes::from(data)))
                .map_err(|e| Error::new(ErrorKind::Unknown, e.to_string())),
            None => Ok(Value::Null),
        }
    }
}

pub struct FredStore<K, V> {
    client: Pool,
    prefix: Option<String>,
    _marker: std::marker::PhantomData<(K, V)>,
}

impl<K, V> FredStore<K, V> {
    pub fn new(client: Pool, prefix: Option<String>) -> Self {
        Self {
            client,
            prefix,
            _marker: std::marker::PhantomData,
        }
    }
    fn key(&self, key: &K) -> String
    where
        K: AsRef<str>,
    {
        match &self.prefix {
            Some(prefix) => format!("{prefix}:{}", key.as_ref()),
            None => key.as_ref().into(),
        }
    }
}

impl<K, V> Store<K, V> for FredStore<K, V>
where
    K: Eq + Hash + AsRef<str> + Send + Sync,
    V: Clone + Serialize + DeserializeOwned + Send + Sync,
{
    type Error = Error;

    async fn get(&self, key: &K) -> Result<Option<V>, Self::Error> {
        self.client
            .get::<FredValue<V>, _>(self.key(key))
            .await
            .map(|result| result.0)
    }
    async fn set(&self, key: K, value: V) -> Result<(), Self::Error> {
        self.client
            .set::<String, _, _>(self.key(&key), FredValue(Some(value)), None, None, false)
            .await
            .map(|_| ())
    }
    async fn del(&self, key: &K) -> Result<(), Self::Error> {
        self.client
            .del::<String, _>(self.key(key))
            .await
            .map(|_| ())
    }
    async fn clear(&self) -> Result<(), Self::Error> {
        unimplemented!()
    }
}

impl StateStore for FredStore<String, InternalStateData> {}

impl SessionStore for FredStore<Did, Session> {}
