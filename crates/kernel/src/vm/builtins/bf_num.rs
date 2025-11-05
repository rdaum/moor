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

/// MOO: `num abs(num x)`
/// Returns the absolute value of x.
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

/// MOO: `num min(num x, ...)`
/// Returns the minimum value among the arguments.
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

/// MOO: `num max(num x, ...)`
/// Returns the maximum value among the arguments.
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

/// MOO: `int random([int max])`
/// Returns a random integer. If max is given, returns 1 to max, otherwise 1 to 2147483647.
fn bf_random(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("random() takes 0 or 1 argument"),
        ));
    }

    let mut rng = rand::rng();
    if bf_args.args.is_empty() {
        Ok(Ret(v_int(rng.random_range(1..=2147483647))))
    } else {
        match &bf_args.args[0].variant() {
            Variant::Int(i) if *i > 0 => Ok(Ret(v_int(rng.random_range(1..=*i)))),
            Variant::Int(_) => Err(BfErr::Code(E_INVARG)),
            _ => Err(BfErr::Code(E_TYPE)),
        }
    }
}

/// MOO: `str floatstr(float x, int precision [, bool scientific])`
/// Converts a float to string with specified precision, optionally in scientific notation.
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
        Variant::Int(i) => *i as f64,
        Variant::Float(f) => *f,
        _ => return Err(BfErr::ErrValue(E_TYPE.msg("non-numeric argument"))),
    };

    Ok(x)
}

/// Macro for creating simple single-argument math functions that take a numeric argument
/// and return a float result. Used for basic trigonometric and mathematical functions.
macro_rules! math_fn {
    ($fn_name:ident, $builtin_name:expr, $math_op:expr) => {
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
    ($fn_name:ident, $builtin_name:expr, $math_op:expr, $validator:expr, $error_msg:expr) => {
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

// Basic trig functions
// MOO: `float sin(num x)` - Returns the sine of x (in radians).
math_fn!(bf_sin, "sin", |x: f64| x.sin());
// MOO: `float cos(num x)` - Returns the cosine of x (in radians).
math_fn!(bf_cos, "cos", |x: f64| x.cos());
// MOO: `float tan(num x)` - Returns the tangent of x (in radians).
math_fn!(bf_tan, "tan", |x: f64| x.tan());

// Functions with domain validation
// MOO: `float sqrt(num x)` - Returns the square root of x (x must be non-negative).
math_fn_with_validation!(
    bf_sqrt,
    "sqrt",
    |x: f64| x.sqrt(),
    |x: f64| x >= 0.0,
    "sqrt() takes a non-negative number"
);
// MOO: `float asin(num x)` - Returns the arcsine of x (x must be between -1 and 1).
math_fn_with_validation!(
    bf_asin,
    "asin",
    |x: f64| x.asin(),
    |x: f64| (-1.0..=1.0).contains(&x),
    "asin() takes a number between -1 and 1"
);
// MOO: `float acos(num x)` - Returns the arccosine of x (x must be between -1 and 1).
math_fn_with_validation!(
    bf_acos,
    "acos",
    |x: f64| x.acos(),
    |x: f64| (-1.0..=1.0).contains(&x),
    "acos() takes a number between -1 and 1"
);

/// MOO: `float atan(num x)` or `float atan(num y, num x)`
/// Returns arctangent of x, or atan2(y, x) if two arguments given.
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
// MOO: `float sinh(num x)` - Returns the hyperbolic sine of x.
math_fn!(bf_sinh, "sinh", |x: f64| x.sinh());
// MOO: `float cosh(num x)` - Returns the hyperbolic cosine of x.
math_fn!(bf_cosh, "cosh", |x: f64| x.cosh());
// MOO: `float tanh(num x)` - Returns the hyperbolic tangent of x.
math_fn!(bf_tanh, "tanh", |x: f64| x.tanh());

// Exponential and logarithmic functions
// MOO: `float exp(num x)` - Returns e raised to the power of x.
math_fn!(bf_exp, "exp", |x: f64| x.exp());
// MOO: `float log(num x)` - Returns the natural logarithm of x (x must be positive).
math_fn_with_validation!(
    bf_log,
    "log",
    |x: f64| x.ln(),
    |x: f64| x > 0.0,
    "log() takes a positive number"
);
// MOO: `float log10(num x)` - Returns the base-10 logarithm of x (x must be positive).
math_fn_with_validation!(
    bf_log10,
    "log10",
    |x: f64| x.log10(),
    |x: f64| x > 0.0,
    "log10() takes a positive number"
);

// Rounding functions
// MOO: `float ceil(num x)` - Returns the smallest integer greater than or equal to x.
math_fn!(bf_ceil, "ceil", |x: f64| x.ceil());
// MOO: `float floor(num x)` - Returns the largest integer less than or equal to x.
math_fn!(bf_floor, "floor", |x: f64| x.floor());
// MOO: `float trunc(num x)` - Returns x with the fractional part removed.
math_fn!(bf_trunc, "trunc", |x: f64| x.trunc());

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
