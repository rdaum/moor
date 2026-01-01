// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

pub struct MootStylesheet<S: std::fmt::Display> {
    pub test_header: S,
    pub block_header: S,
    pub remote: S,
    pub arrows: S,
    pub request: S,
    pub response: S,
}

#[cfg(feature = "colors")]
pub const MOOT_STYLESHEET: MootStylesheet<anstyle::Style> = MootStylesheet {
    test_header: anstyle::Style::new()
        .bold()
        .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::BrightWhite))),
    block_header: anstyle::Style::new().dimmed(),
    remote: anstyle::Style::new()
        .dimmed()
        .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Cyan))),
    arrows: anstyle::Style::new().dimmed(),
    request: anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow))),
    response: anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::White))),
};

#[cfg(not(feature = "colors"))]
pub const MOOT_STYLESHEET: MootStylesheet<&str> = MootStylesheet {
    test_header: "",
    block_header: "",
    remote: "",
    arrows: "",
    request: "",
    response: "",
};
