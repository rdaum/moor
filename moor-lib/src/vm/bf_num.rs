use std::sync::Arc;

use async_trait::async_trait;
use decorum::R64;
use rand::Rng;

use moor_value::var::error::Error::{E_INVARG, E_TYPE};
use moor_value::var::variant::Variant;
use moor_value::var::{v_err, v_float, v_int, v_str, Var};

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::vm::builtin::{BfCallState, BuiltinFunction};
use crate::vm::VM;

async fn bf_abs<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(v_int(i.abs())),
        Variant::Float(f) => Ok(v_float(f.abs())),
        _ => Ok(v_err(E_TYPE)),
    }
}
bf_declare!(abs, bf_abs);

async fn bf_min<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }

    match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Int(a), Variant::Int(b)) => Ok(v_int(*a.max(b))),
        (Variant::Float(a), Variant::Float(b)) => {
            let m = R64::from(*a).min(R64::from(*b));
            Ok(v_float(m.into()))
        }
        _ => Ok(v_err(E_TYPE)),
    }
}
bf_declare!(min, bf_min);

async fn bf_max<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }

    match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Int(a), Variant::Int(b)) => Ok(v_int(*a.max(b))),
        (Variant::Float(a), Variant::Float(b)) => {
            let m = R64::from(*a).max(R64::from(*b));
            Ok(v_float(m.into()))
        }
        _ => Ok(v_err(E_TYPE)),
    }
}
bf_declare!(max, bf_max);

async fn bf_random<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() > 1 {
        return Ok(v_err(E_INVARG));
    }

    let mut rng = rand::thread_rng();
    match bf_args.args[0].variant() {
        Variant::Int(i) => Ok(v_int(rng.gen_range(0..*i))),
        Variant::Float(f) => Ok(v_float(rng.gen_range(0.0..*f))),
        _ => Ok(v_err(E_TYPE)),
    }
}
bf_declare!(random, bf_random);

async fn bf_floatstr<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    let precision = match bf_args.args[1].variant() {
        Variant::Int(i) if *i > 0 => *i as usize,
        _ => return Ok(v_err(E_TYPE)),
    };

    let scientific = match bf_args.args[2].variant() {
        Variant::Int(b) => *b == 1,
        _ => return Ok(v_err(E_TYPE)),
    };

    let mut s = format!("{:.*}", precision, x);
    if scientific {
        s = format!("{:e}", x);
    }

    Ok(v_str(s.as_str()))
}
bf_declare!(floatstr, bf_floatstr);

async fn bf_sin<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    Ok(v_float(x.sin()))
}
bf_declare!(sin, bf_sin);

async fn bf_cos<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    Ok(v_float(x.cos()))
}
bf_declare!(cos, bf_cos);

async fn bf_tan<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    Ok(v_float(x.tan()))
}
bf_declare!(tan, bf_tan);

async fn bf_sqrt<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    if *x < 0.0 {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_float(x.sqrt()))
}
bf_declare!(sqrt, bf_sqrt);

async fn bf_asin<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    if !(-1.0..=1.0).contains(x) {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_float(x.asin()))
}
bf_declare!(asin, bf_asin);

async fn bf_acos<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    if !(-1.0..=1.0).contains(x) {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_float(x.acos()))
}
bf_declare!(acos, bf_acos);

async fn bf_atan<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.is_empty() || bf_args.args.len() > 2 {
        return Ok(v_err(E_INVARG));
    }

    let y = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    let x = match bf_args.args[1].variant() {
        Variant::Float(f) => *f,
        _ => return Ok(v_err(E_TYPE)),
    };

    Ok(v_float(y.atan2(x)))
}
bf_declare!(atan, bf_atan);

async fn bf_sinh<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    Ok(v_float(x.sinh()))
}
bf_declare!(sinh, bf_sinh);

async fn bf_cosh<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    Ok(v_float(x.cosh()))
}
bf_declare!(cosh, bf_cosh);

async fn bf_tanh<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    Ok(v_float(x.tanh()))
}
bf_declare!(tanh, bf_tanh);

async fn bf_exp<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    Ok(v_float(x.exp()))
}
bf_declare!(exp, bf_exp);

async fn bf_log<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    if *x <= 0.0 {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_float(x.ln()))
}
bf_declare!(log, bf_log);

async fn bf_log10<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    if *x <= 0.0 {
        return Ok(v_err(E_INVARG));
    }

    Ok(v_float(x.log10()))
}
bf_declare!(log10, bf_log10);

async fn bf_ceil<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    Ok(v_float(x.ceil()))
}
bf_declare!(ceil, bf_ceil);

async fn bf_floor<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    Ok(v_float(x.floor()))
}
bf_declare!(floor, bf_floor);

async fn bf_trunc<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }

    let x = match bf_args.args[0].variant() {
        Variant::Float(f) => f,
        _ => return Ok(v_err(E_TYPE)),
    };

    Ok(v_float(x.trunc()))
}
bf_declare!(trunc, bf_trunc);

impl VM {
    pub(crate) fn register_bf_num(&mut self) -> Result<(), anyhow::Error> {
        self.builtins[offset_for_builtin("abs")] = Arc::new(Box::new(BfAbs {}));
        self.builtins[offset_for_builtin("min")] = Arc::new(Box::new(BfMin {}));
        self.builtins[offset_for_builtin("max")] = Arc::new(Box::new(BfMax {}));
        self.builtins[offset_for_builtin("random")] = Arc::new(Box::new(BfRandom {}));
        self.builtins[offset_for_builtin("floatstr")] = Arc::new(Box::new(BfFloatstr {}));
        self.builtins[offset_for_builtin("sqrt")] = Arc::new(Box::new(BfSqrt {}));
        self.builtins[offset_for_builtin("sin")] = Arc::new(Box::new(BfSin {}));
        self.builtins[offset_for_builtin("cos")] = Arc::new(Box::new(BfCos {}));
        self.builtins[offset_for_builtin("tan")] = Arc::new(Box::new(BfTan {}));
        self.builtins[offset_for_builtin("asin")] = Arc::new(Box::new(BfAsin {}));
        self.builtins[offset_for_builtin("acos")] = Arc::new(Box::new(BfAcos {}));
        self.builtins[offset_for_builtin("atan")] = Arc::new(Box::new(BfAtan {}));
        self.builtins[offset_for_builtin("sinh")] = Arc::new(Box::new(BfSinh {}));
        self.builtins[offset_for_builtin("cosh")] = Arc::new(Box::new(BfCosh {}));
        self.builtins[offset_for_builtin("tanh")] = Arc::new(Box::new(BfTanh {}));
        self.builtins[offset_for_builtin("exp")] = Arc::new(Box::new(BfExp {}));
        self.builtins[offset_for_builtin("log")] = Arc::new(Box::new(BfLog {}));
        self.builtins[offset_for_builtin("log10")] = Arc::new(Box::new(BfLog10 {}));
        self.builtins[offset_for_builtin("ceil")] = Arc::new(Box::new(BfCeil {}));
        self.builtins[offset_for_builtin("floor")] = Arc::new(Box::new(BfFloor {}));
        self.builtins[offset_for_builtin("trunc")] = Arc::new(Box::new(BfTrunc {}));

        Ok(())
    }
}
