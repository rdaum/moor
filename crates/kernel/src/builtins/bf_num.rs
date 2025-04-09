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
use moor_var::Error::{E_ARGS, E_INVARG, E_TYPE};
use moor_var::{v_float, v_int, v_str};
use moor_var::{Sequence, Var, Variant};

use crate::bf_declare;
use crate::builtins::BfRet::Ret;
use crate::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction};

fn bf_abs(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(Ret(v_int(i.abs()))),
        Variant::Float(f) => Ok(Ret(v_float(f.abs()))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}
bf_declare!(abs, bf_abs);

fn bf_min(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    match (&bf_args.args[0].variant(), &bf_args.args[1].variant()) {
        (Variant::Int(a), Variant::Int(b)) => Ok(Ret(v_int(*a.min(b)))),
        (Variant::Float(a), Variant::Float(b)) => {
            let m = a.min(*b);
            Ok(Ret(v_float(m)))
        }
        _ => Err(BfErr::Code(E_TYPE)),
    }
}
bf_declare!(min, bf_min);

fn bf_max(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    match (&bf_args.args[0].variant(), &bf_args.args[1].variant()) {
        (Variant::Int(a), Variant::Int(b)) => Ok(Ret(v_int(*a.max(b)))),
        (Variant::Float(a), Variant::Float(b)) => {
            let m = a.max(*b);
            Ok(Ret(v_float(m)))
        }
        _ => Err(BfErr::Code(E_TYPE)),
    }
}
bf_declare!(max, bf_max);

fn bf_random(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() > 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let mut rng = rand::thread_rng();
    bf_args
        .args
        .is_empty()
        .then_some(Ok(Ret(v_int(rng.gen_range(1..=2147483647)))))
        .unwrap_or_else(|| match &bf_args.args[0].variant() {
            Variant::Int(i) if *i > 0 => Ok(Ret(v_int(rng.gen_range(1..=*i)))),
            Variant::Int(_) => Err(BfErr::Code(E_INVARG)),
            _ => Err(BfErr::Code(E_TYPE)),
        })
}
bf_declare!(random, bf_random);

fn bf_floatstr(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Err(BfErr::Code(E_TYPE)),
    };

    let precision = match &bf_args.args[1].variant() {
        Variant::Int(i) if *i > 0 => *i as usize,
        _ => return Err(BfErr::Code(E_TYPE)),
    };

    let scientific = bf_args.args.len() == 3 && bf_args.args[2].is_true();

    let mut s = format!("{:.*}", precision, x);
    if scientific {
        s = format!("{:e}", x);
    }

    Ok(Ret(v_str(s.as_str())))
}
bf_declare!(floatstr, bf_floatstr);

fn numeric_arg(arg: &Var) -> Result<f64, BfErr> {
    let x = match arg.variant() {
        Variant::Int(i) => *i as f64,
        Variant::Float(f) => *f,
        _ => return Err(BfErr::Code(E_TYPE)),
    };

    Ok(x)
}

fn bf_sin(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.sin())))
}
bf_declare!(sin, bf_sin);

fn bf_cos(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.cos())))
}
bf_declare!(cos, bf_cos);

fn bf_tan(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.tan())))
}
bf_declare!(tan, bf_tan);

fn bf_sqrt(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    if x < 0.0 {
        return Err(BfErr::Code(E_ARGS));
    }

    Ok(Ret(v_float(x.sqrt())))
}
bf_declare!(sqrt, bf_sqrt);

fn bf_asin(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    if !(-1.0..=1.0).contains(&x) {
        return Err(BfErr::Code(E_ARGS));
    }

    Ok(Ret(v_float(x.asin())))
}
bf_declare!(asin, bf_asin);

fn bf_acos(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    if !(-1.0..=1.0).contains(&x) {
        return Err(BfErr::Code(E_ARGS));
    }

    Ok(Ret(v_float(x.acos())))
}
bf_declare!(acos, bf_acos);

fn bf_atan(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;
    let y = numeric_arg(&bf_args.args[1])?;

    Ok(Ret(v_float(y.atan2(x))))
}
bf_declare!(atan, bf_atan);

fn bf_sinh(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.sinh())))
}
bf_declare!(sinh, bf_sinh);

fn bf_cosh(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.cosh())))
}
bf_declare!(cosh, bf_cosh);

fn bf_tanh(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.tanh())))
}
bf_declare!(tanh, bf_tanh);

fn bf_exp(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.exp())))
}
bf_declare!(exp, bf_exp);

fn bf_log(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    if x <= 0.0 {
        return Err(BfErr::Code(E_ARGS));
    }

    Ok(Ret(v_float(x.ln())))
}
bf_declare!(log, bf_log);

fn bf_log10(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    if x <= 0.0 {
        return Err(BfErr::Code(E_ARGS));
    }

    Ok(Ret(v_float(x.log10())))
}
bf_declare!(log10, bf_log10);

fn bf_ceil(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.ceil())))
}
bf_declare!(ceil, bf_ceil);

fn bf_floor(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.floor())))
}
bf_declare!(floor, bf_floor);

fn bf_trunc(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let x = numeric_arg(&bf_args.args[0])?;

    Ok(Ret(v_float(x.trunc())))
}
bf_declare!(trunc, bf_trunc);

pub(crate) fn register_bf_num(builtins: &mut [Box<dyn BuiltinFunction>]) {
    builtins[offset_for_builtin("abs")] = Box::new(BfAbs {});
    builtins[offset_for_builtin("min")] = Box::new(BfMin {});
    builtins[offset_for_builtin("max")] = Box::new(BfMax {});
    builtins[offset_for_builtin("random")] = Box::new(BfRandom {});
    builtins[offset_for_builtin("floatstr")] = Box::new(BfFloatstr {});
    builtins[offset_for_builtin("sqrt")] = Box::new(BfSqrt {});
    builtins[offset_for_builtin("sin")] = Box::new(BfSin {});
    builtins[offset_for_builtin("cos")] = Box::new(BfCos {});
    builtins[offset_for_builtin("tan")] = Box::new(BfTan {});
    builtins[offset_for_builtin("asin")] = Box::new(BfAsin {});
    builtins[offset_for_builtin("acos")] = Box::new(BfAcos {});
    builtins[offset_for_builtin("atan")] = Box::new(BfAtan {});
    builtins[offset_for_builtin("sinh")] = Box::new(BfSinh {});
    builtins[offset_for_builtin("cosh")] = Box::new(BfCosh {});
    builtins[offset_for_builtin("tanh")] = Box::new(BfTanh {});
    builtins[offset_for_builtin("exp")] = Box::new(BfExp {});
    builtins[offset_for_builtin("log")] = Box::new(BfLog {});
    builtins[offset_for_builtin("log10")] = Box::new(BfLog10 {});
    builtins[offset_for_builtin("ceil")] = Box::new(BfCeil {});
    builtins[offset_for_builtin("floor")] = Box::new(BfFloor {});
    builtins[offset_for_builtin("trunc")] = Box::new(BfTrunc {});
}
