use crate::constant::{CALLBACK_PATH, CLIENT_METADATA_PATH, JWKS_PATH};
use atrium_common::store::Store;
use atrium_identity::did::{CommonDidResolver, CommonDidResolverConfig, DEFAULT_PLC_DIRECTORY_URL};
use atrium_identity::handle::{AtprotoHandleResolver, AtprotoHandleResolverConfig, DnsTxtResolver};
use atrium_oauth_client::store::session::MemorySessionStore;
use atrium_oauth_client::store::state::{InternalStateData, StateStore};
use atrium_oauth_client::{
    AtprotoClientMetadata, AuthMethod, DefaultHttpClient, GrantType, KnownScope, OAuthClient,
    OAuthClientConfig, OAuthResolverConfig, Scope,
};
use elliptic_curve::SecretKey;
use hickory_resolver::TokioAsyncResolver;
use jose_jwk::{Class, Jwk, Key, Parameters};
use pkcs8::DecodePrivateKey;
use redis::AsyncCommands;
use std::sync::Arc;

pub struct HickoryDnsTxtResolver {
    resolver: TokioAsyncResolver,
}

impl Default for HickoryDnsTxtResolver {
    fn default() -> Self {
        Self {
            resolver: TokioAsyncResolver::tokio_from_system_conf()
                .expect("failed to create resolver"),
        }
    }
}

impl DnsTxtResolver for HickoryDnsTxtResolver {
    async fn resolve(
        &self,
        query: &str,
    ) -> core::result::Result<Vec<String>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(self
            .resolver
            .txt_lookup(query)
            .await?
            .iter()
            .map(|txt| txt.to_string())
            .collect())
    }
}

pub struct RedisStateStore {
    client: Arc<redis::Client>,
}

impl Store<String, InternalStateData> for RedisStateStore {
    type Error = redis::RedisError;

    async fn get(&self, key: &String) -> Result<Option<InternalStateData>, Self::Error> {
        self.client
            .get_multiplexed_async_connection()
            .await?
            .get::<_, Option<String>>(key)
            .await
            .map(|value| {
                value.map(|value| serde_json::from_str(&value).expect("failed to deserialize JSON"))
            })
    }
    async fn set(&self, key: String, value: InternalStateData) -> Result<(), Self::Error> {
        self.client
            .get_multiplexed_async_connection()
            .await?
            .set(
                key,
                serde_json::to_string(&value).expect("failed to serialize JSON"),
            )
            .await
    }
    async fn del(&self, key: &String) -> Result<(), Self::Error> {
        // TODO
        Ok(())
    }
    async fn clear(&self) -> Result<(), Self::Error> {
        unimplemented!()
    }
}

impl StateStore for RedisStateStore {}

pub type Client = OAuthClient<
    RedisStateStore,
    MemorySessionStore,
    CommonDidResolver<DefaultHttpClient>,
    AtprotoHandleResolver<HickoryDnsTxtResolver, DefaultHttpClient>,
>;

pub fn create_oauth_client(
    base_url: String,
    private_keys: Option<String>,
    redis_client: Arc<redis::Client>,
) -> atrium_oauth_client::Result<Client> {
    let http_client = Arc::new(DefaultHttpClient::default());
    let keys = private_keys.map(|keys| {
        keys.split(',')
            .enumerate()
            .filter_map(|(i, s)| {
                SecretKey::<p256::NistP256>::from_pkcs8_pem(s)
                    .map(|secret_key| Jwk {
                        key: Key::from(&secret_key.into()),
                        prm: Parameters {
                            kid: Some(format!("kid-{i:02}")),
                            cls: Some(Class::Signing),
                            ..Default::default()
                        },
                    })
                    .ok()
            })
            .collect::<Vec<_>>()
    });
    OAuthClient::new(OAuthClientConfig {
        client_metadata: AtprotoClientMetadata {
            client_id: format!("{base_url}{CLIENT_METADATA_PATH}"),
            client_uri: Some(base_url.clone()),
            redirect_uris: vec![format!("{base_url}{CALLBACK_PATH}")],
            token_endpoint_auth_method: AuthMethod::PrivateKeyJwt,
            grant_types: vec![GrantType::AuthorizationCode],
            scopes: vec![
                Scope::Known(KnownScope::Atproto),
                Scope::Known(KnownScope::TransitionGeneric),
            ],
            jwks_uri: Some(format!("{base_url}{JWKS_PATH}")),
            token_endpoint_auth_signing_alg: Some(String::from("ES256")),
        },
        keys,
        state_store: RedisStateStore {
            client: redis_client,
        },
        session_store: MemorySessionStore::default(),
        resolver: OAuthResolverConfig {
            did_resolver: CommonDidResolver::new(CommonDidResolverConfig {
                plc_directory_url: DEFAULT_PLC_DIRECTORY_URL.to_string(),
                http_client: http_client.clone(),
            }),
            handle_resolver: AtprotoHandleResolver::new(AtprotoHandleResolverConfig {
                dns_txt_resolver: HickoryDnsTxtResolver::default(),
                http_client: http_client.clone(),
            }),
            authorization_server_metadata: Default::default(),
            protected_resource_metadata: Default::default(),
        },
    })
}
