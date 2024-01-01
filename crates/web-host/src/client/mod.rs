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
