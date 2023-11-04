use axum::response::{Html, IntoResponse};

static ROOT_HTML: &str = include_str!("root.html");
pub async fn root_handler() -> impl IntoResponse {
    Html(ROOT_HTML)
}
