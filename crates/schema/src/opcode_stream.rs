// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Stable binary encoding for opcodes using a 16-bit word stream.
//!
//! This encoding is designed for:
//! - Version stability: explicit opcode numbers that never change
//! - Efficient execution: PC is a direct index into the word stream
//! - Natural alignment: most arguments are u16 (Label, Offset, Name fields)
//! - Simple jumps: labels are word offsets

use moor_var::{
    ErrorCode, Obj, Symbol, VarType,
    program::{
        labels::{Label, Offset},
        names::Name,
        opcode::{BuiltinId, ComprehensionType, Op},
    },
};

// ============================================================================
// Stable Opcode Numbers - NEVER change these!
// ============================================================================

const OP_ADD: u16 = 0;
const OP_AND: u16 = 1;
const OP_BIT_AND: u16 = 2;
const OP_BIT_OR: u16 = 3;
const OP_BIT_XOR: u16 = 4;
const OP_BIT_SHL: u16 = 5;
const OP_BIT_SHR: u16 = 6;
const OP_BIT_NOT: u16 = 7;
const OP_CALL_VERB: u16 = 8;
const OP_CHECK_LIST_FOR_SPLICE: u16 = 9;
const OP_FINALLY_CONTINUE: u16 = 10;
const OP_DIV: u16 = 11;
const OP_DONE: u16 = 12;
const OP_END_CATCH: u16 = 13;
const OP_END_EXCEPT: u16 = 14;
const OP_END_FINALLY: u16 = 15;
const OP_EQ: u16 = 16;
const OP_EXIT: u16 = 17;
const OP_EXIT_ID: u16 = 18;
const OP_EXP: u16 = 19;
const OP_BEGIN_FOR_SEQUENCE: u16 = 20;
const OP_ITERATE_FOR_SEQUENCE: u16 = 21;
const OP_BEGIN_FOR_RANGE: u16 = 22;
const OP_ITERATE_FOR_RANGE: u16 = 23;
const OP_FORK: u16 = 24;
const OP_FUNC_CALL: u16 = 25;
const OP_GE: u16 = 26;
const OP_GET_PROP: u16 = 27;
const OP_GT: u16 = 28;
const OP_IF_QUES: u16 = 29;
const OP_IMM: u16 = 30;
const OP_IMM_BIG_INT: u16 = 31;
const OP_IMM_FLOAT: u16 = 32;
const OP_IMM_EMPTY_LIST: u16 = 33;
const OP_IMM_INT: u16 = 34;
const OP_IMM_TYPE: u16 = 35;
const OP_IMM_NONE: u16 = 36;
const OP_IMM_OBJID: u16 = 37;
const OP_IMM_SYMBOL: u16 = 38;
const OP_IN: u16 = 39;
const OP_INDEX_SET: u16 = 40;
const OP_JUMP: u16 = 41;
const OP_LE: u16 = 42;
const OP_LENGTH: u16 = 43;
const OP_LIST_ADD_TAIL: u16 = 44;
const OP_LIST_APPEND: u16 = 45;
const OP_LT: u16 = 46;
const OP_IMM_ERR: u16 = 47;
const OP_MAKE_ERROR: u16 = 48;
const OP_MAKE_SINGLETON_LIST: u16 = 49;
const OP_MAKE_MAP: u16 = 50;
const OP_MAP_INSERT: u16 = 51;
const OP_MAKE_FLYWEIGHT: u16 = 52;
const OP_MOD: u16 = 53;
const OP_MUL: u16 = 54;
const OP_NE: u16 = 55;
const OP_NOT: u16 = 56;
const OP_OR: u16 = 57;
const OP_PASS: u16 = 58;
const OP_POP: u16 = 59;
const OP_PUSH: u16 = 60;
const OP_PUSH_GET_PROP: u16 = 61;
const OP_PUSH_REF: u16 = 62;
const OP_PUSH_TEMP: u16 = 63;
const OP_PUT: u16 = 64;
const OP_PUT_PROP: u16 = 65;
const OP_PUT_TEMP: u16 = 66;
const OP_RANGE_REF: u16 = 67;
const OP_RANGE_SET: u16 = 68;
const OP_REF: u16 = 69;
const OP_RETURN: u16 = 70;
const OP_RETURN0: u16 = 71;
const OP_SCATTER: u16 = 72;
const OP_SUB: u16 = 73;
const OP_PUSH_CATCH_LABEL: u16 = 74;
const OP_TRY_CATCH: u16 = 75;
const OP_TRY_EXCEPT: u16 = 76;
const OP_TRY_FINALLY: u16 = 77;
const OP_BEGIN_SCOPE: u16 = 78;
const OP_END_SCOPE: u16 = 79;
const OP_UNARY_MINUS: u16 = 80;
const OP_WHILE: u16 = 81;
const OP_WHILE_ID: u16 = 82;
const OP_IF: u16 = 83;
const OP_EIF: u16 = 84;
const OP_BEGIN_COMPREHENSION: u16 = 85;
const OP_COMPREHEND_RANGE: u16 = 86;
const OP_COMPREHEND_LIST: u16 = 87;
const OP_CONTINUE_COMPREHENSION: u16 = 88;
const OP_CAPTURE: u16 = 89;
const OP_MAKE_LAMBDA: u16 = 90;
const OP_CALL_LAMBDA: u16 = 91;
const OP_BIT_LSHR: u16 = 92;

// Reserve 93-999 for future opcodes
// Reserve 1000-65535 for extensions

// ============================================================================
// Helper functions for ErrorCode encoding
// ============================================================================

pub fn error_code_to_discriminant(err: &ErrorCode) -> u8 {
    use ErrorCode::*;
    match err {
        E_NONE => 0,
        E_TYPE => 1,
        E_DIV => 2,
        E_PERM => 3,
        E_PROPNF => 4,
        E_VERBNF => 5,
        E_VARNF => 6,
        E_INVIND => 7,
        E_RECMOVE => 8,
        E_MAXREC => 9,
        E_RANGE => 10,
        E_ARGS => 11,
        E_NACC => 12,
        E_INVARG => 13,
        E_QUOTA => 14,
        E_FLOAT => 15,
        E_FILE => 16,
        E_EXEC => 17,
        E_INTRPT => 18,
        ErrCustom(_) => 255,
    }
}

pub fn error_code_from_discriminant(disc: u8) -> Option<ErrorCode> {
    use ErrorCode::*;
    match disc {
        0 => Some(E_NONE),
        1 => Some(E_TYPE),
        2 => Some(E_DIV),
        3 => Some(E_PERM),
        4 => Some(E_PROPNF),
        5 => Some(E_VERBNF),
        6 => Some(E_VARNF),
        7 => Some(E_INVIND),
        8 => Some(E_RECMOVE),
        9 => Some(E_MAXREC),
        10 => Some(E_RANGE),
        11 => Some(E_ARGS),
        12 => Some(E_NACC),
        13 => Some(E_INVARG),
        14 => Some(E_QUOTA),
        15 => Some(E_FLOAT),
        16 => Some(E_FILE),
        17 => Some(E_EXEC),
        18 => Some(E_INTRPT),
        _ => None,
    }
}

// ============================================================================
// Encoding Errors
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("Unexpected end of stream at PC {0}")]
    UnexpectedEnd(usize),
    #[error("Unknown opcode: {0} at PC {1}")]
    UnknownOpcode(u16, usize),
    #[error("Invalid enum value: {0} for {1} at PC {2}")]
    InvalidEnum(u16, &'static str, usize),
}

// ============================================================================
// OpStream - 16-bit word stream for opcodes
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct OpStream {
    words: Vec<u16>,
}

impl OpStream {
    pub fn new() -> Self {
        Self { words: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            words: Vec::with_capacity(capacity),
        }
    }

    /// Get the underlying word vector
    pub fn into_words(self) -> Vec<u16> {
        self.words
    }

    /// Create from a word vector
    pub fn from_words(words: Vec<u16>) -> Self {
        Self { words }
    }

    /// Get the current length in words
    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Encode a single opcode and append to the stream
    pub fn encode(&mut self, op: &Op) {
        match op {
            Op::Add => self.words.push(OP_ADD),
            Op::Sub => self.words.push(OP_SUB),
            Op::Mul => self.words.push(OP_MUL),
            Op::Div => self.words.push(OP_DIV),
            Op::Mod => self.words.push(OP_MOD),
            Op::Exp => self.words.push(OP_EXP),

            Op::And(label) => {
                self.words.push(OP_AND);
                self.words.push(label.0);
            }

            Op::Or(label) => {
                self.words.push(OP_OR);
                self.words.push(label.0);
            }

            Op::Not => self.words.push(OP_NOT),
            Op::UnaryMinus => self.words.push(OP_UNARY_MINUS),

            Op::BitAnd => self.words.push(OP_BIT_AND),
            Op::BitOr => self.words.push(OP_BIT_OR),
            Op::BitXor => self.words.push(OP_BIT_XOR),
            Op::BitShl => self.words.push(OP_BIT_SHL),
            Op::BitShr => self.words.push(OP_BIT_SHR),
            Op::BitLShr => self.words.push(OP_BIT_LSHR),
            Op::BitNot => self.words.push(OP_BIT_NOT),

            Op::Eq => self.words.push(OP_EQ),
            Op::Ne => self.words.push(OP_NE),
            Op::Lt => self.words.push(OP_LT),
            Op::Le => self.words.push(OP_LE),
            Op::Gt => self.words.push(OP_GT),
            Op::Ge => self.words.push(OP_GE),
            Op::In => self.words.push(OP_IN),

            Op::Push(name) => {
                self.words.push(OP_PUSH);
                self.encode_name(name);
            }

            Op::Put(name) => {
                self.words.push(OP_PUT);
                self.encode_name(name);
            }

            Op::Pop => self.words.push(OP_POP),
            Op::Ref => self.words.push(OP_REF),
            Op::PushRef => self.words.push(OP_PUSH_REF),
            Op::PushTemp => self.words.push(OP_PUSH_TEMP),
            Op::PutTemp => self.words.push(OP_PUT_TEMP),

            Op::ImmInt(v) => {
                self.words.push(OP_IMM_INT);
                self.encode_i32(*v);
            }

            Op::ImmBigInt(v) => {
                self.words.push(OP_IMM_BIG_INT);
                self.encode_i64(*v);
            }

            Op::ImmFloat(v) => {
                self.words.push(OP_IMM_FLOAT);
                self.encode_f64(*v);
            }

            Op::ImmObjid(obj) => {
                self.words.push(OP_IMM_OBJID);
                self.encode_u64(obj.as_u64());
            }

            Op::ImmSymbol(sym) => {
                self.words.push(OP_IMM_SYMBOL);
                // Symbol is two u32s - need to serialize as string or use internal IDs
                // For now, we'll need to store the string and re-intern on load
                // TODO: This needs proper symbol table handling
                let sym_str = sym.as_string();
                let bytes = sym_str.as_bytes();
                // Encode length + string bytes
                self.words.push(bytes.len() as u16);
                for chunk in bytes.chunks(2) {
                    let w = if chunk.len() == 2 {
                        u16::from_le_bytes([chunk[0], chunk[1]])
                    } else {
                        u16::from_le_bytes([chunk[0], 0])
                    };
                    self.words.push(w);
                }
            }

            Op::Imm(label) => {
                self.words.push(OP_IMM);
                self.words.push(label.0);
            }

            Op::ImmEmptyList => self.words.push(OP_IMM_EMPTY_LIST),
            Op::ImmNone => self.words.push(OP_IMM_NONE),

            Op::ImmType(vt) => {
                self.words.push(OP_IMM_TYPE);
                self.words.push(*vt as u16);
            }

            Op::ImmErr(err) => {
                self.words.push(OP_IMM_ERR);
                let disc = error_code_to_discriminant(err);
                self.words.push(disc as u16);

                // If custom, also encode the symbol
                if let ErrorCode::ErrCustom(sym) = err {
                    let sym_str = sym.as_string();
                    let bytes = sym_str.as_bytes();
                    self.words.push(bytes.len() as u16);
                    for chunk in bytes.chunks(2) {
                        let w = if chunk.len() == 2 {
                            u16::from_le_bytes([chunk[0], chunk[1]])
                        } else {
                            u16::from_le_bytes([chunk[0], 0])
                        };
                        self.words.push(w);
                    }
                }
            }

            Op::MakeError(offset) => {
                self.words.push(OP_MAKE_ERROR);
                self.words.push(offset.0);
            }

            Op::MakeSingletonList => self.words.push(OP_MAKE_SINGLETON_LIST),
            Op::ListAddTail => self.words.push(OP_LIST_ADD_TAIL),
            Op::ListAppend => self.words.push(OP_LIST_APPEND),

            Op::IndexSet => self.words.push(OP_INDEX_SET),
            Op::RangeRef => self.words.push(OP_RANGE_REF),
            Op::RangeSet => self.words.push(OP_RANGE_SET),

            Op::Length(offset) => {
                self.words.push(OP_LENGTH);
                self.words.push(offset.0);
            }

            Op::MakeMap => self.words.push(OP_MAKE_MAP),
            Op::MapInsert => self.words.push(OP_MAP_INSERT),

            Op::MakeFlyweight(size) => {
                self.words.push(OP_MAKE_FLYWEIGHT);
                self.encode_u64(*size as u64);
            }

            Op::GetProp => self.words.push(OP_GET_PROP),
            Op::PushGetProp => self.words.push(OP_PUSH_GET_PROP),
            Op::PutProp => self.words.push(OP_PUT_PROP),

            Op::Jump { label } => {
                self.words.push(OP_JUMP);
                self.words.push(label.0);
            }

            Op::IfQues(label) => {
                self.words.push(OP_IF_QUES);
                self.words.push(label.0);
            }

            Op::If(label, width) => {
                self.words.push(OP_IF);
                self.words.push(label.0);
                self.words.push(*width);
            }

            Op::Eif(label, width) => {
                self.words.push(OP_EIF);
                self.words.push(label.0);
                self.words.push(*width);
            }

            Op::While {
                jump_label,
                environment_width,
            } => {
                self.words.push(OP_WHILE);
                self.words.push(jump_label.0);
                self.words.push(*environment_width);
            }

            Op::WhileId {
                id,
                end_label,
                environment_width,
            } => {
                self.words.push(OP_WHILE_ID);
                self.encode_name(id);
                self.words.push(end_label.0);
                self.words.push(*environment_width);
            }

            Op::BeginForSequence { operand } => {
                self.words.push(OP_BEGIN_FOR_SEQUENCE);
                self.words.push(operand.0);
            }

            Op::IterateForSequence => self.words.push(OP_ITERATE_FOR_SEQUENCE),

            Op::BeginForRange { operand } => {
                self.words.push(OP_BEGIN_FOR_RANGE);
                self.words.push(operand.0);
            }

            Op::IterateForRange => self.words.push(OP_ITERATE_FOR_RANGE),

            Op::Exit { stack, label } => {
                self.words.push(OP_EXIT);
                self.words.push(stack.0);
                self.words.push(label.0);
            }

            Op::ExitId(label) => {
                self.words.push(OP_EXIT_ID);
                self.words.push(label.0);
            }

            Op::CallVerb => self.words.push(OP_CALL_VERB),
            Op::Pass => self.words.push(OP_PASS),

            Op::FuncCall { id } => {
                self.words.push(OP_FUNC_CALL);
                self.words.push(id.0);
            }

            Op::Return => self.words.push(OP_RETURN),
            Op::Return0 => self.words.push(OP_RETURN0),
            Op::Done => self.words.push(OP_DONE),

            Op::CheckListForSplice => self.words.push(OP_CHECK_LIST_FOR_SPLICE),

            Op::Scatter(offset) => {
                self.words.push(OP_SCATTER);
                self.words.push(offset.0);
            }

            Op::BeginScope {
                num_bindings,
                end_label,
            } => {
                self.words.push(OP_BEGIN_SCOPE);
                self.words.push(*num_bindings);
                self.words.push(end_label.0);
            }

            Op::EndScope { num_bindings } => {
                self.words.push(OP_END_SCOPE);
                self.words.push(*num_bindings);
            }

            Op::PushCatchLabel(label) => {
                self.words.push(OP_PUSH_CATCH_LABEL);
                self.words.push(label.0);
            }

            Op::TryCatch {
                handler_label,
                end_label,
            } => {
                self.words.push(OP_TRY_CATCH);
                self.words.push(handler_label.0);
                self.words.push(end_label.0);
            }

            Op::TryExcept {
                num_excepts,
                environment_width,
                end_label,
            } => {
                self.words.push(OP_TRY_EXCEPT);
                self.words.push(*num_excepts);
                self.words.push(*environment_width);
                self.words.push(end_label.0);
            }

            Op::TryFinally {
                end_label,
                environment_width,
            } => {
                self.words.push(OP_TRY_FINALLY);
                self.words.push(end_label.0);
                self.words.push(*environment_width);
            }

            Op::EndCatch(label) => {
                self.words.push(OP_END_CATCH);
                self.words.push(label.0);
            }

            Op::EndExcept(label) => {
                self.words.push(OP_END_EXCEPT);
                self.words.push(label.0);
            }

            Op::EndFinally => self.words.push(OP_END_FINALLY),
            Op::FinallyContinue => self.words.push(OP_FINALLY_CONTINUE),

            Op::BeginComprehension(comp_type, start_label, end_label) => {
                self.words.push(OP_BEGIN_COMPREHENSION);
                self.words.push(*comp_type as u16);
                self.words.push(start_label.0);
                self.words.push(end_label.0);
            }

            Op::ComprehendRange(offset) => {
                self.words.push(OP_COMPREHEND_RANGE);
                self.words.push(offset.0);
            }

            Op::ComprehendList(offset) => {
                self.words.push(OP_COMPREHEND_LIST);
                self.words.push(offset.0);
            }

            Op::ContinueComprehension(name) => {
                self.words.push(OP_CONTINUE_COMPREHENSION);
                self.encode_name(name);
            }

            Op::Capture(name) => {
                self.words.push(OP_CAPTURE);
                self.encode_name(name);
            }

            Op::MakeLambda {
                scatter_offset,
                program_offset,
                self_var,
                num_captured,
            } => {
                self.words.push(OP_MAKE_LAMBDA);
                self.words.push(scatter_offset.0);
                self.words.push(program_offset.0);
                self.encode_option_name(self_var);
                self.words.push(*num_captured);
            }

            Op::CallLambda => self.words.push(OP_CALL_LAMBDA),

            Op::Fork { fv_offset, id } => {
                self.words.push(OP_FORK);
                self.words.push(fv_offset.0);
                self.encode_option_name(id);
            }
        }
    }

    /// Decode a single opcode at the given PC, advancing PC appropriately
    pub fn decode_at(&self, pc: &mut usize) -> Result<Op, DecodeError> {
        let start_pc = *pc;
        let opcode = self.read_u16(pc)?;

        match opcode {
            OP_ADD => Ok(Op::Add),
            OP_SUB => Ok(Op::Sub),
            OP_MUL => Ok(Op::Mul),
            OP_DIV => Ok(Op::Div),
            OP_MOD => Ok(Op::Mod),
            OP_EXP => Ok(Op::Exp),

            OP_AND => Ok(Op::And(Label(self.read_u16(pc)?))),
            OP_OR => Ok(Op::Or(Label(self.read_u16(pc)?))),
            OP_NOT => Ok(Op::Not),
            OP_UNARY_MINUS => Ok(Op::UnaryMinus),

            OP_BIT_AND => Ok(Op::BitAnd),
            OP_BIT_OR => Ok(Op::BitOr),
            OP_BIT_XOR => Ok(Op::BitXor),
            OP_BIT_SHL => Ok(Op::BitShl),
            OP_BIT_SHR => Ok(Op::BitShr),
            OP_BIT_LSHR => Ok(Op::BitLShr),
            OP_BIT_NOT => Ok(Op::BitNot),

            OP_EQ => Ok(Op::Eq),
            OP_NE => Ok(Op::Ne),
            OP_LT => Ok(Op::Lt),
            OP_LE => Ok(Op::Le),
            OP_GT => Ok(Op::Gt),
            OP_GE => Ok(Op::Ge),
            OP_IN => Ok(Op::In),

            OP_PUSH => Ok(Op::Push(self.decode_name(pc)?)),
            OP_PUT => Ok(Op::Put(self.decode_name(pc)?)),
            OP_POP => Ok(Op::Pop),
            OP_REF => Ok(Op::Ref),
            OP_PUSH_REF => Ok(Op::PushRef),
            OP_PUSH_TEMP => Ok(Op::PushTemp),
            OP_PUT_TEMP => Ok(Op::PutTemp),

            OP_IMM_INT => Ok(Op::ImmInt(self.decode_i32(pc)?)),
            OP_IMM_BIG_INT => Ok(Op::ImmBigInt(self.decode_i64(pc)?)),
            OP_IMM_FLOAT => Ok(Op::ImmFloat(self.decode_f64(pc)?)),
            OP_IMM_OBJID => {
                // Reconstruct Obj from raw u64
                let bits = self.decode_u64(pc)?;
                // SAFETY: Obj is repr(transparent) over u64
                Ok(Op::ImmObjid(unsafe {
                    std::mem::transmute::<u64, Obj>(bits)
                }))
            }
            OP_IMM_SYMBOL => {
                // Decode string and re-intern
                let len = self.read_u16(pc)? as usize;
                let num_words = len.div_ceil(2);
                let mut bytes = Vec::with_capacity(len);
                for _ in 0..num_words {
                    let w = self.read_u16(pc)?;
                    bytes.push((w & 0xff) as u8);
                    if bytes.len() < len {
                        bytes.push((w >> 8) as u8);
                    }
                }
                bytes.truncate(len);
                let sym_str = String::from_utf8(bytes).map_err(|_| {
                    DecodeError::InvalidEnum(0, "Symbol string", *pc - num_words - 1)
                })?;
                Ok(Op::ImmSymbol(Symbol::mk(&sym_str)))
            }
            OP_IMM => Ok(Op::Imm(Label(self.read_u16(pc)?))),
            OP_IMM_EMPTY_LIST => Ok(Op::ImmEmptyList),
            OP_IMM_NONE => Ok(Op::ImmNone),

            OP_IMM_TYPE => {
                let vt_val = self.read_u16(pc)?;
                if vt_val > 255 {
                    return Err(DecodeError::InvalidEnum(vt_val, "VarType", start_pc));
                }
                let vt = VarType::from_repr(vt_val as u8)
                    .ok_or(DecodeError::InvalidEnum(vt_val, "VarType", start_pc))?;
                Ok(Op::ImmType(vt))
            }

            OP_IMM_ERR => {
                let disc_val = self.read_u16(pc)?;
                if disc_val > 255 {
                    return Err(DecodeError::InvalidEnum(
                        disc_val,
                        "ErrorCode discriminant",
                        start_pc,
                    ));
                }
                let disc = disc_val as u8;

                let err = if disc == 255 {
                    // Custom error - decode symbol
                    let len = self.read_u16(pc)? as usize;
                    let num_words = len.div_ceil(2);
                    let mut bytes = Vec::with_capacity(len);
                    for _ in 0..num_words {
                        let w = self.read_u16(pc)?;
                        bytes.push((w & 0xff) as u8);
                        if bytes.len() < len {
                            bytes.push((w >> 8) as u8);
                        }
                    }
                    bytes.truncate(len);
                    let sym_str = String::from_utf8(bytes).map_err(|_| {
                        DecodeError::InvalidEnum(disc_val, "ErrorCode custom symbol", *pc)
                    })?;
                    ErrorCode::ErrCustom(Symbol::mk(&sym_str))
                } else {
                    error_code_from_discriminant(disc).ok_or(DecodeError::InvalidEnum(
                        disc_val,
                        "ErrorCode",
                        start_pc,
                    ))?
                };

                Ok(Op::ImmErr(err))
            }

            OP_MAKE_ERROR => Ok(Op::MakeError(Offset(self.read_u16(pc)?))),
            OP_MAKE_SINGLETON_LIST => Ok(Op::MakeSingletonList),
            OP_LIST_ADD_TAIL => Ok(Op::ListAddTail),
            OP_LIST_APPEND => Ok(Op::ListAppend),

            OP_INDEX_SET => Ok(Op::IndexSet),
            OP_RANGE_REF => Ok(Op::RangeRef),
            OP_RANGE_SET => Ok(Op::RangeSet),

            OP_LENGTH => Ok(Op::Length(Offset(self.read_u16(pc)?))),

            OP_MAKE_MAP => Ok(Op::MakeMap),
            OP_MAP_INSERT => Ok(Op::MapInsert),

            OP_MAKE_FLYWEIGHT => Ok(Op::MakeFlyweight(self.decode_u64(pc)? as usize)),

            OP_GET_PROP => Ok(Op::GetProp),
            OP_PUSH_GET_PROP => Ok(Op::PushGetProp),
            OP_PUT_PROP => Ok(Op::PutProp),

            OP_JUMP => Ok(Op::Jump {
                label: Label(self.read_u16(pc)?),
            }),
            OP_IF_QUES => Ok(Op::IfQues(Label(self.read_u16(pc)?))),

            OP_IF => Ok(Op::If(Label(self.read_u16(pc)?), self.read_u16(pc)?)),
            OP_EIF => Ok(Op::Eif(Label(self.read_u16(pc)?), self.read_u16(pc)?)),

            OP_WHILE => Ok(Op::While {
                jump_label: Label(self.read_u16(pc)?),
                environment_width: self.read_u16(pc)?,
            }),

            OP_WHILE_ID => Ok(Op::WhileId {
                id: self.decode_name(pc)?,
                end_label: Label(self.read_u16(pc)?),
                environment_width: self.read_u16(pc)?,
            }),

            OP_BEGIN_FOR_SEQUENCE => Ok(Op::BeginForSequence {
                operand: Offset(self.read_u16(pc)?),
            }),

            OP_ITERATE_FOR_SEQUENCE => Ok(Op::IterateForSequence),

            OP_BEGIN_FOR_RANGE => Ok(Op::BeginForRange {
                operand: Offset(self.read_u16(pc)?),
            }),

            OP_ITERATE_FOR_RANGE => Ok(Op::IterateForRange),

            OP_EXIT => Ok(Op::Exit {
                stack: Offset(self.read_u16(pc)?),
                label: Label(self.read_u16(pc)?),
            }),

            OP_EXIT_ID => Ok(Op::ExitId(Label(self.read_u16(pc)?))),

            OP_CALL_VERB => Ok(Op::CallVerb),
            OP_PASS => Ok(Op::Pass),

            OP_FUNC_CALL => Ok(Op::FuncCall {
                id: BuiltinId(self.read_u16(pc)?),
            }),

            OP_RETURN => Ok(Op::Return),
            OP_RETURN0 => Ok(Op::Return0),
            OP_DONE => Ok(Op::Done),

            OP_CHECK_LIST_FOR_SPLICE => Ok(Op::CheckListForSplice),

            OP_SCATTER => Ok(Op::Scatter(Offset(self.read_u16(pc)?))),

            OP_BEGIN_SCOPE => Ok(Op::BeginScope {
                num_bindings: self.read_u16(pc)?,
                end_label: Label(self.read_u16(pc)?),
            }),

            OP_END_SCOPE => Ok(Op::EndScope {
                num_bindings: self.read_u16(pc)?,
            }),

            OP_PUSH_CATCH_LABEL => Ok(Op::PushCatchLabel(Label(self.read_u16(pc)?))),

            OP_TRY_CATCH => Ok(Op::TryCatch {
                handler_label: Label(self.read_u16(pc)?),
                end_label: Label(self.read_u16(pc)?),
            }),

            OP_TRY_EXCEPT => Ok(Op::TryExcept {
                num_excepts: self.read_u16(pc)?,
                environment_width: self.read_u16(pc)?,
                end_label: Label(self.read_u16(pc)?),
            }),

            OP_TRY_FINALLY => Ok(Op::TryFinally {
                end_label: Label(self.read_u16(pc)?),
                environment_width: self.read_u16(pc)?,
            }),

            OP_END_CATCH => Ok(Op::EndCatch(Label(self.read_u16(pc)?))),
            OP_END_EXCEPT => Ok(Op::EndExcept(Label(self.read_u16(pc)?))),
            OP_END_FINALLY => Ok(Op::EndFinally),
            OP_FINALLY_CONTINUE => Ok(Op::FinallyContinue),

            OP_BEGIN_COMPREHENSION => {
                let comp_type_val = self.read_u16(pc)?;
                let comp_type = match comp_type_val {
                    0 => ComprehensionType::Range,
                    1 => ComprehensionType::List,
                    _ => {
                        return Err(DecodeError::InvalidEnum(
                            comp_type_val,
                            "ComprehensionType",
                            start_pc,
                        ));
                    }
                };
                Ok(Op::BeginComprehension(
                    comp_type,
                    Label(self.read_u16(pc)?),
                    Label(self.read_u16(pc)?),
                ))
            }

            OP_COMPREHEND_RANGE => Ok(Op::ComprehendRange(Offset(self.read_u16(pc)?))),
            OP_COMPREHEND_LIST => Ok(Op::ComprehendList(Offset(self.read_u16(pc)?))),
            OP_CONTINUE_COMPREHENSION => Ok(Op::ContinueComprehension(self.decode_name(pc)?)),

            OP_CAPTURE => Ok(Op::Capture(self.decode_name(pc)?)),

            OP_MAKE_LAMBDA => Ok(Op::MakeLambda {
                scatter_offset: Offset(self.read_u16(pc)?),
                program_offset: Offset(self.read_u16(pc)?),
                self_var: self.decode_option_name(pc)?,
                num_captured: self.read_u16(pc)?,
            }),

            OP_CALL_LAMBDA => Ok(Op::CallLambda),

            OP_FORK => Ok(Op::Fork {
                fv_offset: Offset(self.read_u16(pc)?),
                id: self.decode_option_name(pc)?,
            }),

            _ => Err(DecodeError::UnknownOpcode(opcode, start_pc)),
        }
    }

    // ========================================================================
    // Helper encoding functions
    // ========================================================================

    fn encode_name(&mut self, name: &Name) {
        self.words.push(name.0); // u16
        self.words.push(name.1 as u16); // u8 -> u16
        self.words.push(name.2); // u16
    }

    fn encode_option_name(&mut self, opt: &Option<Name>) {
        match opt {
            Some(name) => {
                self.words.push(1);
                self.encode_name(name);
            }
            None => self.words.push(0),
        }
    }

    fn encode_i32(&mut self, v: i32) {
        let bytes = v.to_le_bytes();
        self.words.push(u16::from_le_bytes([bytes[0], bytes[1]]));
        self.words.push(u16::from_le_bytes([bytes[2], bytes[3]]));
    }

    fn encode_i64(&mut self, v: i64) {
        let bytes = v.to_le_bytes();
        self.words.push(u16::from_le_bytes([bytes[0], bytes[1]]));
        self.words.push(u16::from_le_bytes([bytes[2], bytes[3]]));
        self.words.push(u16::from_le_bytes([bytes[4], bytes[5]]));
        self.words.push(u16::from_le_bytes([bytes[6], bytes[7]]));
    }

    fn encode_f64(&mut self, v: f64) {
        self.encode_i64(v.to_bits() as i64);
    }

    fn encode_u64(&mut self, v: u64) {
        let bytes = v.to_le_bytes();
        self.words.push(u16::from_le_bytes([bytes[0], bytes[1]]));
        self.words.push(u16::from_le_bytes([bytes[2], bytes[3]]));
        self.words.push(u16::from_le_bytes([bytes[4], bytes[5]]));
        self.words.push(u16::from_le_bytes([bytes[6], bytes[7]]));
    }

    // ========================================================================
    // Helper decoding functions
    // ========================================================================

    fn read_u16(&self, pc: &mut usize) -> Result<u16, DecodeError> {
        if *pc >= self.words.len() {
            return Err(DecodeError::UnexpectedEnd(*pc));
        }
        let val = self.words[*pc];
        *pc += 1;
        Ok(val)
    }

    fn decode_name(&self, pc: &mut usize) -> Result<Name, DecodeError> {
        let offset = self.read_u16(pc)?;
        let scope_depth_word = self.read_u16(pc)?;
        let scope_id = self.read_u16(pc)?;

        // scope_depth is u8, so ensure it fits
        if scope_depth_word > 255 {
            return Err(DecodeError::InvalidEnum(
                scope_depth_word,
                "Name scope_depth",
                *pc - 2,
            ));
        }

        Ok(Name(offset, scope_depth_word as u8, scope_id))
    }

    fn decode_option_name(&self, pc: &mut usize) -> Result<Option<Name>, DecodeError> {
        let has_name = self.read_u16(pc)?;
        match has_name {
            0 => Ok(None),
            1 => Ok(Some(self.decode_name(pc)?)),
            v => Err(DecodeError::InvalidEnum(v, "Option<Name>", *pc - 1)),
        }
    }

    fn decode_i32(&self, pc: &mut usize) -> Result<i32, DecodeError> {
        let w0 = self.read_u16(pc)?;
        let w1 = self.read_u16(pc)?;
        let bytes = [
            (w0 & 0xff) as u8,
            (w0 >> 8) as u8,
            (w1 & 0xff) as u8,
            (w1 >> 8) as u8,
        ];
        Ok(i32::from_le_bytes(bytes))
    }

    fn decode_i64(&self, pc: &mut usize) -> Result<i64, DecodeError> {
        let w0 = self.read_u16(pc)?;
        let w1 = self.read_u16(pc)?;
        let w2 = self.read_u16(pc)?;
        let w3 = self.read_u16(pc)?;
        let bytes = [
            (w0 & 0xff) as u8,
            (w0 >> 8) as u8,
            (w1 & 0xff) as u8,
            (w1 >> 8) as u8,
            (w2 & 0xff) as u8,
            (w2 >> 8) as u8,
            (w3 & 0xff) as u8,
            (w3 >> 8) as u8,
        ];
        Ok(i64::from_le_bytes(bytes))
    }

    fn decode_f64(&self, pc: &mut usize) -> Result<f64, DecodeError> {
        Ok(f64::from_bits(self.decode_i64(pc)? as u64))
    }

    fn decode_u64(&self, pc: &mut usize) -> Result<u64, DecodeError> {
        Ok(self.decode_i64(pc)? as u64)
    }
}

impl Default for OpStream {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_var::{ErrorCode::*, Symbol, VarType::*};

    #[test]
    fn test_simple_opcodes() {
        let ops = vec![Op::Add, Op::Sub, Op::Mul, Op::Div, Op::Return];

        let mut stream = OpStream::new();
        for op in &ops {
            stream.encode(op);
        }

        let mut pc = 0;
        let mut decoded = Vec::new();
        while pc < stream.len() {
            decoded.push(stream.decode_at(&mut pc).unwrap());
        }

        assert_eq!(ops, decoded);
    }

    #[test]
    fn test_opcodes_with_args() {
        let ops = vec![
            Op::ImmInt(42),
            Op::ImmFloat(2.5),
            Op::Push(Name(1, 2, 3)),
            Op::Jump { label: Label(10) },
            Op::Fork {
                fv_offset: Offset(5),
                id: Some(Name(0, 0, 1)),
            },
        ];

        let mut stream = OpStream::new();
        for op in &ops {
            stream.encode(op);
        }

        let mut pc = 0;
        let mut decoded = Vec::new();
        while pc < stream.len() {
            decoded.push(stream.decode_at(&mut pc).unwrap());
        }

        assert_eq!(ops, decoded);
    }

    #[test]
    fn test_complex_opcodes() {
        let ops = vec![
            Op::TryFinally {
                end_label: Label(100),
                environment_width: 5,
            },
            Op::MakeLambda {
                scatter_offset: Offset(1),
                program_offset: Offset(2),
                self_var: None,
                num_captured: 3,
            },
            Op::WhileId {
                id: Name(1, 0, 0),
                end_label: Label(50),
                environment_width: 2,
            },
        ];

        let mut stream = OpStream::new();
        for op in &ops {
            stream.encode(op);
        }

        let mut pc = 0;
        let mut decoded = Vec::new();
        while pc < stream.len() {
            decoded.push(stream.decode_at(&mut pc).unwrap());
        }

        assert_eq!(ops, decoded);
    }

    #[test]
    fn test_all_immediate_types() {
        let ops = vec![
            Op::ImmInt(-12345),
            Op::ImmBigInt(i64::MAX),
            Op::ImmFloat(std::f64::consts::PI),
            Op::ImmNone,
            Op::ImmEmptyList,
        ];

        let mut stream = OpStream::new();
        for op in &ops {
            stream.encode(op);
        }

        let mut pc = 0;
        let mut decoded = Vec::new();
        while pc < stream.len() {
            decoded.push(stream.decode_at(&mut pc).unwrap());
        }

        assert_eq!(ops, decoded);
    }

    #[test]
    fn test_error_codes() {
        let ops = vec![
            Op::ImmErr(E_NONE),
            Op::ImmErr(E_TYPE),
            Op::ImmErr(E_PERM),
            Op::ImmErr(E_INVARG),
            Op::ImmErr(ErrCustom(Symbol::mk("MY_ERROR"))),
        ];

        let mut stream = OpStream::new();
        for op in &ops {
            stream.encode(op);
        }

        let mut pc = 0;
        let mut decoded = Vec::new();
        while pc < stream.len() {
            decoded.push(stream.decode_at(&mut pc).unwrap());
        }

        assert_eq!(ops, decoded);
    }

    #[test]
    fn test_var_types() {
        let ops = vec![
            Op::ImmType(TYPE_INT),
            Op::ImmType(TYPE_STR),
            Op::ImmType(TYPE_LIST),
            Op::ImmType(TYPE_OBJ),
            Op::ImmType(TYPE_FLOAT),
            Op::ImmType(TYPE_LAMBDA),
        ];

        let mut stream = OpStream::new();
        for op in &ops {
            stream.encode(op);
        }

        let mut pc = 0;
        let mut decoded = Vec::new();
        while pc < stream.len() {
            decoded.push(stream.decode_at(&mut pc).unwrap());
        }

        assert_eq!(ops, decoded);
    }

    #[test]
    fn test_control_flow_opcodes() {
        let ops = vec![
            Op::If(Label(10), 2),
            Op::Eif(Label(20), 3),
            Op::While {
                jump_label: Label(5),
                environment_width: 1,
            },
            Op::TryCatch {
                handler_label: Label(15),
                end_label: Label(30),
            },
            Op::BeginScope {
                num_bindings: 5,
                end_label: Label(50),
            },
            Op::EndScope { num_bindings: 5 },
        ];

        let mut stream = OpStream::new();
        for op in &ops {
            stream.encode(op);
        }

        let mut pc = 0;
        let mut decoded = Vec::new();
        while pc < stream.len() {
            decoded.push(stream.decode_at(&mut pc).unwrap());
        }

        assert_eq!(ops, decoded);
    }

    #[test]
    fn test_all_opcodes_roundtrip() {
        // Test a comprehensive mix of opcodes
        let ops = vec![
            // Arithmetic
            Op::Add,
            Op::Sub,
            Op::Mul,
            Op::Div,
            Op::Mod,
            Op::Exp,
            // Logic
            Op::And(Label(10)),
            Op::Or(Label(20)),
            Op::Not,
            // Comparisons
            Op::Eq,
            Op::Ne,
            Op::Lt,
            Op::Le,
            Op::Gt,
            Op::Ge,
            // Stack ops
            Op::Push(Name(1, 0, 0)),
            Op::Put(Name(2, 1, 5)),
            Op::Pop,
            // Immediates
            Op::ImmInt(42),
            Op::ImmFloat(2.5),
            Op::ImmNone,
            // Control flow
            Op::Jump { label: Label(100) },
            Op::Return,
            Op::Return0,
            Op::Done,
        ];

        let mut stream = OpStream::new();
        for op in &ops {
            stream.encode(op);
        }

        let mut pc = 0;
        let mut decoded = Vec::new();
        while pc < stream.len() {
            decoded.push(stream.decode_at(&mut pc).unwrap());
        }

        assert_eq!(ops, decoded);
    }

    #[test]
    fn test_name_with_different_scope_depths() {
        let ops = vec![
            Op::Push(Name(0, 0, 0)),     // scope depth 0
            Op::Push(Name(1, 1, 10)),    // scope depth 1
            Op::Push(Name(5, 255, 100)), // scope depth 255 (max u8)
            Op::Put(Name(10, 2, 50)),
        ];

        let mut stream = OpStream::new();
        for op in &ops {
            stream.encode(op);
        }

        let mut pc = 0;
        let mut decoded = Vec::new();
        while pc < stream.len() {
            decoded.push(stream.decode_at(&mut pc).unwrap());
        }

        assert_eq!(ops, decoded);
    }

    #[test]
    fn test_optional_name_encoding() {
        let ops = vec![
            Op::Fork {
                fv_offset: Offset(1),
                id: None,
            },
            Op::Fork {
                fv_offset: Offset(2),
                id: Some(Name(1, 0, 0)),
            },
            Op::MakeLambda {
                scatter_offset: Offset(5),
                program_offset: Offset(10),
                self_var: None,
                num_captured: 0,
            },
            Op::MakeLambda {
                scatter_offset: Offset(6),
                program_offset: Offset(11),
                self_var: Some(Name(2, 1, 5)),
                num_captured: 3,
            },
        ];

        let mut stream = OpStream::new();
        for op in &ops {
            stream.encode(op);
        }

        let mut pc = 0;
        let mut decoded = Vec::new();
        while pc < stream.len() {
            decoded.push(stream.decode_at(&mut pc).unwrap());
        }

        assert_eq!(ops, decoded);
    }

    #[test]
    fn test_decode_error_unexpected_end() {
        let mut stream = OpStream::new();
        stream.encode(&Op::Push(Name(1, 0, 0)));

        // Truncate the stream to cause unexpected end
        let words = stream.into_words();
        let truncated = OpStream::from_words(words[..2].to_vec()); // Not enough for full Name

        let mut pc = 0;
        let result = truncated.decode_at(&mut pc);
        assert!(matches!(result, Err(DecodeError::UnexpectedEnd(_))));
    }

    #[test]
    fn test_invalid_opcode() {
        let stream = OpStream::from_words(vec![9999]); // Invalid opcode number
        let mut pc = 0;
        let result = stream.decode_at(&mut pc);
        assert!(matches!(result, Err(DecodeError::UnknownOpcode(9999, 0))));
    }
}
