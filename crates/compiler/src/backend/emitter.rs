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
    labels::{JumpLabel, Label},
    names::Name,
    opcode::Op,
};

#[derive(Debug, Default)]
pub struct EmitterState {
    ops: Vec<Op>,
    jumps: Vec<JumpLabel>,
}

impl EmitterState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emit(&mut self, op: Op) {
        self.ops.push(op);
    }

    pub fn pc(&self) -> usize {
        self.ops.len()
    }

    pub fn last_op_mut(&mut self) -> Option<&mut Op> {
        self.ops.last_mut()
    }

    pub fn new_jump_label(&mut self, name: Option<Name>) -> Label {
        let id = Label(self.jumps.len() as u16);
        let position = self.ops.len().into();
        self.jumps.push(JumpLabel { id, name, position });
        id
    }

    pub fn bind_jump_label(&mut self, id: Label) {
        let position = self.ops.len();
        let jump = self
            .jumps
            .get_mut(id.0 as usize)
            .expect("Invalid jump fixup");
        jump.position = position.into();
    }

    pub fn take_ops(&mut self) -> Vec<Op> {
        std::mem::take(&mut self.ops)
    }

    pub fn replace_ops(&mut self, ops: Vec<Op>) {
        self.ops = ops;
    }

    pub fn take_jumps(&mut self) -> Vec<JumpLabel> {
        std::mem::take(&mut self.jumps)
    }

    pub fn replace_jumps(&mut self, jumps: Vec<JumpLabel>) {
        self.jumps = jumps;
    }

    pub fn reset(&mut self) {
        self.ops.clear();
        self.jumps.clear();
    }
}

#[cfg(test)]
mod tests {
    use moor_var::program::{
        labels::Label,
        names::Name,
        opcode::Op,
    };

    use super::EmitterState;

    #[test]
    fn label_binding_tracks_program_counter() {
        let mut emitter = EmitterState::new();
        let named = Name(7, 2, 3);
        let start = emitter.new_jump_label(Some(named));

        emitter.emit(Op::ImmInt(1));
        emitter.emit(Op::ImmInt(2));
        let end = emitter.new_jump_label(None);
        emitter.bind_jump_label(start);

        let jumps = emitter.take_jumps();
        assert_eq!(start, Label(0));
        assert_eq!(end, Label(1));
        assert_eq!(jumps[0].name, Some(named));
        assert_eq!(jumps[0].position.0, 2);
        assert_eq!(jumps[1].position.0, 2);
    }

    #[test]
    fn reset_clears_ops_and_jumps() {
        let mut emitter = EmitterState::new();
        emitter.emit(Op::ImmInt(1));
        emitter.new_jump_label(None);
        assert_eq!(emitter.pc(), 1);

        emitter.reset();

        assert_eq!(emitter.pc(), 0);
        assert!(emitter.take_ops().is_empty());
        assert!(emitter.take_jumps().is_empty());
    }

    #[test]
    fn last_op_mut_updates_recent_opcode() {
        let mut emitter = EmitterState::new();
        emitter.emit(Op::ImmInt(1));
        *emitter.last_op_mut().unwrap() = Op::ImmInt(9);

        assert_eq!(emitter.take_ops(), vec![Op::ImmInt(9)]);
    }
}
