// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::var::error::Error;
use crate::var::error::Error::{E_INVARG, E_RANGE, E_TYPE};
use crate::var::variant::Variant;
use crate::var::{v_empty_list, v_empty_str, v_listv, Var};
use crate::var::{v_err, v_float, v_int};
use num_traits::Zero;
use std::ops::{Div, Mul, Neg, Sub};

macro_rules! binary_numeric_coercion_op {
    ($op:tt ) => {
        pub fn $op(&self, v: &Var) -> Result<Var, Error> {
            match (self.variant(), v.variant()) {
                (Variant::Float(l), Variant::Float(r)) => Ok(v_float(l.$op(*r))),
                (Variant::Int(l), Variant::Int(r)) => Ok(v_int(l.$op(*r))),
                (Variant::Float(l), Variant::Int(r)) => Ok(v_float(l.$op(*r as f64))),
                (Variant::Int(l), Variant::Float(r)) => Ok(v_float((*l as f64).$op(*r))),
                (_, _) => Ok(v_err(E_TYPE)),
            }
        }
    };
}

impl Var {
    #[must_use]
    pub fn is_true(&self) -> bool {
        match self.variant() {
            Variant::Str(s) => !s.is_empty(),
            Variant::Int(i) => *i != 0,
            Variant::Float(f) => !f.is_zero(),
            Variant::List(l) => !l.is_empty(),
            _ => false,
        }
    }

    pub fn index_set(&mut self, i: usize, value: Self) -> Result<Self, Error> {
        match self.variant_mut() {
            Variant::List(l) => {
                if !i < l.len() {
                    return Err(E_RANGE);
                }

                Ok(l.set(i, value))
            }
            Variant::Str(s) => {
                if !i < s.len() {
                    return Err(E_RANGE);
                }

                let Variant::Str(value) = value.variant() else {
                    return Err(E_INVARG);
                };

                if value.len() != 1 {
                    return Err(E_INVARG);
                }

                Ok(s.set(i, value))
            }
            _ => Err(E_TYPE),
        }
    }

    /// 1-indexed position of the first occurrence of `v` in `self`, or `E_TYPE` if `self` is not a
    /// list.
    // TODO(): Make Var consistent on 0-indexing vs 1-indexing
    //   Various places have 1-indexing polluting the Var API, but in others we
    //   assume 0-indexing and adjust in the opcodes.  0 indexing should be done in Var, and opcodes and builtins
    //   should be the ones to adjust 1-indexing.
    //   Examples: index_in, range, rangeset
    #[must_use]
    pub fn index_in(&self, v: &Self) -> Self {
        let Variant::List(l) = self.variant() else {
            return v_err(E_TYPE);
        };

        match l.iter().position(|x| x == v) {
            None => v_int(0),
            Some(i) => v_int(i as i64 + 1),
        }
    }

    binary_numeric_coercion_op!(mul);
    binary_numeric_coercion_op!(div);
    binary_numeric_coercion_op!(sub);

    pub fn add(&self, v: &Self) -> Result<Self, Error> {
        match (self.variant(), v.variant()) {
            (Variant::Float(l), Variant::Float(r)) => Ok(v_float(*l + *r)),
            (Variant::Int(l), Variant::Int(r)) => Ok(v_int(l + r)),
            (Variant::Float(l), Variant::Int(r)) => Ok(v_float(*l + (*r as f64))),
            (Variant::Int(l), Variant::Float(r)) => Ok(v_float(*l as f64 + *r)),
            (Variant::Str(s), Variant::Str(r)) => Ok(s.append(r)),
            (_, _) => Ok(v_err(E_TYPE)),
        }
    }

    pub fn negative(&self) -> Result<Self, Error> {
        match self.variant() {
            Variant::Int(l) => Ok(v_int(-*l)),
            Variant::Float(f) => Ok(v_float(f.neg())),
            _ => Ok(v_err(E_TYPE)),
        }
    }

    pub fn modulus(&self, v: &Self) -> Result<Self, Error> {
        match (self.variant(), v.variant()) {
            (Variant::Float(l), Variant::Float(r)) => Ok(v_float(*l % *r)),
            (Variant::Int(l), Variant::Int(r)) => Ok(v_int(l % r)),
            (Variant::Float(l), Variant::Int(r)) => Ok(v_float(*l % (*r as f64))),
            (Variant::Int(l), Variant::Float(r)) => Ok(v_float(*l as f64 % (*r))),
            (_, _) => Ok(v_err(E_TYPE)),
        }
    }

    pub fn pow(&self, v: &Self) -> Result<Self, Error> {
        match (self.variant(), v.variant()) {
            (Variant::Float(l), Variant::Float(r)) => Ok(v_float(l.powf(*r))),
            (Variant::Int(l), Variant::Int(r)) => Ok(v_int(l.pow(*r as u32))),
            (Variant::Float(l), Variant::Int(r)) => Ok(v_float(l.powi(*r as i32))),
            (Variant::Int(l), Variant::Float(r)) => Ok(v_float((*l as f64).powf(*r))),
            (_, _) => Ok(v_err(E_TYPE)),
        }
    }

    pub fn len(&self) -> Result<Self, Error> {
        match self.variant() {
            Variant::Str(s) => Ok(v_int(s.len() as i64)),
            Variant::List(l) => Ok(v_int(l.len() as i64)),
            _ => Ok(v_err(E_TYPE)),
        }
    }

    pub fn index(&self, idx: usize) -> Result<Self, Error> {
        match self.variant() {
            Variant::List(l) => match l.get(idx) {
                None => Ok(v_err(E_RANGE)),
                Some(v) => Ok(v.clone()),
            },
            Variant::Str(s) => match s.get(idx) {
                None => Ok(v_err(E_RANGE)),
                Some(v) => Ok(v),
            },
            _ => Ok(v_err(E_TYPE)),
        }
    }

    pub fn range(&self, from: i64, to: i64) -> Result<Self, Error> {
        match self.variant() {
            Variant::Str(s) => {
                let len = s.len() as i64;
                if to < from {
                    return Ok(v_empty_str());
                }
                if from <= 0 || from > len + 1 || to > len {
                    return Ok(v_err(E_RANGE));
                }
                let (from, to) = (from as usize, to as usize);
                Ok(s.get_range(from - 1..to).unwrap())
            }
            Variant::List(l) => {
                let len = l.len() as i64;
                if to < from {
                    return Ok(v_empty_list());
                }
                if from <= 0 || from > len + 1 || to < 1 || to > len {
                    return Ok(v_err(E_RANGE));
                }
                let mut res = Vec::with_capacity((to - from + 1) as usize);
                for i in from..=to {
                    res.push(l[(i - 1) as usize].clone());
                }
                Ok(v_listv(res))
            }
            _ => Ok(v_err(E_TYPE)),
        }
    }

    pub fn rangeset(&self, value: Self, from: i64, to: i64) -> Result<Self, Error> {
        let (base_len, val_len) = match (self.variant(), value.variant()) {
            (Variant::Str(base_str), Variant::Str(val_str)) => {
                (base_str.len() as i64, val_str.len() as i64)
            }
            (Variant::List(base_list), Variant::List(val_list)) => {
                (base_list.len() as i64, val_list.len() as i64)
            }
            _ => return Ok(v_err(E_TYPE)),
        };

        // In range assignments only, MOO treats "0" for start (and even negative values) as
        // "start of range."
        // So we'll just min 'from' to '1' here.
        // Does not hold for range retrievals.
        let from = from.max(1);
        if from > base_len + 1 || to > base_len {
            return Ok(v_err(E_RANGE));
        }

        let lenleft = if from > 1 { from - 1 } else { 0 };
        let lenmiddle = val_len;
        let lenright = if base_len > to { base_len - to } else { 0 };
        let newsize = lenleft + lenmiddle + lenright;

        let (from, to) = (from as usize, to as usize);
        let ans = match (self.variant(), value.variant()) {
            (Variant::Str(base_str), Variant::Str(_value_str)) => {
                let ans = base_str.get_range(0..from - 1).unwrap_or_else(v_empty_str);
                let ans = ans.add(&value)?;

                ans.add(
                    &base_str
                        .get_range(to..base_str.len())
                        .unwrap_or_else(v_empty_str),
                )?
            }
            (Variant::List(base_list), Variant::List(value_list)) => {
                let mut ans: Vec<Self> = Vec::with_capacity(newsize as usize);
                ans.extend_from_slice(&base_list[..from - 1]);
                ans.extend(value_list.iter().cloned());
                ans.extend_from_slice(&base_list[to..]);
                v_listv(ans)
            }
            _ => unreachable!(),
        };

        Ok(ans)
    }
}
