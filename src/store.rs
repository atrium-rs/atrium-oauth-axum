use atrium_api::types::string::Did;
use atrium_common::store::Store;
use atrium_oauth_client::store::{
    session::{Session, SessionStore},
    state::{InternalStateData, StateStore},
};
use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};
use std::{hash::Hash, sync::Arc};

pub struct RedisStore<K, V> {
    client: Arc<redis::Client>,
    prefix: Option<String>,
    _marker: std::marker::PhantomData<(K, V)>,
}

impl<K, V> RedisStore<K, V> {
    pub fn new(client: Arc<redis::Client>, prefix: Option<String>) -> Self {
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
            Some(prefix) => format!("{}:{}", prefix, key.as_ref()),
            None => key.as_ref().into(),
        }
    }
}

impl<K, V> Store<K, V> for RedisStore<K, V>
where
    K: Eq + Hash + AsRef<str> + Send + Sync,
    V: Clone + Serialize + DeserializeOwned + Send + Sync,
{
    type Error = redis::RedisError;

    async fn get(&self, key: &K) -> Result<Option<V>, Self::Error> {
        self.client
            .get_multiplexed_async_connection()
            .await?
            .get::<_, Option<String>>(self.key(key))
            .await
            .map(|value| {
                value.map(|value| serde_json::from_str(&value).expect("failed to deserialize JSON"))
            })
    }
    async fn set(&self, key: K, value: V) -> Result<(), Self::Error> {
        self.client
            .get_multiplexed_async_connection()
            .await?
            .set(
                self.key(&key),
                serde_json::to_string(&value).expect("failed to serialize JSON"),
            )
            .await
    }
    async fn del(&self, key: &K) -> Result<(), Self::Error> {
        self.client
            .get_multiplexed_async_connection()
            .await?
            .del(self.key(key))
            .await
    }
    async fn clear(&self) -> Result<(), Self::Error> {
        unimplemented!()
    }
}

impl StateStore for RedisStore<String, InternalStateData> {}

impl SessionStore for RedisStore<Did, Session> {}
