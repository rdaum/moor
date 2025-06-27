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

#[cfg(target_os = "linux")]
mod bench_code {
    use moor_var::{IndexMode, Var, v_int, v_list};
    #[cfg(target_os = "linux")]
    use perf_event::Builder;
    #[cfg(target_os = "linux")]
    use perf_event::events::Hardware;
    use std::hint::black_box;
    use std::time::Duration;
    const CHUNK_SIZE: usize = 100_000;

    struct Results {
        instructions: u64,
        branches: u64,
        branch_misses: u64,
        cache_misses: u64,
        duration: Duration,
        iterations: u64,
    }

    /// Probe to see how many iterations we can do in 5 seconds
    #[cfg(target_os = "linux")]
    fn probe<F: Fn(&Var, usize, usize)>(f: &F, prepared: &Var, chunk_size: usize) -> usize {
        let start_time = minstant::Instant::now();
        let mut num_chunks = 0;
        loop {
            num_chunks += 1;
            black_box(|| f(prepared, chunk_size, num_chunks))();
            if start_time.elapsed() >= Duration::from_secs(5) {
                break;
            }
        }
        num_chunks * chunk_size
    }

    /// Benchmarks a function and returns the results
    #[cfg(target_os = "linux")]
    fn bench<F: Fn(&Var, usize, usize)>(
        f: &F,
        prepared: &Var,
        chunk_size: usize,
        iterations: usize,
    ) -> Results {
        let start_time = minstant::Instant::now();
        let mut instructions_counter = Builder::new(Hardware::INSTRUCTIONS).build().unwrap();
        let mut branch_counter = Builder::new(Hardware::BRANCH_INSTRUCTIONS).build().unwrap();
        let mut branch_misses = Builder::new(Hardware::BRANCH_MISSES).build().unwrap();
        let mut cache_misses = Builder::new(Hardware::CACHE_MISSES).build().unwrap();

        instructions_counter.enable().unwrap();
        branch_counter.enable().unwrap();
        branch_misses.enable().unwrap();
        cache_misses.enable().unwrap();

        for c in 0..iterations / chunk_size {
            black_box(|| f(prepared, chunk_size, c))();
        }

        instructions_counter.disable().unwrap();
        branch_counter.disable().unwrap();
        branch_misses.disable().unwrap();
        cache_misses.disable().unwrap();

        Results {
            instructions: instructions_counter.read().unwrap(),
            branches: branch_counter.read().unwrap(),
            branch_misses: branch_misses.read().unwrap(),
            cache_misses: cache_misses.read().unwrap(),
            duration: start_time.elapsed(),
            iterations: 0,
        }
    }

    /// Benchmark for addition on integers
    #[cfg(target_os = "linux")]
    pub fn op_bench<F: Fn(&Var, usize, usize)>(name: &str, f: F, prepared: Var) {
        eprintln!("Probing .... {name}");
        let probed_iterations = probe(&f, &prepared, CHUNK_SIZE);
        eprintln!("Probed iterations: {probed_iterations}");

        eprintln!("Running .... {name}");
        let mut summed_results = Results {
            instructions: 0,
            branches: 0,
            branch_misses: 0,
            cache_misses: 0,
            duration: Duration::ZERO,
            iterations: probed_iterations as u64,
        };
        for i in 0..5 {
            eprint!("{}...", i + 1);
            let results = bench(&f, &prepared, CHUNK_SIZE, probed_iterations);
            summed_results.instructions += results.instructions;
            summed_results.branches += results.branches;
            summed_results.branch_misses += results.branch_misses;
            summed_results.cache_misses += results.cache_misses;
            summed_results.duration += results.duration;
            summed_results.iterations += results.iterations;
        }
        eprintln!();

        // Calculate averages
        let results = Results {
            instructions: summed_results.instructions / 10,
            branches: summed_results.branches / 10,
            branch_misses: summed_results.branch_misses / 10,
            cache_misses: summed_results.cache_misses / 10,
            duration: summed_results.duration / 10,
            iterations: summed_results.iterations / 10,
        };

        eprintln!("Results for {name}:");
        eprintln!("  Iterations: {}", results.iterations);
        eprintln!("  Instructions: {}", results.instructions);
        eprintln!("  Branches: {}", results.branches);
        eprintln!("  Branch misses: {}", results.branch_misses);
        eprintln!("  Cache misses: {}", results.cache_misses);
        eprintln!("  Duration: {:?}", results.duration);
        eprintln!(
            "  Throughput: {:.2} Mops/s",
            results.iterations as f64 / results.duration.as_secs_f64() / 1_000_000.0
        );
    }

    pub(crate) fn prepare_int() -> Var {
        v_int(0)
    }
    pub(crate) fn int_add(v: &Var, chunk_size: usize, _chunk_num: usize) {
        let mut v = v.clone();
        for _ in 0..chunk_size {
            v = v.add(&v_int(1)).unwrap();
        }
    }

    pub(crate) fn int_eq(v: &Var, chunk_size: usize, _chunk_num: usize) {
        let v = v.clone();
        for _ in 0..chunk_size {
            let _ = v.eq(&v);
        }
    }

    pub(crate) fn int_cmp(v: &Var, chunk_size: usize, _chunk_num: usize) {
        let v = v.clone();
        for _ in 0..chunk_size {
            let _ = v.cmp(&v);
        }
    }

    pub(crate) fn prepare_small_list() -> Var {
        v_list(&[v_int(0)])
    }

    pub(crate) fn list_push(v: &Var, chunk_size: usize, _chunk_num: usize) {
        let mut v = v.clone();
        for _ in 0..chunk_size {
            v = v.push(&v_int(1)).unwrap();
        }
    }

    pub(crate) fn prepare_large_list() -> Var {
        // Make a quite large list
        v_list(&(0..100_000).map(v_int).collect::<Vec<_>>())
    }

    pub(crate) fn list_index_pos(v: &Var, chunk_size: usize, _chunk_num: usize) {
        let v = v.clone();
        for c in 0..chunk_size {
            let _ = v.index(&v_int(c as i64), IndexMode::ZeroBased).unwrap();
        }
    }

    pub(crate) fn list_index_assign(v: &Var, chunk_size: usize, _chunk_num: usize) {
        let mut v = v.clone();
        for c in 0..chunk_size {
            v = v
                .index_set(&v_int(c as i64), &v_int(c as i64), IndexMode::ZeroBased)
                .unwrap();
        }
    }
}

// Linux only...
#[cfg(target_os = "linux")]
pub fn main() {
    use crate::bench_code::{
        int_add, int_cmp, int_eq, list_index_assign, list_index_pos, list_push, op_bench,
        prepare_int, prepare_large_list, prepare_small_list,
    };
    #[cfg(target_os = "linux")]
    use perf_event::Builder;
    #[cfg(target_os = "linux")]
    use perf_event::events::Hardware;

    // Check if we can do perf events, and if not just exit early (wthout panic) so that test runners etc
    // don't fail.
    if Builder::new(Hardware::INSTRUCTIONS).build().is_err() {
        eprintln!("Perf events are not supported on this system. Skipping benchmarks.");
        return;
    }

    op_bench("int_add", int_add, prepare_int());
    op_bench("int_eq", int_eq, prepare_int());
    op_bench("int_cmp", int_cmp, prepare_int());
    op_bench("list_push", list_push, prepare_small_list());
    op_bench("list_index_pos", list_index_pos, prepare_large_list());
    op_bench("list_index_assign", list_index_assign, prepare_large_list());
}

// Non-linux platforms will not run the benchmarks
#[cfg(not(target_os = "linux"))]
pub fn main() {
    eprintln!("Var micro-benchmarks are only supported on Linux due to perf_event usage.");
}
