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

use moor_compiler::ScatterLabel;
use moor_var::program::opcode::ScatterArgs;
use moor_var::{E_ARGS, Error, Var, v_list};

/// Core scatter assignment logic shared between regular Op::Scatter and lambda parameter binding.
/// Returns a result indicating whether default value assignment needs to occur for optional parameters.
pub struct ScatterResult {
    /// Success/failure of the scatter operation
    pub result: Result<(), Error>,
    /// Whether to execute default value assignment for optional parameters
    pub needs_defaults: bool,
}

/// Core scatter assignment logic that can be used by both Op::Scatter and lambda parameter binding.
///
/// For VM execution (regular Op::Scatter), this function sets variables directly on the frame and
/// returns whether jump-to-defaults logic should be executed.
///
/// For lambda parameter binding, this function sets variables in the environment array and
/// the caller should handle default value assignment if `needs_defaults` is true.
pub fn scatter_assign<F>(table: &ScatterArgs, args: &[Var], mut set_var: F) -> ScatterResult
where
    F: FnMut(&moor_var::program::names::Name, Var),
{
    // Count parameter types - this logic is identical in both implementations
    let (nargs, rest, nreq) = {
        let mut nargs = 0;
        let mut rest = 0;
        let mut nreq = 0;
        for label in table.labels.iter() {
            match label {
                ScatterLabel::Rest(_) => rest += 1,
                ScatterLabel::Required(_) => nreq += 1,
                ScatterLabel::Optional(_, _) => {}
            }
            nargs += 1;
        }
        (nargs, rest, nreq)
    };

    // Validate arguments - this logic is identical in both implementations
    let have_rest = rest > 0; // We have rest parameters if any ScatterLabel::Rest exists
    let len = args.len();

    if len < nreq || (!have_rest && len > nargs) {
        return ScatterResult {
            result: Err(E_ARGS.into()),
            needs_defaults: false,
        };
    }

    // Calculate distribution - this logic is identical in both implementations
    let mut nopt_avail = len - nreq;
    let nrest = if have_rest && len >= nargs {
        len - nargs + 1
    } else {
        0
    };

    let mut needs_defaults = false;
    let mut args_iter = args.iter();

    // Assign parameters - this logic is very similar but handles defaults differently
    for label in table.labels.iter() {
        match label {
            ScatterLabel::Rest(id) => {
                // Collect remaining arguments into a list
                let mut v = vec![];
                for _ in 0..nrest {
                    if let Some(rest) = args_iter.next() {
                        v.push(rest.clone());
                    }
                }
                let rest = v_list(&v);
                set_var(id, rest);
            }
            ScatterLabel::Required(id) => {
                // Assign required parameter
                if let Some(arg) = args_iter.next() {
                    set_var(id, arg.clone());
                } else {
                    return ScatterResult {
                        result: Err(E_ARGS.into()),
                        needs_defaults: false,
                    };
                }
            }
            ScatterLabel::Optional(id, jump_to) => {
                if nopt_avail > 0 {
                    nopt_avail -= 1;
                    if let Some(arg) = args_iter.next() {
                        set_var(id, arg.clone());
                    } else {
                        return ScatterResult {
                            result: Err(E_ARGS.into()),
                            needs_defaults: false,
                        };
                    }
                } else {
                    // No argument provided for this optional parameter
                    // For VM execution, this triggers jump-to-defaults logic
                    // For lambdas, the caller should handle default assignment
                    if jump_to.is_some() {
                        needs_defaults = true;
                    }
                    // Note: We don't set the variable here - the caller handles defaults
                }
            }
        }
    }

    ScatterResult {
        result: Ok(()),
        needs_defaults,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::program::names::Name;
    use moor_var::{v_int, v_str};
    use std::collections::HashMap;

    // Create a test ScatterArgs - we'll use unsafe to create the Label since it's private
    fn create_test_scatter_args(labels: Vec<ScatterLabel>) -> ScatterArgs {
        ScatterArgs {
            labels,
            done: unsafe { std::mem::zeroed() }, // Dummy label for testing
        }
    }

    #[test]
    fn test_required_params_success() {
        let table = create_test_scatter_args(vec![
            ScatterLabel::Required(Name(0, 0, 0)),
            ScatterLabel::Required(Name(1, 0, 0)),
        ]);

        let args = vec![v_int(42), v_str("hello")];
        let mut assignments = HashMap::new();

        let result = scatter_assign(&table, &args, |name, value| {
            assignments.insert(*name, value);
        });

        assert!(result.result.is_ok());
        assert!(!result.needs_defaults);
        assert_eq!(assignments.get(&Name(0, 0, 0)), Some(&v_int(42)));
        assert_eq!(assignments.get(&Name(1, 0, 0)), Some(&v_str("hello")));
    }

    #[test]
    fn test_optional_params_without_values() {
        let table = create_test_scatter_args(vec![
            ScatterLabel::Required(Name(0, 0, 0)),
            ScatterLabel::Optional(Name(1, 0, 0), Some(unsafe { std::mem::zeroed() })),
        ]);

        let args = vec![v_str("Alice")]; // Missing optional arg
        let mut assignments = HashMap::new();

        let result = scatter_assign(&table, &args, |name, value| {
            assignments.insert(*name, value);
        });

        assert!(result.result.is_ok());
        assert!(result.needs_defaults); // Need to handle default for optional param
        assert_eq!(assignments.get(&Name(0, 0, 0)), Some(&v_str("Alice")));
        // Name(1,0,0) should NOT be in assignments - defaults handled by caller
        assert!(!assignments.contains_key(&Name(1, 0, 0)));
    }

    #[test]
    fn test_too_many_args_without_rest() {
        let table = create_test_scatter_args(vec![ScatterLabel::Required(Name(0, 0, 0))]);

        let args = vec![v_int(1), v_int(2), v_int(3)]; // Too many!
        let mut assignments = HashMap::new();

        let result = scatter_assign(&table, &args, |name, value| {
            assignments.insert(*name, value);
        });

        assert!(result.result.is_err());
    }
}
