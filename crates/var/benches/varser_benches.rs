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

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use moor_var::{
    AsByteBuffer, ErrorCode, Obj, Symbol, Var, flex, v_binary, v_bool, v_error, v_float, v_int,
    v_list, v_map, v_none, v_obj, v_str, v_sym,
};
use std::f64::consts::PI;
use std::time::Duration;

/// Generate test data for all MOO data types
fn generate_test_data() -> Vec<(String, Var)> {
    vec![
        // Simple primitive types
        ("none".to_string(), v_none()),
        ("bool_true".to_string(), v_bool(true)),
        ("int_large".to_string(), v_int(9999999)),
        ("float_large".to_string(), v_float(1e6)),
        ("str_empty".to_string(), v_str("")),
        ("str_small".to_string(), v_str("hello world")),
        ("str_large".to_string(), v_str(&"x".repeat(1000))),
        // Complex types
        ("obj".to_string(), v_obj(Obj::mk_id(1234))),
        ("sym".to_string(), v_sym(Symbol::mk("test_symbol"))),
        (
            "binary_small".to_string(),
            v_binary(vec![0x01, 0x02, 0x03, 0x04]),
        ),
        ("binary_large".to_string(), v_binary(vec![0xFF; 1000])),
        // Error value
        (
            "error".to_string(),
            v_error(ErrorCode::E_TYPE.with_msg(|| "test error".to_string())),
        ),
        // Collections
        ("list_empty".to_string(), v_list(&[])),
        (
            "list_small".to_string(),
            v_list(&[v_int(1), v_str("test"), v_bool(true)]),
        ),
        (
            "list_large".to_string(),
            v_list(&(0..100).map(v_int).collect::<Vec<_>>()),
        ),
        // Nested collections
        (
            "list_nested".to_string(),
            v_list(&[
                v_int(1),
                v_list(&[v_str("nested"), v_bool(true)]),
                v_list(&[v_int(42), v_list(&[v_float(PI)])]),
            ]),
        ),
        // Maps
        ("map_empty".to_string(), v_map(&[])),
        (
            "map_small".to_string(),
            v_map(&[
                (v_str("key1"), v_int(1)),
                (v_str("key2"), v_bool(true)),
                (v_str("key3"), v_str("value")),
            ]),
        ),
        // Nested maps
        (
            "map_nested".to_string(),
            v_map(&[
                (
                    v_str("level1"),
                    v_map(&[(v_str("level2"), v_map(&[(v_str("level3"), v_int(42))]))]),
                ),
                (v_str("list"), v_list(&[v_int(1), v_str("nested")])),
                (v_int(42), v_map(&[(v_bool(true), v_str("value"))])),
            ]),
        ),
        // Mixed deeply nested structure
        (
            "complex_nested".to_string(),
            v_list(&[
                v_map(&[
                    (v_str("name"), v_str("test")),
                    (
                        v_str("values"),
                        v_list(&[
                            v_int(1),
                            v_float(2.5),
                            v_bool(true),
                            v_map(&[(v_str("inner"), v_str("value"))]),
                        ]),
                    ),
                ]),
                v_list(&[
                    v_obj(Obj::mk_id(123)),
                    v_sym(Symbol::mk("symbol")),
                    v_binary(vec![0x01, 0x02, 0x03]),
                ]),
            ]),
        ),
    ]
}

/// Benchmark serialization performance for each data type, comparing bincode vs flexbuffer
fn bench_serialization_comparison(c: &mut Criterion) {
    let test_data = generate_test_data();

    for (name, var) in &test_data {
        let mut group = c.benchmark_group(format!("serialization_{}", name));
        group.sample_size(100);
        group.measurement_time(Duration::from_secs(5));

        // Benchmark bincode serialization
        let bincode_size = var.size_bytes() as u64;
        group.throughput(Throughput::Bytes(bincode_size));
        group.bench_function("bincode", |b| b.iter(|| var.make_copy_as_vec().unwrap()));

        // Benchmark flexbuffer serialization
        let flexbuffer_sample = flex::var_to_flexbuffer(var);
        group.throughput(Throughput::Bytes(flexbuffer_sample.len() as u64));
        group.bench_function("flexbuffer", |b| b.iter(|| flex::var_to_flexbuffer(var)));

        group.finish();
    }
}

/// Benchmark deserialization performance for each data type, comparing bincode vs flexbuffer
fn bench_deserialization_comparison(c: &mut Criterion) {
    let test_data = generate_test_data();

    for (name, var) in &test_data {
        let mut group = c.benchmark_group(format!("deserialization_{}", name));
        group.sample_size(100);
        group.measurement_time(Duration::from_secs(5));

        // Prepare serialized data
        let bincode_data = var.make_copy_as_vec().unwrap();
        let flexbuffer_data = flex::var_to_flexbuffer(var);

        // Benchmark bincode deserialization
        group.throughput(Throughput::Bytes(bincode_data.len() as u64));
        group.bench_function("bincode", |b| {
            b.iter(|| Var::from_bytes(byteview::ByteView::from(bincode_data.clone())).unwrap())
        });

        // Benchmark flexbuffer deserialization
        group.throughput(Throughput::Bytes(flexbuffer_data.len() as u64));
        group.bench_function("flexbuffer", |b| {
            b.iter(|| flex::var_from_flexbuffer(&flexbuffer_data).unwrap())
        });

        group.finish();
    }
}

criterion_group!(
    benches,
    bench_serialization_comparison,
    bench_deserialization_comparison,
);

criterion_main!(benches);
