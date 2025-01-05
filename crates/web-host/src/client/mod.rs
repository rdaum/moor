// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use axum::body::Body;
use axum::http::{header, HeaderValue};
use axum::response::{Html, IntoResponse, Response};

#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct Js<T>(pub T);

impl<T> IntoResponse for Js<T>
where
    T: Into<Body>,
{
    fn into_response(self) -> Response {
        (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/javascript; charset=utf-8"),
            )],
            self.0.into(),
        )
            .into_response()
    }
}

impl<T> From<T> for Js<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

// Single macro that includes local static file and exposes it as a handler.
macro_rules! static_html_handler {
    ($name:ident, $path:expr) => {
        pub async fn $name() -> impl IntoResponse {
            Html(include_str!($path))
        }
    };
}

macro_rules! static_js_handler {
    ($name:ident, $path:expr) => {
        pub async fn $name() -> impl IntoResponse {
            Js(include_str!($path))
        }
    };
}

macro_rules! static_css_handler {
    ($name:ident, $path:expr) => {
        pub async fn $name() -> impl IntoResponse {
            (
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/css; charset=utf-8"),
                )],
                include_str!($path),
            )
                .into_response()
        }
    };
}

static_html_handler!(root_handler, "root.html");
static_js_handler!(js_handler, "moor.js");
static_js_handler!(var_handler, "var.js");
static_js_handler!(editor_handler, "editor.js");
static_js_handler!(rpc_handler, "rpc.js");
static_css_handler!(css_handler, "moor.css");
