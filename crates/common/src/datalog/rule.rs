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

use std::fmt::{Display, Formatter};

use super::atom::Atom;
use super::variable::Variable;

/// Aggregation operations
#[derive(Clone, Debug)]
pub enum AggregateOp {
    Count,
    Min,
    Max,
}

/// An aggregation literal in a rule body
#[derive(Clone, Debug)]
pub struct AggregateLiteral {
    /// The aggregation operation
    pub op: AggregateOp,
    /// The variable to store the result
    pub result_var: Variable,
    /// The variable to aggregate over
    pub aggregate_var: Variable,
    /// The grouping variables (variables that define the grouping)
    pub group_vars: Vec<Variable>,
    /// The atom pattern that generates the values to aggregate
    pub atom: Atom,
}

/// A literal in a rule body: positive atom, negated atom, or aggregation
#[derive(Clone, Debug)]
pub enum Literal {
    Pos(Atom),
    Neg(Atom),
    Aggregate(AggregateLiteral),
}

/// A Rule is a Horn clause in the form: head :- body
/// where head is a single atom and body is a conjunction of atoms
#[derive(Clone, Debug)]
pub struct Rule {
    /// The head atom of the rule
    pub(crate) head: Atom,
    /// The body literals of the rule
    pub(crate) body: Vec<Literal>,
}

impl Rule {
    /// Create a new rule with the given head and body
    pub fn new(head: Atom, body: Vec<Atom>) -> Self {
        // convenience for all-positive body
        let lits = body.into_iter().map(Literal::Pos).collect();
        Self { head, body: lits }
    }
    /// Create a new rule allowing negation
    pub fn with_negation(head: Atom, body: Vec<Literal>) -> Self {
        Self { head, body }
    }
    /// Create a new rule with literals (including aggregation)
    pub fn with_literals(head: Atom, body: Vec<Literal>) -> Self {
        Self { head, body }
    }
}

impl AggregateLiteral {
    /// Create a new aggregation literal
    pub fn new(
        op: AggregateOp,
        result_var: Variable,
        aggregate_var: Variable,
        group_vars: Vec<Variable>,
        atom: Atom,
    ) -> Self {
        Self {
            op,
            result_var,
            aggregate_var,
            group_vars,
            atom,
        }
    }
}

impl Display for Rule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} :- ", self.head)?;
        for (i, lit) in self.body.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            match lit {
                Literal::Pos(a) => write!(f, "{}", a)?,
                Literal::Neg(a) => write!(f, "not {}", a)?,
                Literal::Aggregate(agg) => write!(f, "{}", agg)?,
            }
        }
        Ok(())
    }
}

impl Display for AggregateOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AggregateOp::Count => write!(f, "count"),
            AggregateOp::Min => write!(f, "min"),
            AggregateOp::Max => write!(f, "max"),
        }
    }
}

impl Display for AggregateLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} = {}({})", self.result_var, self.op, self.aggregate_var)?;
        if !self.group_vars.is_empty() {
            write!(f, " group by [")?;
            for (i, var) in self.group_vars.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", var)?;
            }
            write!(f, "]")?;
        }
        write!(f, " in {}", self.atom)
    }
}
