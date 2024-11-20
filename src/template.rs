use askama::Template;

pub enum Page {
    OAuthLogin,
}

pub fn url_for(page: Page) -> &'static str {
    match page {
        Page::OAuthLogin => "/oauth/login",
    }
}

#[derive(Template)]
#[template(path = "home.html")]
pub struct Home {}
