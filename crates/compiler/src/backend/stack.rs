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

use moor_var::program::labels::Offset;

#[derive(Debug, Default)]
pub struct StackState {
    cur_stack: usize,
    max_stack: usize,
    saved_stack: Option<Offset>,
}

impl StackState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, n: usize) {
        self.cur_stack += n;
        if self.cur_stack > self.max_stack {
            self.max_stack = self.cur_stack;
        }
    }

    pub fn pop(&mut self, n: usize) {
        if self.cur_stack < n {
            panic!(
                "Stack underflow: trying to pop {} items but stack only has {} items",
                n, self.cur_stack
            );
        }
        self.cur_stack -= n;
    }

    pub fn depth(&self) -> usize {
        self.cur_stack
    }

    pub fn set_depth(&mut self, depth: usize) {
        self.cur_stack = depth;
        if self.cur_stack > self.max_stack {
            self.max_stack = self.cur_stack;
        }
    }

    #[cfg(test)]
    pub fn max_depth(&self) -> usize {
        self.max_stack
    }

    pub fn saved_top(&self) -> Option<Offset> {
        self.saved_stack
    }

    pub fn save_top(&mut self) -> Option<Offset> {
        let old = self.saved_stack;
        self.saved_stack = Some((self.cur_stack - 1).into());
        old
    }

    pub fn restore_saved_top(&mut self, old: Option<Offset>) {
        self.saved_stack = old;
    }
}

#[cfg(test)]
mod tests {
    use super::StackState;
    use moor_var::program::labels::Offset;

    #[test]
    fn tracks_depth_and_saved_top() {
        let mut stack = StackState::new();
        stack.push(3);
        assert_eq!(stack.depth(), 3);
        assert_eq!(stack.max_depth(), 3);

        let old = stack.save_top();
        assert_eq!(old, None);
        assert_eq!(stack.saved_top(), Some(Offset(2)));

        stack.pop(2);
        assert_eq!(stack.depth(), 1);
        stack.restore_saved_top(old);
        assert_eq!(stack.saved_top(), None);
    }

    #[test]
    fn depth_reset_updates_max_depth() {
        let mut stack = StackState::new();
        stack.set_depth(4);
        stack.set_depth(2);
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.max_depth(), 4);
    }
}
