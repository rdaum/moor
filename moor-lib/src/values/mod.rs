#![allow(non_camel_case_types, non_snake_case)]

use int_enum::IntEnum;

pub mod error;
pub mod objid;
pub mod var;
pub mod variant;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum)]
pub enum VarType {
    TYPE_INT = 0,
    TYPE_OBJ = 1,
    TYPE_STR = 2,
    TYPE_ERR = 3,
    TYPE_LIST = 4,  /* user-visible */
    TYPE_CLEAR = 5, /* in clear properties' value slot */
    TYPE_NONE = 6,  /* in uninitialized MOO variables */
    TYPE_LABEL = 7,
    TYPE_FLOAT = 9, /* floating-point number; user-visible */
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::values::error::Error;
    use crate::values::error::Error::{E_RANGE, E_TYPE};
    use crate::values::var::{v_err, v_float, v_int, v_list, v_obj, v_str};

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
    fn test_index() {
        assert_eq!(v_list(vec![v_int(1), v_int(2)]).index(0), Ok(v_int(1)));
        assert_eq!(v_list(vec![v_int(1), v_int(2)]).index(1), Ok(v_int(2)));
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).index(2),
            Ok(v_err(E_RANGE))
        );
        assert_eq!(v_str("ab").index(0), Ok(v_str("a")));
        assert_eq!(v_str("ab").index(1), Ok(v_str("b")));
        assert_eq!(v_str("ab").index(2), Ok(v_err(E_RANGE)));
    }

    #[test]
    fn test_eq() {
        assert_eq!(v_int(1), v_int(1));
        assert_eq!(v_float(1.), v_float(1.));
        assert_eq!(v_str("a"), v_str("a"));
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]),
            v_list(vec![v_int(1), v_int(2)])
        );
        assert_eq!(v_obj(1), v_obj(1));
        assert_eq!(v_err(E_TYPE), v_err(E_TYPE));
    }

    #[test]
    fn test_ne() {
        assert_ne!(v_int(1), v_int(2));
        assert_ne!(v_float(1.), v_float(2.));
        assert_ne!(v_str("a"), v_str("b"));
        assert_ne!(
            v_list(vec![v_int(1), v_int(2)]),
            v_list(vec![v_int(1), v_int(3)])
        );
        assert_ne!(v_obj(1), v_obj(2));
        assert_ne!(v_err(E_TYPE), v_err(E_RANGE));
    }

    #[test]
    fn test_lt() {
        assert!(v_int(1) < v_int(2));
        assert!(v_float(1.) < v_float(2.));
        assert!(v_str("a") < v_str("b"));
        assert!(v_list(vec![v_int(1), v_int(2)]) < v_list(vec![v_int(1), v_int(3)]));
        assert!(v_obj(1) < v_obj(2));
        assert!(v_err(E_TYPE) < v_err(E_RANGE));
    }

    #[test]
    fn test_le() {
        assert!(v_int(1) <= v_int(2));
        assert!(v_float(1.) <= v_float(2.));
        assert!(v_str("a") <= v_str("b"));
        assert!(v_list(vec![v_int(1), v_int(2)]) <= v_list(vec![v_int(1), v_int(3)]));
        assert!(v_obj(1) <= v_obj(2));
        assert!(v_err(E_TYPE) <= v_err(E_RANGE));
    }

    #[test]
    fn test_gt() {
        assert!(v_int(2) > v_int(1));
        assert!(v_float(2.) > v_float(1.));
        assert!(v_str("b") > v_str("a"));
        assert!(v_list(vec![v_int(1), v_int(3)]) > v_list(vec![v_int(1), v_int(2)]));
        assert!(v_obj(2) > v_obj(1));
        assert!(v_err(E_RANGE) > v_err(E_TYPE));
    }

    #[test]
    fn test_ge() {
        assert!(v_int(2) >= v_int(1));
        assert!(v_float(2.) >= v_float(1.));
        assert!(v_str("b") >= v_str("a"));
        assert!(v_list(vec![v_int(1), v_int(3)]) >= v_list(vec![v_int(1), v_int(2)]));
        assert!(v_obj(2) >= v_obj(1));
        assert!(v_err(E_RANGE) >= v_err(E_TYPE));
    }

    #[test]
    fn test_partial_cmp() {
        assert_eq!(v_int(1).partial_cmp(&v_int(1)), Some(Ordering::Equal));
        assert_eq!(v_float(1.).partial_cmp(&v_float(1.)), Some(Ordering::Equal));
        assert_eq!(v_str("a").partial_cmp(&v_str("a")), Some(Ordering::Equal));
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).partial_cmp(&v_list(vec![v_int(1), v_int(2)])),
            Some(Ordering::Equal)
        );
        assert_eq!(v_obj(1).partial_cmp(&v_obj(1)), Some(Ordering::Equal));
        assert_eq!(
            v_err(E_TYPE).partial_cmp(&v_err(E_TYPE)),
            Some(Ordering::Equal)
        );

        assert_eq!(v_int(1).partial_cmp(&v_int(2)), Some(Ordering::Less));
        assert_eq!(v_float(1.).partial_cmp(&v_float(2.)), Some(Ordering::Less));
        assert_eq!(v_str("a").partial_cmp(&v_str("b")), Some(Ordering::Less));
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).partial_cmp(&v_list(vec![v_int(1), v_int(3)])),
            Some(Ordering::Less)
        );
        assert_eq!(v_obj(1).partial_cmp(&v_obj(2)), Some(Ordering::Less));
        assert_eq!(
            v_err(E_TYPE).partial_cmp(&v_err(E_RANGE)),
            Some(Ordering::Less)
        );

        assert_eq!(v_int(2).partial_cmp(&v_int(1)), Some(Ordering::Greater));
        assert_eq!(
            v_float(2.).partial_cmp(&v_float(1.)),
            Some(Ordering::Greater)
        );
        assert_eq!(v_str("b").partial_cmp(&v_str("a")), Some(Ordering::Greater));
        assert_eq!(
            v_list(vec![v_int(1), v_int(3)]).partial_cmp(&v_list(vec![v_int(1), v_int(2)])),
            Some(Ordering::Greater)
        );
        assert_eq!(v_obj(2).partial_cmp(&v_obj(1)), Some(Ordering::Greater));
        assert_eq!(
            v_err(E_RANGE).partial_cmp(&v_err(E_TYPE)),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn test_cmp() {
        assert_eq!(v_int(1).cmp(&v_int(1)), Ordering::Equal);
        assert_eq!(v_float(1.).cmp(&v_float(1.)), Ordering::Equal);
        assert_eq!(v_str("a").cmp(&v_str("a")), Ordering::Equal);
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).cmp(&v_list(vec![v_int(1), v_int(2)])),
            Ordering::Equal
        );
        assert_eq!(v_obj(1).cmp(&v_obj(1)), Ordering::Equal);
        assert_eq!(v_err(E_TYPE).cmp(&v_err(E_TYPE)), Ordering::Equal);

        assert_eq!(v_int(1).cmp(&v_int(2)), Ordering::Less);
        assert_eq!(v_float(1.).cmp(&v_float(2.)), Ordering::Less);
        assert_eq!(v_str("a").cmp(&v_str("b")), Ordering::Less);
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).cmp(&v_list(vec![v_int(1), v_int(3)])),
            Ordering::Less
        );
        assert_eq!(v_obj(1).cmp(&v_obj(2)), Ordering::Less);
        assert_eq!(v_err(E_TYPE).cmp(&v_err(E_RANGE)), Ordering::Less);

        assert_eq!(v_int(2).cmp(&v_int(1)), Ordering::Greater);
        assert_eq!(v_float(2.).cmp(&v_float(1.)), Ordering::Greater);
        assert_eq!(v_str("b").cmp(&v_str("a")), Ordering::Greater);
        assert_eq!(
            v_list(vec![v_int(1), v_int(3)]).cmp(&v_list(vec![v_int(1), v_int(2)])),
            Ordering::Greater
        );
        assert_eq!(v_obj(2).cmp(&v_obj(1)), Ordering::Greater);
        assert_eq!(v_err(E_RANGE).cmp(&v_err(E_TYPE)), Ordering::Greater);
    }

    #[test]
    fn test_partial_ord() {
        assert_eq!(v_int(1).partial_cmp(&v_int(1)).unwrap(), Ordering::Equal);
        assert_eq!(
            v_float(1.).partial_cmp(&v_float(1.)).unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            v_str("a").partial_cmp(&v_str("a")).unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)])
                .partial_cmp(&v_list(vec![v_int(1), v_int(2)]))
                .unwrap(),
            Ordering::Equal
        );
        assert_eq!(v_obj(1).partial_cmp(&v_obj(1)).unwrap(), Ordering::Equal);
        assert_eq!(
            v_err(E_TYPE).partial_cmp(&v_err(E_TYPE)).unwrap(),
            Ordering::Equal
        );

        assert_eq!(v_int(1).partial_cmp(&v_int(2)).unwrap(), Ordering::Less);
        assert_eq!(
            v_float(1.).partial_cmp(&v_float(2.)).unwrap(),
            Ordering::Less
        );
        assert_eq!(v_str("a").partial_cmp(&v_str("b")).unwrap(), Ordering::Less);
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)])
                .partial_cmp(&v_list(vec![v_int(1), v_int(3)]))
                .unwrap(),
            Ordering::Less
        );
        assert_eq!(v_obj(1).partial_cmp(&v_obj(2)).unwrap(), Ordering::Less);
        assert_eq!(
            v_err(E_TYPE).partial_cmp(&v_err(E_RANGE)).unwrap(),
            Ordering::Less
        );

        assert_eq!(v_int(2).partial_cmp(&v_int(1)).unwrap(), Ordering::Greater);
        assert_eq!(
            v_float(2.).partial_cmp(&v_float(1.)).unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            v_str("b").partial_cmp(&v_str("a")).unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            v_list(vec![v_int(1), v_int(3)])
                .partial_cmp(&v_list(vec![v_int(1), v_int(2)]))
                .unwrap(),
            Ordering::Greater
        );
        assert_eq!(v_obj(2).partial_cmp(&v_obj(1)).unwrap(), Ordering::Greater);
        assert_eq!(
            v_err(E_RANGE).partial_cmp(&v_err(E_TYPE)).unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn test_ord() {
        assert_eq!(v_int(1).cmp(&v_int(1)), Ordering::Equal);
        assert_eq!(v_float(1.).cmp(&v_float(1.)), Ordering::Equal);
        assert_eq!(v_str("a").cmp(&v_str("a")), Ordering::Equal);
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).cmp(&v_list(vec![v_int(1), v_int(2)])),
            Ordering::Equal
        );
        assert_eq!(v_obj(1).cmp(&v_obj(1)), Ordering::Equal);
        assert_eq!(v_err(E_TYPE).cmp(&v_err(E_TYPE)), Ordering::Equal);

        assert_eq!(v_int(1).cmp(&v_int(2)), Ordering::Less);
        assert_eq!(v_float(1.).cmp(&v_float(2.)), Ordering::Less);
        assert_eq!(v_str("a").cmp(&v_str("b")), Ordering::Less);
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).cmp(&v_list(vec![v_int(1), v_int(3)])),
            Ordering::Less
        );
        assert_eq!(v_obj(1).cmp(&v_obj(2)), Ordering::Less);
        assert_eq!(v_err(E_TYPE).cmp(&v_err(E_RANGE)), Ordering::Less);

        assert_eq!(v_int(2).cmp(&v_int(1)), Ordering::Greater);
        assert_eq!(v_float(2.).cmp(&v_float(1.)), Ordering::Greater);
        assert_eq!(v_str("b").cmp(&v_str("a")), Ordering::Greater);
        assert_eq!(
            v_list(vec![v_int(1), v_int(3)]).cmp(&v_list(vec![v_int(1), v_int(2)])),
            Ordering::Greater
        );
        assert_eq!(v_obj(2).cmp(&v_obj(1)), Ordering::Greater);
        assert_eq!(v_err(E_RANGE).cmp(&v_err(E_TYPE)), Ordering::Greater);
    }

    #[test]
    fn test_is_true() {
        assert!(v_int(1).is_true());
        assert!(v_float(1.).is_true());
        assert!(v_str("a").is_true());
        assert!(v_list(vec![v_int(1), v_int(2)]).is_true());
        assert!(!v_obj(1).is_true());
        assert!(!v_err(E_TYPE).is_true());
    }

    #[test]
    fn test_listrangeset() {
        let base = v_list(vec![v_int(1), v_int(2), v_int(3), v_int(4)]);

        // {1,2,3,4}[1..2] = {"a", "b", "c"} => {1, "a", "b", "c", 4}
        let value = v_list(vec![v_str("a"), v_str("b"), v_str("c")]);
        let expected = v_list(vec![v_int(1), v_str("a"), v_str("b"), v_str("c"), v_int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);

        // {1,2,3,4}[1..2] = {"a"} => {1, "a", 4}
        let value = v_list(vec![v_str("a")]);
        let expected = v_list(vec![v_int(1), v_str("a"), v_int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);

        // {1,2,3,4}[1..2] = {} => {1,4}
        let value = v_list(vec![]);
        let expected = v_list(vec![v_int(1), v_int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);

        // {1,2,3,4}[1..2] = {"a", "b"} => {1, "a", "b", 4}
        let value = v_list(vec![v_str("a"), v_str("b")]);
        let expected = v_list(vec![v_int(1), v_str("a"), v_str("b"), v_int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);
    }

    #[test]
    fn test_strrangeset() {
        // Test interior insertion
        let base = v_str("12345");
        let value = v_str("abc");
        let expected = v_str("1abc45");
        let result = base.rangeset(value, 2, 3);
        assert_eq!(result, Ok(expected));

        // Test interior replacement
        let base = v_str("12345");
        let value = v_str("ab");
        let expected = v_str("1ab45");
        let result = base.rangeset(value, 2, 3);
        assert_eq!(result, Ok(expected));

        // Test interior deletion
        let base = v_str("12345");
        let value = v_str("");
        let expected = v_str("145");
        let result = base.rangeset(value, 2, 3);
        assert_eq!(result, Ok(expected));

        // Test interior subtraction
        let base = v_str("12345");
        let value = v_str("z");
        let expected = v_str("1z45");
        let result = base.rangeset(value, 2, 3);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_rangeset_check_negative() {
        // Test negative cases for strings
        let base = v_str("abcdef");
        let instr = v_str("ghi");
        assert_eq!(base.rangeset(instr.clone(), 1, 0), Ok(v_err(E_RANGE)));
        assert_eq!(base.rangeset(instr.clone(), 0, 3), Ok(v_err(E_RANGE)));
        assert_eq!(base.rangeset(instr.clone(), 2, 7), Ok(v_err(E_RANGE)));
        assert_eq!(base.rangeset(instr, 1, 100), Ok(v_err(E_RANGE)));

        // Test negative cases for lists
        let base = v_list(vec![v_int(1), v_int(2), v_int(3), v_int(4)]);
        let instr = v_list(vec![v_int(5), v_int(6), v_int(7)]);
        assert_eq!(base.rangeset(instr.clone(), 0, 2), Ok(v_err(E_RANGE)));
        assert_eq!(base.rangeset(instr.clone(), 1, 5), Ok(v_err(E_RANGE)));
        assert_eq!(base.rangeset(instr.clone(), 2, 7), Ok(v_err(E_RANGE)));
        assert_eq!(base.rangeset(instr, 1, 100), Ok(v_err(E_RANGE)));
    }

    #[test]
    fn test_range() -> Result<(), Error> {
        // test on integer list
        let int_list = v_list(vec![1.into(), 2.into(), 3.into(), 4.into(), 5.into()]);
        assert_eq!(
            int_list.range(2, 4)?,
            v_list(vec![2.into(), 3.into(), 4.into()])
        );

        // test on string
        let string = v_str("hello world");
        assert_eq!(string.range(2, 7)?, v_str("ello w"));

        // range with upper higher than lower, moo returns empty list for this (!)
        let empty_list = v_list(vec![]);
        assert_eq!(empty_list.range(1, 0), Ok(v_list(vec![])));
        // test on out of range
        let int_list = v_list(vec![1.into(), 2.into(), 3.into()]);
        assert_eq!(int_list.range(2, 4), Ok(v_err(E_RANGE)));
        // test on type mismatch
        let var_int = v_int(10);
        assert_eq!(var_int.range(1, 5), Ok(v_err(E_TYPE)));

        Ok(())
    }
}
