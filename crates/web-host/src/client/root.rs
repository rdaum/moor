use axum::http::header;
use axum::response::{Html, IntoResponse};

static ROOT_HTML: &str = include_str!("root.html");
static BROWSER_HTML: &str = include_str!("browser.html");
static MOOR_JS: &str = include_str!("moor.js");
pub async fn root_handler() -> impl IntoResponse {
    Html(ROOT_HTML)
}
pub async fn browser_handler() -> impl IntoResponse {
    Html(BROWSER_HTML)
}
pub async fn js_handler() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/javascript")], MOOR_JS)
}
