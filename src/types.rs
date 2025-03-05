use atrium_api::types::string::{Did, Handle};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct User {
    pub did: Did,
    pub handle: Handle,
}
