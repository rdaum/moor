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

//! Builtin functions for numeric operations, mathematical functions, and random number generation.

use rand::Rng;

use moor_compiler::offset_for_builtin;
use moor_var::{E_ARGS, E_INVARG, E_TYPE, Sequence, Var, Variant, v_float, v_int, v_str};

use crate::vm::builtins::{BfCallState, BfErr, BfRet, BfRet::Ret, BuiltinFunction};

/// Usage: `num abs(num x)`
/// Returns the absolute value of x. If x is negative, the result is -x; otherwise the result is x.
/// The argument can be either integer or floating-point; the result is of the same type.
fn bf_abs(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("abs() takes 1 argument")));
    }

    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(Ret(v_int(i.abs()))),
        Variant::Float(f) => Ok(Ret(v_float(f.abs()))),
        _ => Err(BfErr::ErrValue(E_TYPE.msg("abs() takes a number"))),
    }
}

/// Usage: `num min(num x, ...)`
/// Returns the smallest of its arguments. All arguments must be numbers of the same type
/// (i.e., either integer or floating-point); otherwise E_TYPE is raised.
fn bf_min(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("min() takes at least 1 argument"),
        ));
    }
    let expected_type = bf_args.args[0].type_code();
    let mut minimum = bf_args.args[0].clone();
    for arg in bf_args.args.iter() {
        if arg.type_code() != expected_type {
            return Err(BfErr::ErrValue(E_TYPE.msg("min() takes numbers")));
        }
        if arg.lt(&minimum) {
            minimum = arg.clone();
        }
    }
    Ok(Ret(minimum))
}

/// Usage: `num max(num x, ...)`
/// Returns the largest of its arguments. All arguments must be numbers of the same type
/// (i.e., either integer or floating-point); otherwise E_TYPE is raised.
fn bf_max(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("max() takes at least 1 argument"),
        ));
    }
    let expected_type = bf_args.args[0].type_code();
    let mut maximum = bf_args.args[0].clone();
    for arg in bf_args.args.iter() {
        if arg.type_code() != expected_type {
            return Err(BfErr::ErrValue(E_TYPE.msg("max() takes numbers")));
        }
        if arg.gt(&maximum) {
            maximum = arg.clone();
        }
    }
    Ok(Ret(maximum))
}

/// Usage: `int random([int mod [, int range]])`
/// Returns a random integer. With no arguments, returns a random integer from 1 to
/// i64::MAX. With one argument, returns a random integer from 1
/// to mod (inclusive). With two arguments, returns a random integer from mod to range
/// (inclusive). Raises E_INVARG if the range is invalid (e.g., mod > range or mod < 1).
fn bf_random(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 2 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("random() takes 0, 1, or 2 arguments"),
        ));
    }

    let mut rng = rand::rng();

    match bf_args.args.len() {
        0 => Ok(Ret(v_int(rng.random_range(1..=i64::MAX)))),
        1 => {
            let Variant::Int(max) = bf_args.args[0].variant() else {
                return Err(BfErr::Code(E_TYPE));
            };
            if max < 1 {
                return Err(BfErr::Code(E_INVARG));
            }
            Ok(Ret(v_int(rng.random_range(1..=max))))
        }
        2 => {
            let Variant::Int(min) = bf_args.args[0].variant() else {
                return Err(BfErr::Code(E_TYPE));
            };
            let Variant::Int(max) = bf_args.args[1].variant() else {
                return Err(BfErr::Code(E_TYPE));
            };
            if min < 1 || max < min {
                return Err(BfErr::Code(E_INVARG));
            }
            Ok(Ret(v_int(rng.random_range(min..=max))))
        }
        _ => unreachable!(),
    }
}

/// Usage: `str floatstr(float x, int precision [, bool scientific])`
/// Converts x into a string with more control than tostr() or toliteral(). Precision is the
/// number of digits to appear to the right of the decimal point. If scientific is false or
/// not provided, the result is in the form "MMMMMMM.DDDDDD". If scientific is true, the
/// result is in the form "M.DDDDDDe+EEE".
fn bf_floatstr(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("floatstr() takes 2 or 3 arguments"),
        ));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("floatstr() first argument must be a float"),
            ));
        }
    };

    let precision = match &bf_args.args[1].variant() {
        Variant::Int(i) if *i > 0 => *i as usize,
        _ => {
            return Err(BfErr::ErrValue(
                E_TYPE.msg("floatstr() second argument must be a positive integer"),
            ));
        }
    };

    let scientific = bf_args.args.len() == 3 && bf_args.args[2].is_true();

    let mut s = format!("{x:.precision$}");
    if scientific {
        s = format!("{x:e}");
    }

    Ok(Ret(v_str(s.as_str())))
}

/// Internal helper to extract numeric value from Var.
fn numeric_arg(arg: &Var) -> Result<f64, BfErr> {
    let x = match arg.variant() {
        Variant::Int(i) => i as f64,
        Variant::Float(f) => f,
        _ => return Err(BfErr::ErrValue(E_TYPE.msg("non-numeric argument"))),
    };

    Ok(x)
}

/// Macro for creating simple single-argument math functions that take a numeric argument
/// and return a float result. Used for basic trigonometric and mathematical functions.
macro_rules! math_fn {
    ($doc:expr, $fn_name:ident, $builtin_name:expr, $math_op:expr) => {
        #[doc = $doc]
        fn $fn_name(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
            if bf_args.args.len() != 1 {
                return Err(BfErr::ErrValue(
                    E_ARGS.msg(concat!($builtin_name, "() takes 1 argument")),
                ));
            }

            let x = numeric_arg(&bf_args.args[0])?;
            Ok(Ret(v_float($math_op(x))))
        }
    };
}

/// Macro for creating math functions with domain validation (e.g., sqrt, log).
/// Used for functions that have restricted input domains and need validation.
macro_rules! math_fn_with_validation {
    ($doc:expr, $fn_name:ident, $builtin_name:expr, $math_op:expr, $validator:expr, $error_msg:expr) => {
        #[doc = $doc]
        fn $fn_name(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
            if bf_args.args.len() != 1 {
                return Err(BfErr::ErrValue(
                    E_ARGS.msg(concat!($builtin_name, "() takes 1 argument")),
                ));
            }

            let x = numeric_arg(&bf_args.args[0])?;

            if !$validator(x) {
                return Err(BfErr::ErrValue(E_ARGS.msg($error_msg)));
            }

            Ok(Ret(v_float($math_op(x))))
        }
    };
}

// Basic trig functions (angles in radians)
math_fn!(
    "Usage: `float sin(num x)`\nReturns the sine of x (in radians).",
    bf_sin,
    "sin",
    |x: f64| x.sin()
);
math_fn!(
    "Usage: `float cos(num x)`\nReturns the cosine of x (in radians).",
    bf_cos,
    "cos",
    |x: f64| x.cos()
);
math_fn!(
    "Usage: `float tan(num x)`\nReturns the tangent of x (in radians).",
    bf_tan,
    "tan",
    |x: f64| x.tan()
);

// Functions with domain validation
math_fn_with_validation!(
    "Usage: `float sqrt(num x)`\nReturns the square root of x. Raises E_INVARG if x is negative.",
    bf_sqrt,
    "sqrt",
    |x: f64| x.sqrt(),
    |x: f64| x >= 0.0,
    "sqrt() takes a non-negative number"
);
math_fn_with_validation!(
    "Usage: `float asin(num x)`\nReturns the arc-sine of x, in the range [-pi/2..pi/2]. Raises E_INVARG if x is outside [-1.0..1.0].",
    bf_asin,
    "asin",
    |x: f64| x.asin(),
    |x: f64| (-1.0..=1.0).contains(&x),
    "asin() takes a number between -1 and 1"
);
math_fn_with_validation!(
    "Usage: `float acos(num x)`\nReturns the arc-cosine of x, in the range [0..pi]. Raises E_INVARG if x is outside [-1.0..1.0].",
    bf_acos,
    "acos",
    |x: f64| x.acos(),
    |x: f64| (-1.0..=1.0).contains(&x),
    "acos() takes a number between -1 and 1"
);

/// Usage: `float atan(num y [, num x])`
/// Returns the arc-tangent of y in the range [-pi/2..pi/2] if one argument is given.
/// If x is also provided, returns atan2(y, x) in the range [-pi..pi].
fn bf_atan(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(BfErr::ErrValue(E_ARGS.msg("atan() takes 1 or 2 arguments")));
    }

    if bf_args.args.len() == 1 {
        // Single argument: regular atan
        let x = numeric_arg(&bf_args.args[0])?;
        Ok(Ret(v_float(x.atan())))
    } else {
        // Two arguments: atan2(y, x) - args[0] is y, args[1] is x
        let y = numeric_arg(&bf_args.args[0])?;
        let x = numeric_arg(&bf_args.args[1])?;
        Ok(Ret(v_float(y.atan2(x))))
    }
}

// Hyperbolic functions
math_fn!(
    "Usage: `float sinh(num x)`\nReturns the hyperbolic sine of x.",
    bf_sinh,
    "sinh",
    |x: f64| x.sinh()
);
math_fn!(
    "Usage: `float cosh(num x)`\nReturns the hyperbolic cosine of x.",
    bf_cosh,
    "cosh",
    |x: f64| x.cosh()
);
math_fn!(
    "Usage: `float tanh(num x)`\nReturns the hyperbolic tangent of x.",
    bf_tanh,
    "tanh",
    |x: f64| x.tanh()
);

// Exponential and logarithmic functions
math_fn!(
    "Usage: `float exp(num x)`\nReturns e raised to the power of x.",
    bf_exp,
    "exp",
    |x: f64| x.exp()
);
math_fn_with_validation!(
    "Usage: `float log(num x)`\nReturns the natural logarithm of x. Raises E_INVARG if x is not positive.",
    bf_log,
    "log",
    |x: f64| x.ln(),
    |x: f64| x > 0.0,
    "log() takes a positive number"
);
math_fn_with_validation!(
    "Usage: `float log10(num x)`\nReturns the base-10 logarithm of x. Raises E_INVARG if x is not positive.",
    bf_log10,
    "log10",
    |x: f64| x.log10(),
    |x: f64| x > 0.0,
    "log10() takes a positive number"
);

// Rounding functions
math_fn!(
    "Usage: `float ceil(num x)`\nReturns the smallest integer not less than x, as a float.",
    bf_ceil,
    "ceil",
    |x: f64| x.ceil()
);
math_fn!(
    "Usage: `float floor(num x)`\nReturns the largest integer not greater than x, as a float.",
    bf_floor,
    "floor",
    |x: f64| x.floor()
);
math_fn!(
    "Usage: `float trunc(num x)`\nReturns the integer part of x, as a float. For negative x, equivalent to ceil(); otherwise equivalent to floor().",
    bf_trunc,
    "trunc",
    |x: f64| x.trunc()
);

pub(crate) fn register_bf_num(builtins: &mut [BuiltinFunction]) {
    builtins[offset_for_builtin("abs")] = bf_abs;
    builtins[offset_for_builtin("min")] = bf_min;
    builtins[offset_for_builtin("max")] = bf_max;
    builtins[offset_for_builtin("random")] = bf_random;
    builtins[offset_for_builtin("floatstr")] = bf_floatstr;
    builtins[offset_for_builtin("sqrt")] = bf_sqrt;
    builtins[offset_for_builtin("sin")] = bf_sin;
    builtins[offset_for_builtin("cos")] = bf_cos;
    builtins[offset_for_builtin("tan")] = bf_tan;
    builtins[offset_for_builtin("asin")] = bf_asin;
    builtins[offset_for_builtin("acos")] = bf_acos;
    builtins[offset_for_builtin("atan")] = bf_atan;
    builtins[offset_for_builtin("sinh")] = bf_sinh;
    builtins[offset_for_builtin("cosh")] = bf_cosh;
    builtins[offset_for_builtin("tanh")] = bf_tanh;
    builtins[offset_for_builtin("exp")] = bf_exp;
    builtins[offset_for_builtin("log")] = bf_log;
    builtins[offset_for_builtin("log10")] = bf_log10;
    builtins[offset_for_builtin("ceil")] = bf_ceil;
    builtins[offset_for_builtin("floor")] = bf_floor;
    builtins[offset_for_builtin("trunc")] = bf_trunc;
}
