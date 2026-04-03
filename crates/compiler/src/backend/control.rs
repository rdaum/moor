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

#[cfg(test)]
mod tests {
    use moor_var::program::{
        labels::{Label, Offset},
        names::Name,
    };

    use super::{ControlState, LoopFrame};

    #[test]
    fn current_and_named_loop_lookup_use_innermost_frame() {
        let mut control = ControlState::new();
        let outer_name = Name(1, 0, 1);
        let inner_name = Name(2, 0, 2);
        let outer = LoopFrame {
            loop_name: Some(outer_name),
            top_label: Label(1),
            top_stack: Offset(0),
            bottom_label: Label(2),
            bottom_stack: Offset(0),
        };
        let inner = LoopFrame {
            loop_name: Some(inner_name),
            top_label: Label(3),
            top_stack: Offset(1),
            bottom_label: Label(4),
            bottom_stack: Offset(1),
        };

        control.push_loop(outer);
        control.push_loop(inner);

        assert_eq!(control.current_loop().unwrap().top_label, Label(3));
        assert_eq!(control.find_loop(&outer_name).unwrap().bottom_label, Label(2));
        assert_eq!(control.find_loop(&inner_name).unwrap().bottom_label, Label(4));
        assert!(control.find_loop(&Name(9, 0, 9)).is_none());
        assert_eq!(control.pop_loop().unwrap().top_label, Label(3));
        assert_eq!(control.current_loop().unwrap().top_label, Label(1));
    }

    #[test]
    fn lambda_depth_push_saturates() {
        let mut control = ControlState::new();
        control.set_lambda_scope_depth(250);

        let outer = control.push_lambda_scope_depth(10);

        assert_eq!(outer, 250);
        assert_eq!(control.lambda_scope_depth(), u8::MAX);
    }
}
