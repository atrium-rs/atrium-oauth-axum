use crate::constant::{CALLBACK_PATH, CLIENT_METADATA_PATH, JWKS_PATH};
use atrium_identity::did::{CommonDidResolver, CommonDidResolverConfig, DEFAULT_PLC_DIRECTORY_URL};
use atrium_identity::handle::{AtprotoHandleResolver, AtprotoHandleResolverConfig, DnsTxtResolver};
use atrium_oauth_client::store::memory::MemorySimpleStore;
use atrium_oauth_client::store::state::{InternalStateData, MemoryStateStore};
use atrium_oauth_client::{
    AtprotoClientMetadata, AuthMethod, DefaultHttpClient, GrantType, KnownScope, OAuthClient,
    OAuthClientConfig, OAuthResolverConfig, Result, Scope,
};
use elliptic_curve::SecretKey;
use hickory_resolver::TokioAsyncResolver;
use jose_jwk::{Class, Jwk, Key, Parameters};
use pkcs8::DecodePrivateKey;
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

pub type Client = OAuthClient<
    MemorySimpleStore<String, InternalStateData>,
    CommonDidResolver<DefaultHttpClient>,
    AtprotoHandleResolver<HickoryDnsTxtResolver, DefaultHttpClient>,
>;

pub fn create_oauth_client(base_url: String, private_key: Option<String>) -> Result<Client> {
    let http_client = Arc::new(DefaultHttpClient::default());
    let keys = private_key
        .and_then(|s| SecretKey::<p256::NistP256>::from_pkcs8_pem(&s).ok())
        .map(|secret_key| {
            vec![Jwk {
                key: Key::from(&secret_key.into()),
                prm: Parameters {
                    kid: Some(String::from("key01")),
                    cls: Some(Class::Signing),
                    ..Default::default()
                },
            }]
        });
    OAuthClient::new(OAuthClientConfig {
        client_metadata: AtprotoClientMetadata {
            client_id: format!("{base_url}{CLIENT_METADATA_PATH}"),
            client_uri: base_url.clone(),
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
        state_store: MemoryStateStore::default(),
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
