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

use moor_var::program::{
    labels::{Label, Offset},
    names::Name,
};

#[derive(Debug, Clone, Copy)]
pub struct LoopFrame {
    pub loop_name: Option<Name>,
    pub top_label: Label,
    pub top_stack: Offset,
    pub bottom_label: Label,
    pub bottom_stack: Offset,
}

#[derive(Debug, Default)]
pub struct ControlState {
    loops: Vec<LoopFrame>,
    lambda_scope_depth: u8,
}

impl ControlState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_loop(&mut self, loop_frame: LoopFrame) {
        self.loops.push(loop_frame);
    }

    pub fn pop_loop(&mut self) -> Option<LoopFrame> {
        self.loops.pop()
    }

    pub fn current_loop(&self) -> Option<&LoopFrame> {
        self.loops.last()
    }

    pub fn find_loop(&self, loop_label: &Name) -> Option<&LoopFrame> {
        self.loops
            .iter()
            .rev()
            .find(|frame| frame.loop_name.as_ref() == Some(loop_label))
    }

    pub fn lambda_scope_depth(&self) -> u8 {
        self.lambda_scope_depth
    }

    pub fn set_lambda_scope_depth(&mut self, depth: u8) {
        self.lambda_scope_depth = depth;
    }

    pub fn push_lambda_scope_depth(&mut self, levels: u8) -> u8 {
        let outer = self.lambda_scope_depth;
        self.lambda_scope_depth = self.lambda_scope_depth.saturating_add(levels);
        outer
    }
}
