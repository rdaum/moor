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

mod common;
use pretty_assertions::assert_eq;

// TODO: this test seems to run for a really really long time on MacOS, so we'll make it linux-only
//   for now.
#[cfg(target_os = "linux")]
#[test]
fn test_echo() -> eyre::Result<()> {
    common::run_test_as(&["wizard"], |mut client| {
        assert_eq!("42\n", client.command("; 42")?);
        Ok(())
    })
}
