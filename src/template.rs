use crate::types::User;
use askama::Template;
use askama_derive_axum::IntoResponse;

pub enum Page {
    OAuthLogin,
    OAuthLogout,
    BskyPost,
}

pub fn url_for(page: Page) -> &'static str {
    match page {
        Page::OAuthLogin => "/oauth/login",
        Page::OAuthLogout => "/oauth/logout",
        Page::BskyPost => "/bsky/post",
    }
}

pub struct GlobalContext {
    pub user: Option<User>,
}

#[derive(Template, IntoResponse)]
#[template(path = "home.html")]
pub struct Home {
    pub g: GlobalContext,
}

#[derive(Template, IntoResponse)]
#[template(path = "login.html")]
pub struct Login {
    pub g: GlobalContext,
}

#[derive(Template, IntoResponse)]
#[template(path = "bsky_post.html")]
pub struct BskyPost {
    pub g: GlobalContext,
}
