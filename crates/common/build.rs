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

use vergen::{CargoBuilder, Emitter};
use vergen_gitcl::GitclBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Emit cargo version info with idempotent mode to prevent constant rebuilds
    let cargo = CargoBuilder::default().build()?;

    Emitter::default()
        .idempotent()
        .add_instructions(&cargo)?
        .emit()?;

    // Emit git SHA with idempotent mode
    let gitcl = GitclBuilder::default().sha(true).build()?;

    vergen_gitcl::Emitter::default()
        .idempotent()
        .add_instructions(&gitcl)?
        .emit()?;

    Ok(())
}
