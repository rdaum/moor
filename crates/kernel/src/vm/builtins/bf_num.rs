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

use rand::Rng;

use moor_compiler::offset_for_builtin;
use moor_var::{E_ARGS, E_INVARG, E_TYPE, Sequence, Var, Variant};
use moor_var::{v_float, v_int, v_str};

use crate::vm::builtins::BfRet::Ret;
use crate::vm::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction};

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

fn bf_random(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("random() takes 0 or 1 argument"),
        ));
    }

    let mut rng = rand::thread_rng();
    if bf_args.args.is_empty() {
        Ok(Ret(v_int(rng.gen_range(1..=2147483647))))
    } else {
        match &bf_args.args[0].variant() {
            Variant::Int(i) if *i > 0 => Ok(Ret(v_int(rng.gen_range(1..=*i)))),
            Variant::Int(_) => Err(BfErr::Code(E_INVARG)),
            _ => Err(BfErr::Code(E_TYPE)),
        }
    }
}

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

    let mut s = format!("{:.*}", precision, x);
    if scientific {
        s = format!("{:e}", x);
    }

    Ok(Ret(v_str(s.as_str())))
}

fn numeric_arg(arg: &Var) -> Result<f64, BfErr> {
    let x = match arg.variant() {
        Variant::Int(i) => *i as f64,
        Variant::Float(f) => *f,
        _ => return Err(BfErr::ErrValue(E_TYPE.msg("non-numeric argument"))),
    };

    Ok(x)
}

fn bf_sin(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("sin() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.sin())))
}

fn bf_cos(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("cos() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.cos())))
}

fn bf_tan(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("tan() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.tan())))
}

fn bf_sqrt(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("sqrt() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    if x < 0.0 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("sqrt() takes a non-negative number"),
        ));
    }

    Ok(Ret(v_float(x.sqrt())))
}

fn bf_asin(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("asin() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    if !(-1.0..=1.0).contains(&x) {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("asin() takes a number between -1 and 1"),
        ));
    }

    Ok(Ret(v_float(x.asin())))
}

fn bf_acos(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("acos() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    if !(-1.0..=1.0).contains(&x) {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("acos() takes a number between -1 and 1"),
        ));
    }

    Ok(Ret(v_float(x.acos())))
}

fn bf_atan(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(BfErr::ErrValue(E_ARGS.msg("atan() takes 1 or 2 arguments")));
    }

    let x = numeric_arg(&bf_args.args[0])?;
    let y = numeric_arg(&bf_args.args[1])?;

    Ok(Ret(v_float(y.atan2(x))))
}

fn bf_sinh(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("sinh() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.sinh())))
}

fn bf_cosh(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("cosh() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.cosh())))
}

fn bf_tanh(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("tanh() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.tanh())))
}

fn bf_exp(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("exp() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.exp())))
}

fn bf_log(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("log() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    if x <= 0.0 {
        return Err(BfErr::ErrValue(E_ARGS.msg("log() takes a positive number")));
    }

    Ok(Ret(v_float(x.ln())))
}

fn bf_log10(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("log10() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    if x <= 0.0 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("log10() takes a positive number"),
        ));
    }

    Ok(Ret(v_float(x.log10())))
}

fn bf_ceil(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("ceil() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.ceil())))
}

fn bf_floor(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("floor() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.floor())))
}

fn bf_trunc(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::ErrValue(E_ARGS.msg("trunc() takes 1 argument")));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.trunc())))
}

pub(crate) fn register_bf_num(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("abs")] = Box::new(bf_abs);
    builtins[offset_for_builtin("min")] = Box::new(bf_min);
    builtins[offset_for_builtin("max")] = Box::new(bf_max);
    builtins[offset_for_builtin("random")] = Box::new(bf_random);
    builtins[offset_for_builtin("floatstr")] = Box::new(bf_floatstr);
    builtins[offset_for_builtin("sqrt")] = Box::new(bf_sqrt);
    builtins[offset_for_builtin("sin")] = Box::new(bf_sin);
    builtins[offset_for_builtin("cos")] = Box::new(bf_cos);
    builtins[offset_for_builtin("tan")] = Box::new(bf_tan);
    builtins[offset_for_builtin("asin")] = Box::new(bf_asin);
    builtins[offset_for_builtin("acos")] = Box::new(bf_acos);
    builtins[offset_for_builtin("atan")] = Box::new(bf_atan);
    builtins[offset_for_builtin("sinh")] = Box::new(bf_sinh);
    builtins[offset_for_builtin("cosh")] = Box::new(bf_cosh);
    builtins[offset_for_builtin("tanh")] = Box::new(bf_tanh);
    builtins[offset_for_builtin("exp")] = Box::new(bf_exp);
    builtins[offset_for_builtin("log")] = Box::new(bf_log);
    builtins[offset_for_builtin("log10")] = Box::new(bf_log10);
    builtins[offset_for_builtin("ceil")] = Box::new(bf_ceil);
    builtins[offset_for_builtin("floor")] = Box::new(bf_floor);
    builtins[offset_for_builtin("trunc")] = Box::new(bf_trunc);
}
