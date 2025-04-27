use atrium_api::types::string::{Did, Handle};
use atrium_common::resolver::Resolver;
use atrium_identity::{
    did::{CommonDidResolver, CommonDidResolverConfig, DEFAULT_PLC_DIRECTORY_URL},
    Error,
};
use atrium_oauth::DefaultHttpClient;
use std::sync::Arc;

pub async fn resolve_identity(did: &Did) -> Result<Option<Handle>, Error> {
    let resolver = CommonDidResolver::new(CommonDidResolverConfig {
        plc_directory_url: DEFAULT_PLC_DIRECTORY_URL.into(),
        http_client: Arc::new(DefaultHttpClient::default()),
    });
    let document = resolver.resolve(did).await?;
    Ok(document.also_known_as.and_then(|aka| {
        aka.iter()
            .find_map(|aka| aka.strip_prefix("at://").and_then(|s| s.parse().ok()))
    }))
}
