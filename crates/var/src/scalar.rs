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
    Error,
    error::ErrorCode::{E_INVARG, E_TYPE},
    variant::{Var, v_err, v_error, v_float, v_int},
    variant::Variant,
};
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
                    paste! { l.[<checked_ $op>](r).map(v_int).ok_or_else( || E_INVARG.into()) }
                }
                (Variant::Float(l), Variant::Int(r)) => {
                    Ok(v_float(l.to_f64().unwrap().$op(r as f64)))
                }
                (Variant::Int(l), Variant::Float(r)) => {
                    Ok(v_float((l as f64).$op(r.to_f64().unwrap())))
                }
                (_, _) => Ok(v_error(E_TYPE.with_msg(|| {
                    format!(
                        "Cannot {} type {} and {}",
                        stringify!($op),
                        self.type_code().to_literal(),
                        v.type_code().to_literal()
                    )
                }))),
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
            (Variant::Int(l), Variant::Int(r)) => l
                .checked_add(r)
                .map(v_int)
                .ok_or_else(|| E_INVARG.msg("Integer overflow")),
            (Variant::Float(l), Variant::Int(r)) => Ok(v_float(l.to_f64().unwrap() + (r as f64))),
            (Variant::Int(l), Variant::Float(r)) => Ok(v_float(l as f64 + r.to_f64().unwrap())),
            (Variant::Str(s), Variant::Str(r)) => Ok(s.str_append(r)),
            (_, _) => Ok(v_error(E_TYPE.with_msg(|| {
                format!(
                    "Cannot add type {} and {}",
                    self.type_code().to_literal(),
                    v.type_code().to_literal()
                )
            }))),
        }
    }

    pub fn negative(&self) -> Result<Self, Error> {
        match self.variant() {
            Variant::Int(l) => l
                .checked_neg()
                .map(v_int)
                .ok_or_else(|| E_INVARG.msg("Integer underflow")),
            Variant::Float(f) => Ok(v_float(f.neg())),
            _ => Ok(v_error(E_TYPE.with_msg(|| {
                format!("Cannot negate type {}", self.type_code().to_literal())
            }))),
        }
    }

    pub fn modulus(&self, v: &Self) -> Result<Self, Error> {
        match (self.variant(), v.variant()) {
            (Variant::Float(l), Variant::Float(r)) => Ok(v_float(l % r)),
            (Variant::Int(l), Variant::Int(r)) => l
                .checked_rem(r)
                .map(v_int)
                .ok_or_else(|| E_INVARG.with_msg(|| "Integer division by zero".to_string())),
            (Variant::Float(l), Variant::Int(r)) => Ok(v_float(l.to_f64().unwrap() % (r as f64))),
            (Variant::Int(l), Variant::Float(r)) => Ok(v_float(l as f64 % (r.to_f64().unwrap()))),
            (_, _) => Ok(v_error(E_TYPE.with_msg(|| {
                format!(
                    "Cannot modulus type {} and {}",
                    self.type_code().to_literal(),
                    v.type_code().to_literal()
                )
            }))),
        }
    }

    pub fn pow(&self, v: &Self) -> Result<Self, Error> {
        match (self.variant(), v.variant()) {
            (Variant::Float(l), Variant::Float(r)) => Ok(v_float(l.powf(r))),
            (Variant::Int(l), Variant::Int(r)) => {
                let r = u32::try_from(r).map_err(|_| E_INVARG.msg("Invalid argument for pow"))?;
                l.checked_pow(r).map(v_int).ok_or_else(|| E_INVARG.into())
            }
            (Variant::Float(l), Variant::Int(r)) => Ok(v_float(l.powi(r as i32))),
            (Variant::Int(l), Variant::Float(r)) => Ok(v_float((l as f64).powf(r))),
            (_, _) => Ok(v_err(E_TYPE)),
        }
    }

    pub fn is_sysobj(&self) -> bool {
        matches!(self.variant(), Variant::Obj(o) if o.is_sysobj())
    }

    pub fn bitand(&self, v: &Self) -> Result<Self, Error> {
        let (Variant::Int(l), Variant::Int(r)) = (self.variant(), v.variant()) else {
            return Ok(v_error(E_TYPE.with_msg(|| {
                format!(
                    "Cannot bitwise AND type {} and {}",
                    self.type_code().to_literal(),
                    v.type_code().to_literal()
                )
            })));
        };
        Ok(v_int(l & r))
    }

    pub fn bitor(&self, v: &Self) -> Result<Self, Error> {
        let (Variant::Int(l), Variant::Int(r)) = (self.variant(), v.variant()) else {
            return Ok(v_error(E_TYPE.with_msg(|| {
                format!(
                    "Cannot bitwise OR type {} and {}",
                    self.type_code().to_literal(),
                    v.type_code().to_literal()
                )
            })));
        };
        Ok(v_int(l | r))
    }

    pub fn bitxor(&self, v: &Self) -> Result<Self, Error> {
        let (Variant::Int(l), Variant::Int(r)) = (self.variant(), v.variant()) else {
            return Ok(v_error(E_TYPE.with_msg(|| {
                format!(
                    "Cannot bitwise XOR type {} and {}",
                    self.type_code().to_literal(),
                    v.type_code().to_literal()
                )
            })));
        };
        Ok(v_int(l ^ r))
    }

    pub fn bitshl(&self, v: &Self) -> Result<Self, Error> {
        let (Variant::Int(l), Variant::Int(r)) = (self.variant(), v.variant()) else {
            return Ok(v_error(E_TYPE.with_msg(|| {
                format!(
                    "Cannot left shift type {} by {}",
                    self.type_code().to_literal(),
                    v.type_code().to_literal()
                )
            })));
        };
        if !(0..=63).contains(&r) {
            return Ok(v_error(E_INVARG.msg("Invalid shift amount")));
        }
        l.checked_shl(r as u32)
            .map(v_int)
            .ok_or_else(|| E_INVARG.msg("Integer overflow in left shift"))
    }

    pub fn bitshr(&self, v: &Self) -> Result<Self, Error> {
        let (Variant::Int(l), Variant::Int(r)) = (self.variant(), v.variant()) else {
            return Ok(v_error(E_TYPE.with_msg(|| {
                format!(
                    "Cannot right shift type {} by {}",
                    self.type_code().to_literal(),
                    v.type_code().to_literal()
                )
            })));
        };
        if !(0..=63).contains(&r) {
            return Ok(v_error(E_INVARG.msg("Invalid shift amount")));
        }
        l.checked_shr(r as u32)
            .map(v_int)
            .ok_or_else(|| E_INVARG.msg("Integer overflow in right shift"))
    }

    pub fn bitlshr(&self, v: &Self) -> Result<Self, Error> {
        let (Variant::Int(l), Variant::Int(r)) = (self.variant(), v.variant()) else {
            return Ok(v_error(E_TYPE.with_msg(|| {
                format!(
                    "Cannot logical right shift type {} by {}",
                    self.type_code().to_literal(),
                    v.type_code().to_literal()
                )
            })));
        };
        if !(0..=63).contains(&r) {
            return Ok(v_error(E_INVARG.msg("Invalid shift amount")));
        }
        // Logical (unsigned) right shift: cast to u64, shift, cast back to i64
        Ok(v_int(((l as u64) >> (r as u32)) as i64))
    }

    pub fn bitnot(&self) -> Result<Self, Error> {
        let Variant::Int(l) = self.variant() else {
            return Ok(v_error(E_TYPE.with_msg(|| {
                format!(
                    "Cannot bitwise complement type {}",
                    self.type_code().to_literal()
                )
            })));
        };
        Ok(v_int(!l))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Error,
        error::ErrorCode::{E_RANGE, E_TYPE},
        variant::{v_err, v_float, v_int, v_list, v_objid, v_str},
    };

    #[test]
    fn test_truthy() {
        assert!(v_int(1).is_true());
        assert!(!v_int(0).is_true());
    }

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
        assert_eq!(v_objid(1), v_objid(1));
        assert_eq!(v_err(E_TYPE), v_err(E_TYPE));
    }

    #[test]
    fn test_ne() {
        assert_ne!(v_int(1), v_int(2));
        assert_ne!(v_float(1.), v_float(2.));
        assert_ne!(v_str("a"), v_str("b"));
        assert_ne!(v_list(&[v_int(1), v_int(2)]), v_list(&[v_int(1), v_int(3)]));
        assert_ne!(v_objid(1), v_objid(2));
        assert_ne!(v_err(E_TYPE), v_err(E_RANGE));
    }

    #[test]
    fn test_lt() {
        assert!(v_int(1) < v_int(2));
        assert!(v_float(1.) < v_float(2.));
        assert!(v_str("a") < v_str("b"));
        assert!(v_objid(1) < v_objid(2));
        assert!(v_err(E_TYPE) < v_err(E_RANGE));
    }

    #[test]
    fn test_le() {
        assert!(v_int(1) <= v_int(2));
        assert!(v_float(1.) <= v_float(2.));
        assert!(v_str("a") <= v_str("b"));
        assert!(v_objid(1) <= v_objid(2));
        assert!(v_err(E_TYPE) <= v_err(E_RANGE));
    }

    #[test]
    fn test_gt() {
        assert!(v_int(2) > v_int(1));
        assert!(v_float(2.) > v_float(1.));
        assert!(v_str("b") > v_str("a"));
        assert!(v_objid(2) > v_objid(1));
        assert!(v_err(E_RANGE) > v_err(E_TYPE));
    }

    #[test]
    fn test_ge() {
        assert!(v_int(2) >= v_int(1));
        assert!(v_float(2.) >= v_float(1.));
        assert!(v_str("b") >= v_str("a"));
        assert!(v_objid(2) >= v_objid(1));
        assert!(v_err(E_RANGE) >= v_err(E_TYPE));
    }

    #[test]
    fn test_bitand() {
        assert_eq!(v_int(5).bitand(&v_int(3)), Ok(v_int(1))); // 0101 & 0011 = 0001
        assert_eq!(v_int(12).bitand(&v_int(10)), Ok(v_int(8))); // 1100 & 1010 = 1000
        assert_eq!(v_int(0).bitand(&v_int(15)), Ok(v_int(0))); // 0000 & 1111 = 0000

        // Test with non-integers
        assert!(v_str("test").bitand(&v_int(5)).is_ok());
        assert!(v_int(5).bitand(&v_str("test")).is_ok());
    }

    #[test]
    fn test_bitor() {
        assert_eq!(v_int(5).bitor(&v_int(3)), Ok(v_int(7))); // 0101 | 0011 = 0111
        assert_eq!(v_int(12).bitor(&v_int(10)), Ok(v_int(14))); // 1100 | 1010 = 1110
        assert_eq!(v_int(0).bitor(&v_int(15)), Ok(v_int(15))); // 0000 | 1111 = 1111

        // Test with non-integers
        assert!(v_str("test").bitor(&v_int(5)).is_ok());
        assert!(v_int(5).bitor(&v_str("test")).is_ok());
    }

    #[test]
    fn test_bitxor() {
        assert_eq!(v_int(5).bitxor(&v_int(3)), Ok(v_int(6))); // 0101 ^ 0011 = 0110
        assert_eq!(v_int(12).bitxor(&v_int(10)), Ok(v_int(6))); // 1100 ^ 1010 = 0110
        assert_eq!(v_int(15).bitxor(&v_int(15)), Ok(v_int(0))); // 1111 ^ 1111 = 0000

        // Test with non-integers
        assert!(v_str("test").bitxor(&v_int(5)).is_ok());
        assert!(v_int(5).bitxor(&v_str("test")).is_ok());
    }

    #[test]
    fn test_bitshl() {
        assert_eq!(v_int(1).bitshl(&v_int(2)), Ok(v_int(4))); // 1 << 2 = 4
        assert_eq!(v_int(5).bitshl(&v_int(1)), Ok(v_int(10))); // 5 << 1 = 10
        assert_eq!(v_int(3).bitshl(&v_int(3)), Ok(v_int(24))); // 3 << 3 = 24

        // Test bounds checking
        assert!(v_int(1).bitshl(&v_int(-1)).is_ok()); // Should return error
        assert!(v_int(1).bitshl(&v_int(64)).is_ok()); // Should return error

        // Test with non-integers
        assert!(v_str("test").bitshl(&v_int(2)).is_ok());
        assert!(v_int(5).bitshl(&v_str("test")).is_ok());
    }

    #[test]
    fn test_bitshr() {
        assert_eq!(v_int(8).bitshr(&v_int(2)), Ok(v_int(2))); // 8 >> 2 = 2
        assert_eq!(v_int(10).bitshr(&v_int(1)), Ok(v_int(5))); // 10 >> 1 = 5
        assert_eq!(v_int(24).bitshr(&v_int(3)), Ok(v_int(3))); // 24 >> 3 = 3

        // Test bounds checking
        assert!(v_int(8).bitshr(&v_int(-1)).is_ok()); // Should return error
        assert!(v_int(8).bitshr(&v_int(64)).is_ok()); // Should return error

        // Test with non-integers
        assert!(v_str("test").bitshr(&v_int(2)).is_ok());
        assert!(v_int(5).bitshr(&v_str("test")).is_ok());
    }

    #[test]
    fn test_bitnot() {
        assert_eq!(v_int(5).bitnot(), Ok(v_int(!5))); // ~5 = -6 in two's complement
        assert_eq!(v_int(0).bitnot(), Ok(v_int(-1))); // ~0 = -1
        assert_eq!(v_int(-1).bitnot(), Ok(v_int(0))); // ~(-1) = 0
        assert_eq!(v_int(42).bitnot(), Ok(v_int(!42))); // ~42 = -43

        // Test with non-integers
        assert!(v_str("test").bitnot().is_ok()); // Should return error
        assert!(v_float(5.0).bitnot().is_ok()); // Should return error
    }

    #[test]
    fn test_intertype_snorgling() {
        let f = v_float(10.74107142857142);
        let i = v_int(100);
        assert!(f <= i);
    }
}
