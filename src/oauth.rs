use crate::{
    constant::{CALLBACK_PATH, CLIENT_METADATA_PATH, JWKS_PATH},
    store::FredStore,
};
use atrium_api::types::string::Did;
use atrium_common::{
    resolver::CachedResolver,
    types::cached::{
        r#impl::{Cache, CacheImpl},
        Cacheable,
    },
};
use atrium_identity::{
    did::{CommonDidResolver, CommonDidResolverConfig, DEFAULT_PLC_DIRECTORY_URL},
    handle::{AtprotoHandleResolver, AtprotoHandleResolverConfig, DnsTxtResolver},
};
use atrium_oauth::{
    store::{session::Session, state::InternalStateData},
    AtprotoClientMetadata, AuthMethod, DefaultHttpClient, GrantType, KnownScope, OAuthClient,
    OAuthClientConfig, OAuthResolverConfig, Scope,
};
use elliptic_curve::SecretKey;
use hickory_resolver::TokioAsyncResolver;
use jose_jwk::{Class, Jwk, Key, Parameters};
use pkcs8::DecodePrivateKey;
use std::sync::Arc;
use tower_sessions_redis_store::fred::clients::Pool;

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

pub type Client = OAuthClient<
    FredStore<String, InternalStateData>,
    FredStore<Did, Session>,
    CachedResolver<CommonDidResolver<DefaultHttpClient>>,
    CachedResolver<AtprotoHandleResolver<HickoryDnsTxtResolver, DefaultHttpClient>>,
>;

pub fn create_oauth_client(
    base_url: String,
    private_keys: Option<String>,
    redis_pool: Pool,
) -> atrium_oauth::Result<Client> {
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
            grant_types: vec![GrantType::AuthorizationCode, GrantType::RefreshToken],
            scopes: vec![
                Scope::Known(KnownScope::Atproto),
                Scope::Known(KnownScope::TransitionGeneric),
            ],
            jwks_uri: Some(format!("{base_url}{JWKS_PATH}")),
            token_endpoint_auth_signing_alg: Some(String::from("ES256")),
        },
        keys,
        state_store: FredStore::new(redis_pool.clone(), Some(String::from("state"))),
        session_store: FredStore::new(redis_pool.clone(), Some(String::from("session"))),
        resolver: OAuthResolverConfig {
            did_resolver: CommonDidResolver::new(CommonDidResolverConfig {
                plc_directory_url: DEFAULT_PLC_DIRECTORY_URL.to_string(),
                http_client: http_client.clone(),
            })
            .cached(CacheImpl::new(Default::default())),
            handle_resolver: AtprotoHandleResolver::new(AtprotoHandleResolverConfig {
                dns_txt_resolver: HickoryDnsTxtResolver::default(),
                http_client: http_client.clone(),
            })
            .cached(CacheImpl::new(Default::default())),
            authorization_server_metadata: Default::default(),
            protected_resource_metadata: Default::default(),
        },
    })
}
