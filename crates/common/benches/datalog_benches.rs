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

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use moor_common::datalog::{Atom, Datalog, Rule, Term}; // Removed Fact import
use moor_var::{Symbol, v_int, v_str, v_string};
use std::hint::black_box;

fn create_ancestor_datalog() -> (Datalog, Atom) {
    let mut dl = Datalog::new();

    // Add parent facts
    for i in 0..100 {
        dl.add_fact(
            // Changed
            Symbol::from("parent"),
            vec![v_int(i), v_int(i + 1)],
        );
    }

    // Rule: ancestor(X, Y) :- parent(X, Y)
    let x = dl.new_variable("X");
    let y = dl.new_variable("Y");
    let parent_atom = Atom::new(
        Symbol::from("parent"),
        vec![Term::Variable(x.clone()), Term::Variable(y.clone())],
    );
    let ancestor_atom = Atom::new(
        Symbol::from("ancestor"),
        vec![Term::Variable(x.clone()), Term::Variable(y.clone())],
    );
    dl.add_rule(Rule::new(ancestor_atom, vec![parent_atom]));

    // Rule: ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z)
    let x = dl.new_variable("X");
    let y = dl.new_variable("Y");
    let z = dl.new_variable("Z");
    let parent_atom = Atom::new(
        Symbol::from("parent"),
        vec![Term::Variable(x.clone()), Term::Variable(y.clone())],
    );
    let ancestor_atom_body = Atom::new(
        Symbol::from("ancestor"),
        vec![Term::Variable(y.clone()), Term::Variable(z.clone())],
    );
    let ancestor_atom_head = Atom::new(
        Symbol::from("ancestor"),
        vec![Term::Variable(x.clone()), Term::Variable(z.clone())],
    );
    dl.add_rule(Rule::new(
        ancestor_atom_head,
        vec![parent_atom, ancestor_atom_body],
    ));

    // Query: ancestor(0, X)
    let query = Atom::new(
        Symbol::from("ancestor"),
        vec![
            Term::Constant(v_int(0)),
            Term::Variable(dl.new_variable("X")),
        ],
    );

    (dl, query)
}

fn create_path_datalog(size: usize) -> (Datalog, Atom) {
    let mut dl = Datalog::new();

    // Create a graph with 'size' nodes
    let size_i64 = size as i64; // Use a different variable name to avoid conflict
    for i in 0..size_i64 {
        // Add an edge to the next node
        dl.add_fact(
            // Changed
            Symbol::from("edge"),
            vec![v_int(i), v_int((i + 1) % size_i64)],
        );

        // Add some random edges to create a more complex graph
        if i % 5 == 0 && i + 10 < size_i64 {
            dl.add_fact(
                // Changed
                Symbol::from("edge"),
                vec![v_int(i), v_int(i + 10)],
            );
        }
        if i % 7 == 0 && i + 20 < size_i64 {
            dl.add_fact(
                // Changed
                Symbol::from("edge"),
                vec![v_int(i), v_int(i + 20)],
            );
        }
    }

    // Rule: path(X, Y) :- edge(X, Y)
    let x = dl.new_variable("X");
    let y = dl.new_variable("Y");
    let edge_atom = Atom::new(
        Symbol::from("edge"),
        vec![Term::Variable(x.clone()), Term::Variable(y.clone())],
    );
    let path_atom = Atom::new(
        Symbol::from("path"),
        vec![Term::Variable(x.clone()), Term::Variable(y.clone())],
    );
    dl.add_rule(Rule::new(path_atom, vec![edge_atom]));

    // Rule: path(X, Z) :- edge(X, Y), path(Y, Z)
    let x = dl.new_variable("X");
    let y = dl.new_variable("Y");
    let z = dl.new_variable("Z");
    let edge_atom = Atom::new(
        Symbol::from("edge"),
        vec![Term::Variable(x.clone()), Term::Variable(y.clone())],
    );
    let path_atom_body = Atom::new(
        Symbol::from("path"),
        vec![Term::Variable(y.clone()), Term::Variable(z.clone())],
    );
    let path_atom_head = Atom::new(
        Symbol::from("path"),
        vec![Term::Variable(x.clone()), Term::Variable(z.clone())],
    );
    dl.add_rule(Rule::new(path_atom_head, vec![edge_atom, path_atom_body]));

    // Query: path(0, size/2)
    let query = Atom::new(
        Symbol::from("path"),
        vec![
            Term::Constant(v_int(0)),
            Term::Constant(v_int(size_i64 / 2)),
        ],
    );

    (dl, query)
}

fn create_adventure_game_datalog(size: usize) -> (Datalog, Atom) {
    let mut dl = Datalog::new();

    // Create a grid-like world with 'size' total rooms
    let grid_size = (size as f64).sqrt().ceil() as usize;

    // Add rooms and connections
    for i in 0..grid_size {
        for j in 0..grid_size {
            let room_id = i * grid_size + j;
            if room_id >= size {
                break;
            }

            let room_name = format!("room_{}", room_id);

            // Connect to room to the right
            if j + 1 < grid_size && room_id + 1 < size {
                dl.add_fact(
                    // Changed
                    Symbol::from("connection"),
                    vec![
                        v_string(room_name.clone()),
                        v_string(format!("room_{}", room_id + 1)),
                        v_int(0),
                    ],
                );
            }

            // Connect to room below
            if i + 1 < grid_size && room_id + grid_size < size {
                dl.add_fact(
                    // Changed
                    Symbol::from("connection"),
                    vec![
                        v_string(room_name.clone()),
                        v_string(format!("room_{}", room_id + grid_size)),
                        v_int(0),
                    ],
                );
            }

            // Add some locked doors
            if room_id % 11 == 0 && room_id + grid_size + 1 < size {
                dl.add_fact(
                    // Changed
                    Symbol::from("connection"),
                    vec![
                        v_string(room_name.clone()),
                        v_string(format!("room_{}", room_id + grid_size + 1)),
                        v_int(1),
                    ],
                );
            }

            // Add some items to rooms
            if room_id % 5 == 0 {
                dl.add_fact(
                    // Changed
                    Symbol::from("contains"),
                    vec![
                        v_string(room_name.clone()),
                        v_string(format!("item_{}", room_id / 5)),
                    ],
                );
            }

            // Add some keys to certain rooms
            if room_id % 13 == 0 {
                dl.add_fact(
                    // Changed
                    Symbol::from("contains"),
                    vec![
                        v_string(room_name),
                        v_string(format!("key_{}", room_id / 13)),
                    ],
                );
            }
        }
    }

    // Add player facts
    dl.add_fact(Symbol::from("player"), vec![v_str("player_1")]); // Changed

    // Add key unlocking facts
    for i in 0..size / 13 {
        for j in 0..size / 11 {
            let room1 = format!("room_{}", j * 11);
            let room2 = format!("room_{}", j * 11 + grid_size + 1);
            if j * 11 + grid_size + 1 < size {
                dl.add_fact(
                    // Changed
                    Symbol::from("unlocks"),
                    vec![
                        v_string(format!("key_{}", i)),
                        v_string(room1),
                        v_string(room2),
                    ],
                );
            }
        }
    }

    // Give player some keys
    for i in 0..size / 26 {
        dl.add_fact(
            // Changed
            Symbol::from("has_item"),
            vec![v_str("player_1"), v_string(format!("key_{}", i))],
        );
    }

    // Rules for room access
    // can_access(Player, From, To) :- connection(From, To, 0)
    let player1 = dl.new_variable("Player");
    let from1 = dl.new_variable("From");
    let to1 = dl.new_variable("To");
    let unlocked_path_atom = Atom::new(
        Symbol::from("connection"),
        vec![
            Term::Variable(from1.clone()),
            Term::Variable(to1.clone()),
            Term::Constant(v_int(0)),
        ],
    );
    let can_access_atom = Atom::new(
        Symbol::from("can_access"),
        vec![
            Term::Variable(player1.clone()),
            Term::Variable(from1.clone()),
            Term::Variable(to1.clone()),
        ],
    );
    dl.add_rule(Rule::new(can_access_atom, vec![unlocked_path_atom]));

    // can_access(Player, From, To) :- connection(From, To, 1), has_item(Player, Key), unlocks(Key, From, To)
    let player2 = dl.new_variable("Player");
    let from2 = dl.new_variable("From");
    let to2 = dl.new_variable("To");
    let key2 = dl.new_variable("Key");

    let locked_path_atom = Atom::new(
        Symbol::from("connection"),
        vec![
            Term::Variable(from2.clone()),
            Term::Variable(to2.clone()),
            Term::Constant(v_int(1)),
        ],
    );
    let has_item_atom = Atom::new(
        Symbol::from("has_item"),
        vec![
            Term::Variable(player2.clone()),
            Term::Variable(key2.clone()),
        ],
    );
    let unlocks_atom = Atom::new(
        Symbol::from("unlocks"),
        vec![
            Term::Variable(key2.clone()),
            Term::Variable(from2.clone()),
            Term::Variable(to2.clone()),
        ],
    );
    let can_access_locked_atom = Atom::new(
        Symbol::from("can_access"),
        vec![
            Term::Variable(player2.clone()),
            Term::Variable(from2.clone()),
            Term::Variable(to2.clone()),
        ],
    );
    dl.add_rule(Rule::new(
        can_access_locked_atom,
        vec![locked_path_atom, has_item_atom, unlocks_atom],
    ));

    // path(Player, X, Y) :- can_access(Player, X, Y)
    let player3 = dl.new_variable("Player");
    let x3 = dl.new_variable("X");
    let y3 = dl.new_variable("Y");
    let can_access_atom = Atom::new(
        Symbol::from("can_access"),
        vec![
            Term::Variable(player3.clone()),
            Term::Variable(x3.clone()),
            Term::Variable(y3.clone()),
        ],
    );
    let path_atom = Atom::new(
        Symbol::from("path"),
        vec![
            Term::Variable(player3.clone()),
            Term::Variable(x3.clone()),
            Term::Variable(y3.clone()),
        ],
    );
    dl.add_rule(Rule::new(path_atom, vec![can_access_atom]));

    // path(Player, X, Z) :- can_access(Player, X, Y), path(Player, Y, Z)
    let player4 = dl.new_variable("Player");
    let x4 = dl.new_variable("X");
    let y4 = dl.new_variable("Y");
    let z4 = dl.new_variable("Z");

    let can_access_atom = Atom::new(
        Symbol::from("can_access"),
        vec![
            Term::Variable(player4.clone()),
            Term::Variable(x4.clone()),
            Term::Variable(y4.clone()),
        ],
    );
    let path_atom_body = Atom::new(
        Symbol::from("path"),
        vec![
            Term::Variable(player4.clone()),
            Term::Variable(y4.clone()),
            Term::Variable(z4.clone()),
        ],
    );
    let path_atom_head = Atom::new(
        Symbol::from("path"),
        vec![
            Term::Variable(player4.clone()),
            Term::Variable(x4.clone()),
            Term::Variable(z4.clone()),
        ],
    );
    dl.add_rule(Rule::new(
        path_atom_head,
        vec![can_access_atom, path_atom_body],
    ));

    // can_find(Player, Item) :- path(Player, Start, Room), contains(Room, Item)
    let player5 = dl.new_variable("Player");
    let room5 = dl.new_variable("Room");
    let item5 = dl.new_variable("Item");
    let start5 = dl.new_variable("Start");

    let path_atom = Atom::new(
        Symbol::from("path"),
        vec![
            Term::Variable(player5.clone()),
            Term::Variable(start5.clone()),
            Term::Variable(room5.clone()),
        ],
    );
    let contains_atom = Atom::new(
        Symbol::from("contains"),
        vec![Term::Variable(room5.clone()), Term::Variable(item5.clone())],
    );
    let can_find_atom = Atom::new(
        Symbol::from("can_find"),
        vec![
            Term::Variable(player5.clone()),
            Term::Variable(item5.clone()),
        ],
    );
    dl.add_rule(Rule::new(can_find_atom, vec![path_atom, contains_atom]));

    // Query: can_find(player_1, item_X) - ask about a specific item
    let target_item = format!("item_{}", size / 10);
    let query = Atom::new(
        Symbol::from("can_find"),
        vec![
            Term::Constant(v_str("player_1")),
            Term::Constant(v_string(target_item)),
        ],
    );

    (dl, query)
}

fn benchmark_full_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_query");
    group.sample_size(10);
    // Ancestor benchmark with different sizes
    for size in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::new("ancestor", size), size, |b, &_size| {
            let (mut dl, query) = create_ancestor_datalog();
            b.iter(|| dl.query(black_box(&query)));
        });
    }

    // Path benchmark with different sizes
    for size in [20, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::new("path", size), size, |b, &size| {
            let (mut dl, query) = create_path_datalog(size as usize);
            b.iter(|| dl.query(black_box(&query)));
        });
    }

    // Adventure game benchmark with different sizes
    for size in [25, 100, 225].iter() {
        group.bench_with_input(
            BenchmarkId::new("adventure_game", size),
            size,
            |b, &size| {
                let (mut dl, query) = create_adventure_game_datalog(size as usize);
                b.iter(|| dl.query(black_box(&query)));
            },
        );
    }

    group.finish();
}

fn benchmark_incremental_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_query");
    group.sample_size(10);

    // Ancestor benchmark with different sizes
    for size in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("ancestor_init", size),
            size,
            |b, &_size| {
                let (mut dl, _query) = create_ancestor_datalog();
                b.iter(|| {
                    black_box(dl.query_incremental_init());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("ancestor_step", size),
            size,
            |b, &_size| {
                let (mut dl, _query) = create_ancestor_datalog();
                dl.query_incremental_init();
                // Measure the time for each step
                b.iter(|| {
                    black_box(dl.step_evaluation());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("ancestor_complete", size),
            size,
            |b, &_size| {
                let (mut dl, _query) = create_ancestor_datalog();
                dl.query_incremental_init();
                b.iter(|| {
                    black_box(dl.complete_evaluation());
                });
            },
        );
    }

    // Path benchmark with different sizes
    for size in [20, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::new("path_init", size), size, |b, &size| {
            let (mut dl, _query) = create_path_datalog(size as usize);
            b.iter(|| {
                black_box(dl.query_incremental_init());
            });
        });

        group.bench_with_input(BenchmarkId::new("path_step", size), size, |b, &size| {
            let (mut dl, _query) = create_path_datalog(size as usize);
            dl.query_incremental_init();
            b.iter(|| {
                black_box(dl.step_evaluation());
            });
        });

        group.bench_with_input(BenchmarkId::new("path_complete", size), size, |b, &size| {
            let (mut dl, _query) = create_path_datalog(size as usize);
            dl.query_incremental_init();
            b.iter(|| {
                black_box(dl.complete_evaluation());
            });
        });
    }

    // Adventure game benchmark with different sizes
    for size in [25, 100, 225].iter() {
        group.bench_with_input(
            BenchmarkId::new("adventure_game_init", size),
            size,
            |b, &size| {
                let (mut dl, _query) = create_adventure_game_datalog(size as usize);
                b.iter(|| {
                    black_box(dl.query_incremental_init());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("adventure_game_step", size),
            size,
            |b, &size| {
                let (mut dl, _query) = create_adventure_game_datalog(size as usize);
                dl.query_incremental_init();
                b.iter(|| {
                    black_box(dl.step_evaluation());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("adventure_game_complete", size),
            size,
            |b, &size| {
                let (mut dl, _query) = create_adventure_game_datalog(size as usize);
                dl.query_incremental_init();
                b.iter(|| {
                    black_box(dl.complete_evaluation());
                });
            },
        );
    }

    group.finish();
}

fn benchmark_incremental_vs_complete(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_vs_complete");
    group.sample_size(10);

    // Measure time to find first result for incremental evaluation vs complete evaluation
    for size in [50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::new("path_first_result_incremental", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let (mut dl, query) = create_path_datalog(size as usize);
                    dl.query_incremental_init();
                    let mut found = false;
                    let mut steps = 0;

                    while !found && steps < 1000 {
                        dl.step_evaluation();
                        steps += 1;

                        // Check if we have results
                        if !dl.query_incremental_results(&query).is_empty() {
                            found = true;
                        }
                    }
                    black_box(found)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("path_first_result_complete", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let (mut dl, query) = create_path_datalog(size as usize);
                    black_box(dl.query(&query).len() > 0)
                });
            },
        );
    }

    group.finish();
}

fn benchmark_step_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("step_overhead");
    group.sample_size(10);

    // This benchmark measures the overhead of the incremental evaluation framework
    // by comparing the total time for all steps vs. the time for a complete evaluation
    for size in [20, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("path_all_steps", size),
            size,
            |b, &size| {
                let (mut dl, _query) = create_path_datalog(size as usize);
                dl.query_incremental_init();

                b.iter(|| {
                    let (mut dl, _query) = create_path_datalog(size as usize);
                    dl.query_incremental_init();
                    let mut steps = 0;

                    while dl.step_evaluation() {
                        steps += 1;
                    }
                    black_box(steps)
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("path_complete", size), size, |b, &size| {
            b.iter(|| {
                let (mut dl, query) = create_path_datalog(size as usize);
                black_box(dl.query(&query))
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_full_query,
    benchmark_incremental_query,
    benchmark_incremental_vs_complete,
    benchmark_step_overhead
);
criterion_main!(benches);
