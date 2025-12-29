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

//! Spec file parser

mod frontmatter;

use crate::ast::*;
use eyre::Result;

pub use frontmatter::parse_frontmatter;

/// Parse a spec file from markdown content
pub fn parse_spec(content: &str) -> Result<Spec> {
    let (frontmatter, _body) = parse_frontmatter(content)?;

    // TODO: Parse markdown body
    let sections = vec![];

    Ok(Spec {
        frontmatter,
        sections,
    })
}
