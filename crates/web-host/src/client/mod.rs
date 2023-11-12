use axum::response::{Html, IntoResponse};

// Single macro that includes local static file and exposes it as a handler.
macro_rules! static_handler {
    ($name:ident, $path:expr) => {
        pub async fn $name() -> impl IntoResponse {
            Html(include_str!($path))
        }
    };
}

static_handler!(root_handler, "root.html");
static_handler!(browser_handler, "browser.html");
static_handler!(js_handler, "moor.js");
static_handler!(editor_handler, "editor.js");
