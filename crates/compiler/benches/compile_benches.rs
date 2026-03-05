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

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use moor_compiler::{CompileOptions, compile};

fn tiny_program() -> &'static str {
    "return 1 + 2 * 3;"
}

fn medium_program() -> &'static str {
    r#"
total = 0;
for i in [1..100]
    if (i % 2 == 0)
        total = total + i;
    elseif (i % 3 == 0)
        total = total + (i * 2);
    else
        total = total + 1;
    endif
endfor
return total;
"#
}

fn large_program() -> String {
    let mut src = String::with_capacity(128 * 1024);
    src.push_str("total = 0;\n");
    for i in 0..3000 {
        src.push_str("if (");
        src.push_str(&(i % 17).to_string());
        src.push_str(" < 9)\n");
        src.push_str("  total = total + ");
        src.push_str(&(i % 11).to_string());
        src.push_str(";\n");
        src.push_str("else\n");
        src.push_str("  total = total + ");
        src.push_str(&(i % 7).to_string());
        src.push_str(";\n");
        src.push_str("endif\n");
    }
    src.push_str("return total;\n");
    src
}

fn compile_benches(c: &mut Criterion) {
    let tiny = tiny_program();
    let medium = medium_program();
    let large = large_program();

    let mut group = c.benchmark_group("compiler_compile");
    group.sample_size(30);

    group.throughput(Throughput::Bytes(tiny.len() as u64));
    group.bench_function("tiny", |b| {
        b.iter(|| {
            black_box(compile(black_box(tiny), CompileOptions::default()))
                .expect("tiny benchmark source should compile");
        });
    });

    group.throughput(Throughput::Bytes(medium.len() as u64));
    group.bench_function("medium", |b| {
        b.iter(|| {
            black_box(compile(black_box(medium), CompileOptions::default()))
                .expect("medium benchmark source should compile");
        });
    });

    group.throughput(Throughput::Bytes(large.len() as u64));
    group.bench_function("large_generated", |b| {
        b.iter(|| {
            black_box(compile(black_box(large.as_str()), CompileOptions::default()))
                .expect("large benchmark source should compile");
        });
    });

    group.finish();
}

criterion_group!(benches, compile_benches);
criterion_main!(benches);
