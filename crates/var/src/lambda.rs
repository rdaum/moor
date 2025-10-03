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

use crate::{
    Var,
    program::{opcode::ScatterArgs, program::Program},
};
use std::sync::Arc;

/// Lambda function value containing parameter specification, compiled body, and captured environment
#[derive(Clone, Debug, PartialEq)]
pub struct Lambda(pub Arc<LambdaInner>);

#[derive(Clone, Debug, PartialEq)]
pub struct LambdaInner {
    /// Parameter specification (reuses scatter assignment structure)
    pub params: ScatterArgs,
    /// The lambda body as standalone executable program
    /// Compiled at compile-time into a complete, self-contained Program
    pub body: Program,
    /// Captured variable environment from lambda creation site
    pub captured_env: Vec<Vec<Var>>,
    /// Optional self-reference variable name for recursive lambdas
    pub self_var: Option<crate::program::names::Name>,
}

impl Lambda {
    /// Create a new lambda value
    pub fn new(
        params: ScatterArgs,
        body: Program,
        captured_env: Vec<Vec<Var>>,
        self_var: Option<crate::program::names::Name>,
    ) -> Self {
        Self(Arc::new(LambdaInner {
            params,
            body,
            captured_env,
            self_var,
        }))
    }

    /// Create a deep copy of this lambda for self-reference to avoid cycles.
    ///
    /// TODO: If we ever get a cycle-collecting or full tracing GC, we can safely back this out
    ///   and allow true cycles since they would be properly collected.
    pub fn for_self_reference(&self) -> Self {
        Self(Arc::new(LambdaInner {
            params: self.0.params.clone(),
            body: self.0.body.clone(),
            captured_env: self.0.captured_env.clone(),
            self_var: self.0.self_var,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program::names::Name;

    #[test]
    fn test_recursive_lambda_cycle_problem() {
        // This test demonstrates the cycle problem with the naive approach

        // Create a simple program for testing
        let program = Program::new();
        let params = ScatterArgs {
            labels: vec![],
            done: crate::program::labels::Label(0),
        };
        let self_var_name = Name(0, 0, 0); // Variable index 0 for self-reference

        // Create initial lambda with self-reference
        let lambda = Lambda::new(
            params.clone(),
            program.clone(),
            vec![vec![crate::v_int(0); 1]], // Environment with one slot
            Some(self_var_name),
        );

        // Simulate what the OLD activation code would do - create a cycle!
        let mut env = vec![crate::v_int(0); 1];

        // Create a lambda that refers to itself using the SAME Arc - this creates the cycle!
        let self_lambda_var = crate::Var::mk_lambda(
            lambda.0.params.clone(),
            lambda.0.body.clone(),
            vec![env.clone()], // This will eventually contain itself
            lambda.0.self_var,
        );

        // Put the lambda into its own environment
        env[0] = self_lambda_var.clone();

        // Update the lambda with the self-referencing environment
        let _cyclic_lambda = Lambda::new(
            lambda.0.params.clone(),
            lambda.0.body.clone(),
            vec![env],
            lambda.0.self_var,
        );

        // This creates a cycle, which is bad!
    }

    #[test]
    fn test_recursive_lambda_cycle_fix() {
        // This test demonstrates the fix using for_self_reference()

        // Create a simple program for testing
        let program = Program::new();
        let params = ScatterArgs {
            labels: vec![],
            done: crate::program::labels::Label(0),
        };
        let self_var_name = Name(0, 0, 0); // Variable index 0 for self-reference

        // Create initial lambda with self-reference
        let lambda = Lambda::new(
            params.clone(),
            program.clone(),
            vec![vec![crate::v_int(0); 1]], // Environment with one slot
            Some(self_var_name),
        );

        // Simulate what the NEW activation code does - use for_self_reference()
        let mut env = vec![crate::v_int(0); 1];

        // Create a DEEP COPY of the lambda to avoid cycles
        let self_lambda = lambda.for_self_reference();
        let self_lambda_var = crate::Var::mk_lambda(
            self_lambda.0.params.clone(),
            self_lambda.0.body.clone(),
            vec![env.clone()],
            self_lambda.0.self_var,
        );

        // Put the lambda copy into the environment
        env[0] = self_lambda_var.clone();

        // This doesn't create a cycle because self_lambda is a separate Arc allocation
        let non_cyclic_lambda = Lambda::new(
            lambda.0.params.clone(),
            lambda.0.body.clone(),
            vec![env],
            lambda.0.self_var,
        );

        // Verify that the lambda and self-reference lambda are separate Arc instances
        if let Some(inner_lambda) = self_lambda_var.as_lambda() {
            // These should be different Arc instances (different pointer addresses)
            let lambda_ptr = &*lambda.0 as *const LambdaInner;
            let self_lambda_ptr = &*inner_lambda.0 as *const LambdaInner;
            assert_ne!(
                lambda_ptr, self_lambda_ptr,
                "Self-reference should be a separate Arc"
            );
        }

        // No cycle created - memory will be properly freed when dropped
        drop(non_cyclic_lambda);
        drop(self_lambda_var);
        drop(lambda);
    }
}
