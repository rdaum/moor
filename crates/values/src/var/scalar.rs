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

use crate::var::var::{v_err, v_float, v_int, Var};
use crate::var::variant::Variant;
use crate::var::Error;
use crate::var::Error::{E_INVARG, E_TYPE};
use num_traits::ToPrimitive;
use paste::paste;
use std::ops::{Div, Mul, Neg, Sub};

macro_rules! binary_numeric_coercion_op {
    ($op:tt ) => {
        pub fn $op(&self, v: &Var) -> Result<Var, Error> {
            match (self.variant(), v.variant()) {
                (Variant::Float(l), Variant::Float(r)) => {
                    Ok(v_float(l.to_f64().unwrap().$op(r.to_f64().unwrap())))
                }
                (Variant::Int(l), Variant::Int(r)) => {
                    paste! { l.[<checked_ $op>](r).map(v_int).ok_or(E_INVARG) }
                }
                (Variant::Float(l), Variant::Int(r)) => {
                    Ok(v_float(l.to_f64().unwrap().$op(r as f64)))
                }
                (Variant::Int(l), Variant::Float(r)) => {
                    Ok(v_float((l as f64).$op(r.to_f64().unwrap())))
                }
                (_, _) => Ok(v_err(E_TYPE)),
            }
        }
    };
}

impl Var {
    binary_numeric_coercion_op!(mul);
    binary_numeric_coercion_op!(div);
    binary_numeric_coercion_op!(sub);

    pub fn add(&self, v: &Self) -> Result<Self, Error> {
        match (self.variant(), v.variant()) {
            (Variant::Float(l), Variant::Float(r)) => {
                Ok(v_float(l.to_f64().unwrap() + r.to_f64().unwrap()))
            }
            (Variant::Int(l), Variant::Int(r)) => l.checked_add(r).map(v_int).ok_or(E_INVARG),
            (Variant::Float(l), Variant::Int(r)) => Ok(v_float(l.to_f64().unwrap() + (r as f64))),
            (Variant::Int(l), Variant::Float(r)) => Ok(v_float(l as f64 + r.to_f64().unwrap())),
            (Variant::Str(s), Variant::Str(r)) => Ok(s.append(&r)),
            (_, _) => Ok(v_err(E_TYPE)),
        }
    }

    pub fn negative(&self) -> Result<Self, Error> {
        match self.variant() {
            Variant::Int(l) => l.checked_neg().map(v_int).ok_or(E_INVARG),
            Variant::Float(f) => Ok(v_float(f.neg())),
            _ => Ok(v_err(E_TYPE)),
        }
    }

    pub fn modulus(&self, v: &Self) -> Result<Self, Error> {
        match (self.variant(), v.variant()) {
            (Variant::Float(l), Variant::Float(r)) => Ok(v_float(l % r)),
            (Variant::Int(l), Variant::Int(r)) => l.checked_rem(r).map(v_int).ok_or(E_INVARG),
            (Variant::Float(l), Variant::Int(r)) => Ok(v_float(l.to_f64().unwrap() % (r as f64))),
            (Variant::Int(l), Variant::Float(r)) => Ok(v_float(l as f64 % (r.to_f64().unwrap()))),
            (_, _) => Ok(v_err(E_TYPE)),
        }
    }

    pub fn pow(&self, v: &Self) -> Result<Self, Error> {
        match (self.variant(), v.variant()) {
            (Variant::Float(l), Variant::Float(r)) => Ok(v_float(l.powf(r))),
            (Variant::Int(l), Variant::Int(r)) => {
                let r = u32::try_from(r).map_err(|_| E_INVARG)?;
                l.checked_pow(r).map(v_int).ok_or(E_INVARG)
            }
            (Variant::Float(l), Variant::Int(r)) => Ok(v_float(l.powi(r as i32))),
            (Variant::Int(l), Variant::Float(r)) => Ok(v_float((l as f64).powf(r))),
            (_, _) => Ok(v_err(E_TYPE)),
        }
    }

    pub fn is_sysobj(&self) -> bool {
        matches!(self.variant(), Variant::Obj(o) if o.0 == 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::var::var::{v_err, v_float, v_int, v_list, v_obj, v_str};
    use crate::var::Error;
    use crate::var::Error::{E_RANGE, E_TYPE};

    #[test]
    fn test_add() {
        assert_eq!(v_int(1).add(&v_int(2)), Ok(v_int(3)));
        assert_eq!(v_int(1).add(&v_float(2.0)), Ok(v_float(3.0)));
        assert_eq!(v_float(1.).add(&v_int(2)), Ok(v_float(3.)));
        assert_eq!(v_float(1.).add(&v_float(2.)), Ok(v_float(3.)));
        assert_eq!(v_str("a").add(&v_str("b")), Ok(v_str("ab")));
    }

    #[test]
    fn test_sub() -> Result<(), Error> {
        assert_eq!(v_int(1).sub(&v_int(2))?, v_int(-1));
        assert_eq!(v_int(1).sub(&v_float(2.))?, v_float(-1.));
        assert_eq!(v_float(1.).sub(&v_int(2))?, v_float(-1.));
        assert_eq!(v_float(1.).sub(&v_float(2.))?, v_float(-1.));
        Ok(())
    }

    #[test]
    fn test_mul() -> Result<(), Error> {
        assert_eq!(v_int(1).mul(&v_int(2))?, v_int(2));
        assert_eq!(v_int(1).mul(&v_float(2.))?, v_float(2.));
        assert_eq!(v_float(1.).mul(&v_int(2))?, v_float(2.));
        assert_eq!(v_float(1.).mul(&v_float(2.))?, v_float(2.));
        Ok(())
    }

    #[test]
    fn test_div() -> Result<(), Error> {
        assert_eq!(v_int(1).div(&v_int(2))?, v_int(0));
        assert_eq!(v_int(1).div(&v_float(2.))?, v_float(0.5));
        assert_eq!(v_float(1.).div(&v_int(2))?, v_float(0.5));
        assert_eq!(v_float(1.).div(&v_float(2.))?, v_float(0.5));
        Ok(())
    }

    #[test]
    fn test_modulus() {
        assert_eq!(v_int(1).modulus(&v_int(2)), Ok(v_int(1)));
        assert_eq!(v_int(1).modulus(&v_float(2.)), Ok(v_float(1.)));
        assert_eq!(v_float(1.).modulus(&v_int(2)), Ok(v_float(1.)));
        assert_eq!(v_float(1.).modulus(&v_float(2.)), Ok(v_float(1.)));
        assert_eq!(v_str("moop").modulus(&v_int(2)), Ok(v_err(E_TYPE)));
    }

    #[test]
    fn test_pow() {
        assert_eq!(v_int(1).pow(&v_int(2)), Ok(v_int(1)));
        assert_eq!(v_int(2).pow(&v_int(2)), Ok(v_int(4)));
        assert_eq!(v_int(2).pow(&v_float(2.)), Ok(v_float(4.)));
        assert_eq!(v_float(2.).pow(&v_int(2)), Ok(v_float(4.)));
        assert_eq!(v_float(2.).pow(&v_float(2.)), Ok(v_float(4.)));
    }

    #[test]
    fn test_negative() {
        assert_eq!(v_int(1).negative(), Ok(v_int(-1)));
        assert_eq!(v_float(1.).negative(), Ok(v_float(-1.0)));
    }

    #[test]
    fn test_eq() {
        assert_eq!(v_int(1), v_int(1));
        assert_eq!(v_float(1.), v_float(1.));
        assert_eq!(v_str("a"), v_str("a"));
        assert_eq!(v_str("a"), v_str("A"));
        assert_eq!(v_list(&[v_int(1), v_int(2)]), v_list(&[v_int(1), v_int(2)]));
        assert_eq!(v_obj(1), v_obj(1));
        assert_eq!(v_err(E_TYPE), v_err(E_TYPE));
    }

    #[test]
    fn test_ne() {
        assert_ne!(v_int(1), v_int(2));
        assert_ne!(v_float(1.), v_float(2.));
        assert_ne!(v_str("a"), v_str("b"));
        assert_ne!(v_list(&[v_int(1), v_int(2)]), v_list(&[v_int(1), v_int(3)]));
        assert_ne!(v_obj(1), v_obj(2));
        assert_ne!(v_err(E_TYPE), v_err(E_RANGE));
    }

    #[test]
    fn test_lt() {
        assert!(v_int(1) < v_int(2));
        assert!(v_float(1.) < v_float(2.));
        assert!(v_str("a") < v_str("b"));
        assert!(v_obj(1) < v_obj(2));
        assert!(v_err(E_TYPE) < v_err(E_RANGE));
    }

    #[test]
    fn test_le() {
        assert!(v_int(1) <= v_int(2));
        assert!(v_float(1.) <= v_float(2.));
        assert!(v_str("a") <= v_str("b"));
        assert!(v_obj(1) <= v_obj(2));
        assert!(v_err(E_TYPE) <= v_err(E_RANGE));
    }

    #[test]
    fn test_gt() {
        assert!(v_int(2) > v_int(1));
        assert!(v_float(2.) > v_float(1.));
        assert!(v_str("b") > v_str("a"));
        assert!(v_obj(2) > v_obj(1));
        assert!(v_err(E_RANGE) > v_err(E_TYPE));
    }

    #[test]
    fn test_ge() {
        assert!(v_int(2) >= v_int(1));
        assert!(v_float(2.) >= v_float(1.));
        assert!(v_str("b") >= v_str("a"));
        assert!(v_obj(2) >= v_obj(1));
        assert!(v_err(E_RANGE) >= v_err(E_TYPE));
    }
}
