// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! Reproducible codec CPU measurements, independent of storage and durability costs.
use moor_compiler::{
    SourceProfile, compile, program_to_tree, read_persistent_literal, read_persistent_source,
    to_literal, unparse, write_persistent_literal, write_persistent_source,
};
use moor_var::{program::ProgramType, v_int, v_list};
use std::{hint::black_box, time::Instant};

fn measure(mut operation: impl FnMut()) -> [f64; 3] {
    for _ in 0..20 {
        operation();
    }
    std::array::from_fn(|_| {
        let start = Instant::now();
        for _ in 0..1000 {
            operation();
        }
        start.elapsed().as_secs_f64() * 1_000_000.0 / 1000.0
    })
}

fn main() {
    let profile = SourceProfile::default();
    let values = [
        ("scalar", v_int(42)),
        (
            "list_1024",
            v_list(&(0..1024).map(v_int).collect::<Vec<_>>()),
        ),
        (
            "closure",
            read_persistent_literal(
                "{x, ?y = 5} => x + y + base with captured [{base: 40}]",
                &profile,
            )
            .unwrap(),
        ),
    ];
    println!("{{\"iterations_per_sample\":1000,\"unit\":\"us/op\",\"measurements\":[");
    for (name, value) in values {
        let text = to_literal(&value);
        let render = measure(|| {
            black_box(to_literal(black_box(&value)));
        });
        let decode = measure(|| {
            black_box(read_persistent_literal(black_box(&text), &profile).unwrap());
        });
        let encode = measure(|| {
            let mut text = String::new();
            write_persistent_literal(black_box(&value), &profile, &mut text).unwrap();
            black_box(text);
        });
        println!(
            "{{\"case\":\"{name}\",\"bytes\":{},\"render\":{render:?},\"decode\":{decode:?},\"validated_encode\":{encode:?}}},",
            text.len()
        );
    }
    let program = compile(
        "let base = 40; let f = {x, ?y = 5} => base + x + y; return {f(2), f(2, 8)};",
        profile.options.clone(),
    )
    .unwrap();
    let mut text = String::new();
    let stored = ProgramType::MooR(program.clone());
    write_persistent_source(&stored, &profile, &mut text).unwrap();
    let render = measure(|| {
        black_box(
            unparse(&program_to_tree(black_box(&program)).unwrap(), false, true)
                .unwrap()
                .join("\n"),
        );
    });
    let decode = measure(|| {
        black_box(read_persistent_source(black_box(&text), &profile).unwrap());
    });
    let encode = measure(|| {
        let mut text = String::new();
        write_persistent_source(black_box(&stored), &profile, &mut text).unwrap();
        black_box(text);
    });
    println!(
        "{{\"case\":\"program\",\"bytes\":{},\"render\":{render:?},\"decode\":{decode:?},\"validated_encode\":{encode:?}}}]}}",
        text.len()
    );
}
