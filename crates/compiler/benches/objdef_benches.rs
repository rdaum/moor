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
use moor_compiler::{CompileOptions, ObjFileContext, compile_object_definitions};

fn small_objdef() -> &'static str {
    r#"
object #1
    parent: #1
    name: "Small Object"
    location: #2
    wizard: false
    programmer: false
    player: false
    fertile: true
    readable: true

    property description (owner: #1, flags: "rc") = "small";
    override description = "small updated";

    verb look_self (this none this) owner: #1 flags: "rxd"
        return this;
    endverb
endobject
"#
}

fn medium_objdef() -> String {
    let mut src = String::with_capacity(64 * 1024);
    src.push_str("define ROOT = #1;\n");
    src.push_str("define OWNER = #2;\n\n");

    for i in 1..=64 {
        src.push_str("object #");
        src.push_str(&i.to_string());
        src.push('\n');
        src.push_str("    parent: ROOT\n");
        src.push_str("    name: \"Medium ");
        src.push_str(&i.to_string());
        src.push_str("\"\n");
        src.push_str("    location: OWNER\n");
        src.push_str("    wizard: false\n");
        src.push_str("    programmer: false\n");
        src.push_str("    player: false\n");
        src.push_str("    fertile: true\n");
        src.push_str("    readable: true\n\n");

        src.push_str("    property p");
        src.push_str(&i.to_string());
        src.push_str(" (owner: OWNER, flags: \"rc\") = ");
        src.push_str(&i.to_string());
        src.push_str(";\n");
        src.push_str("    override p");
        src.push_str(&i.to_string());
        src.push_str(" = ");
        src.push_str(&(i + 1).to_string());
        src.push_str(";\n\n");

        src.push_str("    verb v");
        src.push_str(&i.to_string());
        src.push_str(" (this none this) owner: OWNER flags: \"rxd\"\n");
        src.push_str("        x = ");
        src.push_str(&(i % 9).to_string());
        src.push_str(";\n");
        src.push_str("        while (x < 50)\n");
        src.push_str("            x = x + 1;\n");
        src.push_str("        endwhile\n");
        src.push_str("        return x;\n");
        src.push_str("    endverb\n");
        src.push_str("endobject\n\n");
    }

    src
}

fn large_objdef() -> String {
    let mut src = String::with_capacity(512 * 1024);
    src.push_str("define ROOT = #1;\n");
    src.push_str("define OWNER = #2;\n");
    src.push_str("define PLACE = #3;\n\n");

    for i in 1..=200 {
        src.push_str("object #");
        src.push_str(&i.to_string());
        src.push('\n');
        src.push_str("    parent: ROOT\n");
        src.push_str("    name: \"Large ");
        src.push_str(&i.to_string());
        src.push_str("\"\n");
        src.push_str("    location: PLACE\n");
        src.push_str("    wizard: false\n");
        src.push_str("    programmer: false\n");
        src.push_str("    player: false\n");
        src.push_str("    fertile: true\n");
        src.push_str("    readable: true\n\n");

        for p in 0..3 {
            src.push_str("    property p");
            src.push_str(&i.to_string());
            src.push('_');
            src.push_str(&p.to_string());
            src.push_str(" (owner: OWNER, flags: \"rc\") = ");
            src.push_str(&(i + p).to_string());
            src.push_str(";\n");
        }

        for p in 0..3 {
            src.push_str("    override p");
            src.push_str(&i.to_string());
            src.push('_');
            src.push_str(&p.to_string());
            src.push_str(" = ");
            src.push_str(&(i + p + 10).to_string());
            src.push_str(";\n");
        }
        src.push('\n');

        for v in 0..2 {
            src.push_str("    verb v");
            src.push_str(&i.to_string());
            src.push('_');
            src.push_str(&v.to_string());
            src.push_str(" (this none this) owner: OWNER flags: \"rxd\"\n");
            src.push_str("        sum = 0;\n");
            src.push_str("        for n in [1..40]\n");
            src.push_str("            if (n % 2 == 0)\n");
            src.push_str("                sum = sum + n;\n");
            src.push_str("            else\n");
            src.push_str("                sum = sum + 1;\n");
            src.push_str("            endif\n");
            src.push_str("        endfor\n");
            src.push_str("        return sum;\n");
            src.push_str("    endverb\n\n");
        }
        src.push_str("endobject\n\n");
    }

    src
}

fn objdef_benches(c: &mut Criterion) {
    let options = CompileOptions::default();
    let small = small_objdef();
    let medium = medium_objdef();
    let large = large_objdef();

    let mut group = c.benchmark_group("compiler_objdef_compile");
    group.sample_size(20);

    group.throughput(Throughput::Bytes(small.len() as u64));
    group.bench_function("small", |b| {
        b.iter(|| {
            let mut context = ObjFileContext::new();
            black_box(compile_object_definitions(
                black_box(small),
                &options,
                &mut context,
            ))
            .expect("small objdef should compile");
        });
    });

    group.throughput(Throughput::Bytes(medium.len() as u64));
    group.bench_function("medium_generated", |b| {
        b.iter(|| {
            let mut context = ObjFileContext::new();
            black_box(compile_object_definitions(
                black_box(medium.as_str()),
                &options,
                &mut context,
            ))
            .expect("medium objdef should compile");
        });
    });

    group.throughput(Throughput::Bytes(large.len() as u64));
    group.bench_function("large_generated", |b| {
        b.iter(|| {
            let mut context = ObjFileContext::new();
            black_box(compile_object_definitions(
                black_box(large.as_str()),
                &options,
                &mut context,
            ))
            .expect("large objdef should compile");
        });
    });

    group.finish();
}

criterion_group!(benches, objdef_benches);
criterion_main!(benches);
