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

//! Builtin functions for spatial/tile-map operations (pathfinding, etc.).

use std::collections::BinaryHeap;
use std::cmp::Reverse;

use moor_compiler::offset_for_builtin;
use moor_var::{E_ARGS, E_INVARG, E_TYPE, Variant, v_empty_list, v_int, v_list, v_list_iter};

use crate::vm::builtins::{BfCallState, BfErr, BfRet, BfRet::Ret, BuiltinFunction};

/// Usage: `list astar(int width, int height, int start_x, int start_y, int goal_x, int goal_y, list tile_map, list solid_tiles)`
///
/// A* pathfinding on a tile grid. Returns a list of `{x, y}` waypoints from
/// the start to the goal (excluding the start position), or an empty list if
/// no path exists.
///
/// Supports 8-directional movement (cardinal + diagonal). Diagonal moves are
/// only permitted when both adjacent cardinal tiles are passable (no corner-cutting).
///
/// `tile_map` is a flat list of tile IDs (1-based indexing, row-major).
/// `solid_tiles` is a list of tile IDs that are impassable.
///
/// Uses Chebyshev distance as the heuristic (diagonal cost = cardinal cost).
fn bf_astar(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 8 {
        return Err(BfErr::ErrValue(E_ARGS.msg("astar() takes 8 arguments")));
    }

    let width = match bf_args.args[0].variant() {
        Variant::Int(i) if i > 0 => i as usize,
        _ => return Err(BfErr::ErrValue(E_INVARG.msg("width must be a positive integer"))),
    };
    let height = match bf_args.args[1].variant() {
        Variant::Int(i) if i > 0 => i as usize,
        _ => return Err(BfErr::ErrValue(E_INVARG.msg("height must be a positive integer"))),
    };
    let start_x = match bf_args.args[2].variant() {
        Variant::Int(i) => i as i32,
        _ => return Err(BfErr::ErrValue(E_TYPE.msg("start_x must be an integer"))),
    };
    let start_y = match bf_args.args[3].variant() {
        Variant::Int(i) => i as i32,
        _ => return Err(BfErr::ErrValue(E_TYPE.msg("start_y must be an integer"))),
    };
    let goal_x = match bf_args.args[4].variant() {
        Variant::Int(i) => i as i32,
        _ => return Err(BfErr::ErrValue(E_TYPE.msg("goal_x must be an integer"))),
    };
    let goal_y = match bf_args.args[5].variant() {
        Variant::Int(i) => i as i32,
        _ => return Err(BfErr::ErrValue(E_TYPE.msg("goal_y must be an integer"))),
    };

    let tile_map_list = bf_args.args[6].as_list()
        .ok_or_else(|| BfErr::ErrValue(E_TYPE.msg("tile_map must be a list")))?;
    let solid_tiles_list = bf_args.args[7].as_list()
        .ok_or_else(|| BfErr::ErrValue(E_TYPE.msg("solid_tiles must be a list")))?;

    let w = width as i32;
    let h = height as i32;

    // Validate start/goal in bounds.
    if start_x < 0 || start_x >= w || start_y < 0 || start_y >= h {
        return Err(BfErr::ErrValue(E_INVARG.msg("start position out of bounds")));
    }
    if goal_x < 0 || goal_x >= w || goal_y < 0 || goal_y >= h {
        return Err(BfErr::ErrValue(E_INVARG.msg("goal position out of bounds")));
    }

    // Build passability bitmap from tile_map and solid_tiles.
    // solid_tile_ids: collect into a set for fast lookup.
    let mut solid_ids = std::collections::HashSet::new();
    for item in solid_tiles_list.iter() {
        if let Variant::Int(id) = item.variant() {
            solid_ids.insert(id);
        }
    }

    // Build flat passability grid: true = passable.
    let grid_size = width * height;
    let mut passable = vec![true; grid_size];
    for (i, item) in tile_map_list.iter().enumerate() {
        if i >= grid_size {
            break;
        }
        if let Variant::Int(tile_id) = item.variant() {
            if solid_ids.contains(&tile_id) {
                passable[i] = false;
            }
        }
    }

    // Check goal is passable.
    if !passable[(goal_y as usize) * width + (goal_x as usize)] {
        return Ok(Ret(v_empty_list()));
    }

    // Already there.
    if start_x == goal_x && start_y == goal_y {
        return Ok(Ret(v_empty_list()));
    }

    let is_passable = |x: i32, y: i32| -> bool {
        x >= 0 && x < w && y >= 0 && y < h && passable[(y as usize) * width + (x as usize)]
    };

    // A* with binary heap (min-heap via Reverse).
    // Node: (f_score, g_score, x, y)
    let mut open: BinaryHeap<Reverse<(i32, i32, i32, i32)>> = BinaryHeap::new();
    let mut g_score = vec![i32::MAX; grid_size];
    let mut came_from = vec![u32::MAX; grid_size]; // flat index of parent

    let start_idx = (start_y as usize) * width + (start_x as usize);
    let goal_idx = (goal_y as usize) * width + (goal_x as usize);

    g_score[start_idx] = 0;
    let h0 = (goal_x - start_x).abs().max((goal_y - start_y).abs()); // Chebyshev
    open.push(Reverse((h0, 0, start_x, start_y)));

    // 8-directional neighbors.
    const DIRS: [(i32, i32); 8] = [
        (-1, -1), (0, -1), (1, -1),
        (-1,  0),          (1,  0),
        (-1,  1), (0,  1), (1,  1),
    ];

    while let Some(Reverse((_f, g, cx, cy))) = open.pop() {
        let cidx = (cy as usize) * width + (cx as usize);

        // Skip stale entries.
        if g > g_score[cidx] {
            continue;
        }

        // Goal reached.
        if cidx == goal_idx {
            // Reconstruct path.
            let mut path = Vec::new();
            let mut idx = goal_idx;
            while idx != start_idx {
                let px = (idx % width) as i64;
                let py = (idx / width) as i64;
                path.push(v_list(&[v_int(px), v_int(py)]));
                idx = came_from[idx] as usize;
            }
            path.reverse();
            return Ok(Ret(v_list_iter(path)));
        }

        let ng = g + 1;

        for &(dx, dy) in &DIRS {
            let nx = cx + dx;
            let ny = cy + dy;

            if !is_passable(nx, ny) {
                continue;
            }

            // Diagonal corner-cutting check.
            if dx != 0 && dy != 0 {
                if !is_passable(cx + dx, cy) || !is_passable(cx, cy + dy) {
                    continue;
                }
            }

            let nidx = (ny as usize) * width + (nx as usize);
            if ng < g_score[nidx] {
                g_score[nidx] = ng;
                came_from[nidx] = cidx as u32;
                let heuristic = (goal_x - nx).abs().max((goal_y - ny).abs());
                open.push(Reverse((ng + heuristic, ng, nx, ny)));
            }
        }
    }

    // No path found.
    Ok(Ret(v_empty_list()))
}

pub(crate) fn register_bf_spatial(builtins: &mut [BuiltinFunction]) {
    builtins[offset_for_builtin("astar")] = bf_astar;
}
