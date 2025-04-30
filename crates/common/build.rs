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

use shadow_rs::{BuildPattern, ShadowBuilder};

fn main() {
    let shadow = ShadowBuilder::builder()
        .build_pattern(BuildPattern::Lazy)
        .build()
        .unwrap();
    // Note:  If there are no rerun-if-changed directives, cargo helpfully rebuilds *every single time*
    //   despite ShadowBuilder not emitting anything new.
    println!("rerun-if-changed={}", shadow.out_path);
}
