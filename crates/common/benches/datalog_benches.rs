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
use moor_common::datalog::Term::{Constant, Variable};
use moor_common::datalog::{Atom, KnowledgeBase, Rule}; // Removed Fact import
use moor_var::{Symbol, v_int, v_str, v_string};
use std::hint::black_box;

// Lazy init a bunch of symbols we'll use ("parent", "X", *Y, etc.), just so we're not really
// measuring the time it takes to create these symbols in the benchmarks.
lazy_static::lazy_static! {
    static ref PARENT: Symbol = Symbol::from("parent");
    static ref ANCESTOR: Symbol = Symbol::from("ancestor");
    static ref EDGE: Symbol = Symbol::from("edge");
    static ref PATH: Symbol = Symbol::from("path");
    static ref CONNECTION: Symbol = Symbol::from("connection");
    static ref CONTAINS: Symbol = Symbol::from("contains");
    static ref UNLOCKS: Symbol = Symbol::from("unlocks");
    static ref HAS_ITEM: Symbol = Symbol::from("has_item");
    static ref CAN_ACCESS: Symbol = Symbol::from("can_access");
    static ref CAN_FIND: Symbol = Symbol::from("can_find");
    static ref X: Symbol = Symbol::from("X");
    static ref Y: Symbol = Symbol::from("Y");
    static ref Z: Symbol = Symbol::from("Y");
    static ref PLAYER: Symbol = Symbol::from("player");
    static ref FROM: Symbol = Symbol::from("From");
    static ref TO: Symbol = Symbol::from("To");
    static ref KEY: Symbol = Symbol::from("Key");
    static ref ROOM: Symbol = Symbol::from("Room");
    static ref ITEM: Symbol = Symbol::from("Item");
    static ref START: Symbol = Symbol::from("Start");
}

fn create_ancestor_datalog() -> (KnowledgeBase, Atom) {
    let mut dl = KnowledgeBase::new();

    // Add parent facts
    for i in 0..100 {
        dl.add_fact(
            // Changed
            *PARENT,
            vec![v_int(i), v_int(i + 1)],
        );
    }

    // Rule: ancestor(X, Y) :- parent(X, Y)
    let x = dl.new_variable(*X);
    let y = dl.new_variable(*Y);
    let parent_atom = Atom::new(*PARENT, vec![Variable(x.clone()), Variable(y.clone())]);
    let ancestor_atom = Atom::new(*ANCESTOR, vec![Variable(x.clone()), Variable(y.clone())]);
    dl.add_rule(Rule::new(ancestor_atom, vec![parent_atom]));

    // Rule: ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z)
    let x = dl.new_variable(*X);
    let y = dl.new_variable(*Y);
    let z = dl.new_variable(*Z);
    let parent_atom = Atom::new(*PARENT, vec![Variable(x.clone()), Variable(y.clone())]);
    let ancestor_atom_body = Atom::new(*ANCESTOR, vec![Variable(y.clone()), Variable(z.clone())]);
    let ancestor_atom_head = Atom::new(*ANCESTOR, vec![Variable(x.clone()), Variable(z.clone())]);
    dl.add_rule(Rule::new(
        ancestor_atom_head,
        vec![parent_atom, ancestor_atom_body],
    ));

    // Query: ancestor(0, X)
    let query = Atom::new(
        *ANCESTOR,
        vec![Constant(v_int(0)), Variable(dl.new_variable(*X))],
    );

    (dl, query)
}

fn create_path_datalog(size: usize) -> (KnowledgeBase, Atom) {
    let mut dl = KnowledgeBase::new();

    // Create a graph with 'size' nodes
    let size_i64 = size as i64; // Use a different variable name to avoid conflict
    for i in 0..size_i64 {
        // Add an edge to the next node
        dl.add_fact(
            // Changed
            *EDGE,
            vec![v_int(i), v_int((i + 1) % size_i64)],
        );

        // Add some random edges to create a more complex graph
        if i % 5 == 0 && i + 10 < size_i64 {
            dl.add_fact(
                // Changed
                *EDGE,
                vec![v_int(i), v_int(i + 10)],
            );
        }
        if i % 7 == 0 && i + 20 < size_i64 {
            dl.add_fact(
                // Changed
                *EDGE,
                vec![v_int(i), v_int(i + 20)],
            );
        }
    }

    // Rule: path(X, Y) :- edge(X, Y)
    let x = dl.new_variable(*X);
    let y = dl.new_variable(*Y);
    let edge_atom = Atom::new(*EDGE, vec![Variable(x.clone()), Variable(y.clone())]);
    let path_atom = Atom::new(*PATH, vec![Variable(x.clone()), Variable(y.clone())]);
    dl.add_rule(Rule::new(path_atom, vec![edge_atom]));

    // Rule: path(X, Z) :- edge(X, Y), path(Y, Z)
    let x = dl.new_variable(*X);
    let y = dl.new_variable(*Y);
    let z = dl.new_variable(*Z);
    let edge_atom = Atom::new(*EDGE, vec![Variable(x.clone()), Variable(y.clone())]);
    let path_atom_body = Atom::new(*PATH, vec![Variable(y.clone()), Variable(z.clone())]);
    let path_atom_head = Atom::new(*PATH, vec![Variable(x.clone()), Variable(z.clone())]);
    dl.add_rule(Rule::new(path_atom_head, vec![edge_atom, path_atom_body]));

    // Query: path(0, size/2)
    let query = Atom::new(
        *PATH,
        vec![Constant(v_int(0)), Constant(v_int(size_i64 / 2))],
    );

    (dl, query)
}

fn create_adventure_game_datalog(size: usize) -> (KnowledgeBase, Atom) {
    let mut dl = KnowledgeBase::new();

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
                    *CONNECTION,
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
                    *CONNECTION,
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
                    *CONNECTION,
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
                    *CONTAINS,
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
                    *CONTAINS,
                    vec![
                        v_string(room_name),
                        v_string(format!("key_{}", room_id / 13)),
                    ],
                );
            }
        }
    }

    // Add player facts
    dl.add_fact(*PLAYER, vec![v_str("player_1")]);

    // Add key unlocking facts
    for i in 0..size / 13 {
        for j in 0..size / 11 {
            let room1 = format!("room_{}", j * 11);
            let room2 = format!("room_{}", j * 11 + grid_size + 1);
            if j * 11 + grid_size + 1 < size {
                dl.add_fact(
                    // Changed
                    *UNLOCKS,
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
            *HAS_ITEM,
            vec![v_str("player_1"), v_string(format!("key_{}", i))],
        );
    }

    // Rules for room access
    // can_access(Player, From, To) :- connection(From, To, 0)
    let player1 = dl.new_variable(*PLAYER);
    let from1 = dl.new_variable(*FROM);
    let to1 = dl.new_variable(*TO);
    let unlocked_path_atom = Atom::new(
        *CONNECTION,
        vec![
            Variable(from1.clone()),
            Variable(to1.clone()),
            Constant(v_int(0)),
        ],
    );
    let can_access_atom = Atom::new(
        *CAN_ACCESS,
        vec![
            Variable(player1.clone()),
            Variable(from1.clone()),
            Variable(to1.clone()),
        ],
    );
    dl.add_rule(Rule::new(can_access_atom, vec![unlocked_path_atom]));

    // can_access(Player, From, To) :- connection(From, To, 1), has_item(Player, Key), unlocks(Key, From, To)
    let player2 = dl.new_variable(*PLAYER);
    let from2 = dl.new_variable(*FROM);
    let to2 = dl.new_variable(*TO);
    let key2 = dl.new_variable(*KEY);

    let locked_path_atom = Atom::new(
        *CONNECTION,
        vec![
            Variable(from2.clone()),
            Variable(to2.clone()),
            Constant(v_int(1)),
        ],
    );
    let has_item_atom = Atom::new(
        *HAS_ITEM,
        vec![Variable(player2.clone()), Variable(key2.clone())],
    );
    let unlocks_atom = Atom::new(
        *UNLOCKS,
        vec![
            Variable(key2.clone()),
            Variable(from2.clone()),
            Variable(to2.clone()),
        ],
    );
    let can_access_locked_atom = Atom::new(
        *CAN_ACCESS,
        vec![
            Variable(player2.clone()),
            Variable(from2.clone()),
            Variable(to2.clone()),
        ],
    );
    dl.add_rule(Rule::new(
        can_access_locked_atom,
        vec![locked_path_atom, has_item_atom, unlocks_atom],
    ));

    // path(Player, X, Y) :- can_access(Player, X, Y)
    let player3 = dl.new_variable(*PLAYER);
    let x3 = dl.new_variable(*X);
    let y3 = dl.new_variable(*Y);
    let can_access_atom = Atom::new(
        *CAN_ACCESS,
        vec![
            Variable(player3.clone()),
            Variable(x3.clone()),
            Variable(y3.clone()),
        ],
    );
    let path_atom = Atom::new(
        *PATH,
        vec![
            Variable(player3.clone()),
            Variable(x3.clone()),
            Variable(y3.clone()),
        ],
    );
    dl.add_rule(Rule::new(path_atom, vec![can_access_atom]));

    // path(Player, X, Z) :- can_access(Player, X, Y), path(Player, Y, Z)
    let player4 = dl.new_variable(*PLAYER);
    let x4 = dl.new_variable(*X);
    let y4 = dl.new_variable(*Y);
    let z4 = dl.new_variable(*Z);

    let can_access_atom = Atom::new(
        *CAN_ACCESS,
        vec![
            Variable(player4.clone()),
            Variable(x4.clone()),
            Variable(y4.clone()),
        ],
    );
    let path_atom_body = Atom::new(
        *PATH,
        vec![
            Variable(player4.clone()),
            Variable(y4.clone()),
            Variable(z4.clone()),
        ],
    );
    let path_atom_head = Atom::new(
        *PATH,
        vec![
            Variable(player4.clone()),
            Variable(x4.clone()),
            Variable(z4.clone()),
        ],
    );
    dl.add_rule(Rule::new(
        path_atom_head,
        vec![can_access_atom, path_atom_body],
    ));

    // can_find(Player, Item) :- path(Player, Start, Room), contains(Room, Item)
    let player5 = dl.new_variable(*PLAYER);
    let room5 = dl.new_variable(*ROOM);
    let item5 = dl.new_variable(*ITEM);
    let start5 = dl.new_variable(*START);

    let path_atom = Atom::new(
        *PATH,
        vec![
            Variable(player5.clone()),
            Variable(start5.clone()),
            Variable(room5.clone()),
        ],
    );
    let contains_atom = Atom::new(
        *CONTAINS,
        vec![Variable(room5.clone()), Variable(item5.clone())],
    );
    let can_find_atom = Atom::new(
        *CAN_FIND,
        vec![Variable(player5.clone()), Variable(item5.clone())],
    );
    dl.add_rule(Rule::new(can_find_atom, vec![path_atom, contains_atom]));

    // Query: can_find(player_1, item_X) - ask about a specific item
    let target_item = format!("item_{}", size / 10);
    let query = Atom::new(
        *CAN_FIND,
        vec![Constant(v_str("player_1")), Constant(v_string(target_item))],
    );

    (dl, query)
}

fn benchmark_full_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_query");
    group.sample_size(10);
    // Ancestor benchmark with different sizes
    for size in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new(ANCESTOR.as_str(), size),
            size,
            |b, &_size| {
                let (mut dl, query) = create_ancestor_datalog();
                b.iter(|| dl.query(black_box(&query)));
            },
        );
    }

    // Path benchmark with different sizes
    for size in [20, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::new(PATH.as_str(), size), size, |b, &size| {
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
                    black_box(!dl.query(&query).is_empty())
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
