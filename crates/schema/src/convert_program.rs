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

//! Conversion between Program (runtime) and StoredProgram (FlatBuffer wire format).

use crate::{
    convert_var::{ConversionContext, var_to_flatbuffer_internal},
    define_enum_mapping,
    opcode_stream::{OpStream, error_code_from_discriminant, error_code_to_discriminant},
    program as fb,
};

// Generate bidirectional DeclType conversion
define_enum_mapping! {
    moor_var::program::DeclType <=> fb::StoredDeclType {
        Global <=> Global,
        Let <=> Let,
        Assign <=> Assign,
        For <=> For,
        Unknown <=> Unknown,
        Register <=> Register,
        Except <=> Except,
        WhileLabel <=> WhileLabel,
        ForkLabel <=> ForkLabel,
    }
}
use crate::convert::var_from_db_flatbuffer_ref;
use byteview::ByteView;
use moor_common::builtins::builtin_signature_for_ids;
use moor_var::program::{
    labels::{JumpLabel, Label, Offset},
    names::{Name, VarName},
    opcode::{BuiltinId, ForSequenceOperand, Op, ScatterArgs, ScatterLabel},
    program::{PrgInner, Program},
    stored_program::StoredProgram,
};
use planus::{ReadAsRoot, WriteAsOffset};
use std::collections::HashSet;
use triomphe::Arc;

const STORED_PROGRAM_VERSION: u16 = 3;

// Helper to encode a Name into FlatBuffer StoredName
fn encode_name(name: &Name) -> fb::StoredName {
    fb::StoredName {
        offset: name.0,
        scope_depth: name.1,
        scope_id: name.2,
    }
}

// Internal helper: encode to StoredMooRProgram (for recursive lambda encoding)
fn encode_moor_program(program: &Program) -> Result<fb::StoredMooRProgram, EncodeError> {
    let builtin_signature = builtin_signature_for_ids(used_builtin_ids(program));

    // 1. Encode main_vector using OpStream
    let mut main_stream = OpStream::new();
    for op in program.main_vector() {
        main_stream.encode(op);
    }
    let main_vector = main_stream.into_words();

    // 2. Encode fork_vectors
    let fork_vectors: Vec<fb::ForkVector> = program
        .0
        .fork_vectors
        .iter()
        .map(|(offset, ops)| {
            let mut fork_stream = OpStream::new();
            for op in ops {
                fork_stream.encode(op);
            }
            fb::ForkVector {
                offset: *offset as u64,
                opcodes: fork_stream.into_words(),
            }
        })
        .collect();

    // 3. Encode literals as Var FlatBuffers (not bincode)
    let literals: Result<Vec<crate::var::Var>, EncodeError> = program
        .literals()
        .iter()
        .map(|lit| {
            var_to_flatbuffer_internal(lit, ConversionContext::Database)
                .map_err(|e| EncodeError::EncodeFailed(format!("Failed to encode literal: {e}")))
        })
        .collect();
    let literals = literals?;

    // 4. Encode jump labels
    let jump_labels: Vec<fb::StoredJumpLabel> = program
        .jump_labels()
        .iter()
        .map(|jl| {
            let name = jl.name.as_ref().map(|n| {
                Box::new(fb::StoredName {
                    offset: n.0,
                    scope_depth: n.1,
                    scope_id: n.2,
                })
            });
            fb::StoredJumpLabel {
                id: jl.id.0,
                position: jl.position.0,
                name,
            }
        })
        .collect();

    // 5. Encode variable names (full Names structure with decls HashMap)
    let decl_pairs: Vec<fb::StoredNameDeclPair> = program
        .var_names()
        .decls
        .iter()
        .map(|(name, decl)| {
            let decl_type: fb::StoredDeclType = (&decl.decl_type).into();

            // Encode VarName
            let var_name_union = match decl.identifier.nr {
                VarName::Named(sym) => {
                    fb::StoredVarNameUnion::StoredNamedVar(Box::new(fb::StoredNamedVar {
                        symbol: Box::new(crate::common::Symbol {
                            value: sym.as_string(),
                        }),
                    }))
                }
                VarName::Register(reg_num) => {
                    fb::StoredVarNameUnion::StoredRegisterVar(Box::new(fb::StoredRegisterVar {
                        register_num: reg_num,
                    }))
                }
            };

            // Encode Variable
            let identifier = Box::new(fb::StoredVariable {
                id: decl.identifier.id,
                scope_id: decl.identifier.scope_id,
                var_name: var_name_union,
            });

            // Encode Decl
            let stored_decl = Box::new(fb::StoredDecl {
                decl_type,
                identifier,
                depth: decl.depth as u64,
                constant: decl.constant,
                scope_id: decl.scope_id,
            });

            // Encode Name
            let stored_name = Box::new(encode_name(name));

            fb::StoredNameDeclPair {
                name: stored_name,
                decl: stored_decl,
            }
        })
        .collect();

    let var_names = Box::new(fb::StoredNames {
        global_width: program.var_names().global_width as u64,
        decls: decl_pairs,
    });

    // 7. Encode scatter tables
    let scatter_tables: Vec<fb::StoredScatterArgs> = program
        .0
        .scatter_tables
        .iter()
        .map(|st| {
            let labels = st
                .labels
                .iter()
                .map(|label| {
                    use moor_var::program::opcode::ScatterLabel as SL;
                    let fb_label = match label {
                        SL::Required(name) => fb::StoredScatterLabelUnion::StoredScatterRequired(
                            Box::new(fb::StoredScatterRequired {
                                name: Box::new(encode_name(name)),
                            }),
                        ),
                        SL::Optional(name, default_label) => {
                            fb::StoredScatterLabelUnion::StoredScatterOptional(Box::new(
                                fb::StoredScatterOptional {
                                    name: Box::new(encode_name(name)),
                                    default_label: default_label.map(|l| l.0).unwrap_or(0),
                                    has_default: default_label.is_some(),
                                },
                            ))
                        }
                        SL::Rest(name) => fb::StoredScatterLabelUnion::StoredScatterRest(Box::new(
                            fb::StoredScatterRest {
                                name: Box::new(encode_name(name)),
                            },
                        )),
                    };
                    fb::StoredScatterLabel { label: fb_label }
                })
                .collect();

            fb::StoredScatterArgs {
                labels,
                done: st.done.0,
            }
        })
        .collect();

    // 8. Encode for-sequence operands
    let for_sequence_operands: Vec<fb::StoredForSequenceOperand> = program
        .0
        .for_sequence_operands
        .iter()
        .map(|fso| fb::StoredForSequenceOperand {
            value_bind: Box::new(encode_name(&fso.value_bind)),
            key_bind: fso.key_bind.as_ref().map(|n| Box::new(encode_name(n))),
            end_label: fso.end_label.0,
            environment_width: fso.environment_width,
        })
        .collect();

    // 9. Encode for-range operands
    let for_range_operands: Vec<fb::StoredForRangeOperand> = program
        .0
        .for_range_operands
        .iter()
        .map(|fro| fb::StoredForRangeOperand {
            loop_variable: Box::new(encode_name(&fro.loop_variable)),
            end_label: fro.end_label.0,
            environment_width: fro.environment_width,
        })
        .collect();

    // 10. Encode range comprehensions
    let range_comprehensions: Vec<fb::StoredRangeComprehend> = program
        .0
        .range_comprehensions
        .iter()
        .map(|rc| fb::StoredRangeComprehend {
            position: Box::new(encode_name(&rc.position)),
            end_of_range_register: Box::new(encode_name(&rc.end_of_range_register)),
            end_label: rc.end_label.0,
        })
        .collect();

    // 11. Encode list comprehensions
    let list_comprehensions: Vec<fb::StoredListComprehend> = program
        .0
        .list_comprehensions
        .iter()
        .map(|lc| fb::StoredListComprehend {
            position_register: Box::new(encode_name(&lc.position_register)),
            list_register: Box::new(encode_name(&lc.list_register)),
            item_variable: Box::new(encode_name(&lc.item_variable)),
            end_label: lc.end_label.0,
        })
        .collect();

    // 12. Encode error operands with complete information including symbols for custom errors
    let error_operands_full: Vec<fb::StoredErrorOperand> = program
        .0
        .error_operands
        .iter()
        .map(|err| {
            let discriminant = error_code_to_discriminant(err);
            let symbol = if let moor_var::ErrorCode::ErrCustom(sym) = err {
                Some(sym.as_string())
            } else {
                None
            };
            fb::StoredErrorOperand {
                discriminant,
                symbol,
            }
        })
        .collect();

    // 13. Encode lambda programs (recursive)
    let lambda_programs: Result<Vec<fb::StoredMooRProgram>, EncodeError> = program
        .0
        .lambda_programs
        .iter()
        .map(encode_moor_program)
        .collect();
    let lambda_programs = lambda_programs?;

    // 14. Encode line number spans
    let line_number_spans: Vec<fb::LineSpan> = program
        .line_number_spans()
        .iter()
        .map(|(offset, line_number)| fb::LineSpan {
            offset: *offset as u64,
            line_number: *line_number as u64,
        })
        .collect();

    // 15. Encode fork line number spans
    let fork_line_number_spans: Vec<fb::ForkLineSpans> = program
        .0
        .fork_line_number_spans
        .iter()
        .map(|spans| {
            let spans_vec = spans
                .iter()
                .map(|(offset, line_number)| fb::LineSpan {
                    offset: *offset as u64,
                    line_number: *line_number as u64,
                })
                .collect();
            fb::ForkLineSpans { spans: spans_vec }
        })
        .collect();

    // Build the FlatBuffer struct
    Ok(fb::StoredMooRProgram {
        version: STORED_PROGRAM_VERSION,
        builtin_signature,
        main_vector,
        fork_vectors,
        literals,
        jump_labels,
        var_names,
        scatter_tables,
        for_sequence_operands,
        for_range_operands,
        range_comprehensions,
        list_comprehensions,
        error_operands_full,
        lambda_programs,
        line_number_spans,
        fork_line_number_spans,
    })
}

// Public API: encode to StoredProgram wrapper (for embedding in other schemas)
pub fn encode_program_to_fb(program: &Program) -> Result<fb::StoredProgram, EncodeError> {
    let moor_program = encode_moor_program(program)?;
    Ok(fb::StoredProgram {
        language: fb::StoredProgramLanguage::StoredMooRProgram(Box::new(moor_program)),
    })
}

/// Convert a FlatBuffer StoredProgram struct to a Program (runtime format)
pub fn decode_stored_program_struct(stored: &fb::StoredProgram) -> Result<Program, DecodeError> {
    // Convert owned struct to bytes and back to ref for decoding
    let mut builder = planus::Builder::new();
    let offset = stored.prepare(&mut builder);
    let bytes = builder.finish(offset, None);
    let fb_ref = fb::StoredProgramRef::read_as_root(bytes)
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read StoredProgram: {e}")))?;

    // Extract language union and decode
    let language = fb_ref
        .language()
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read language union: {e}")))?;

    match language {
        fb::StoredProgramLanguageRef::StoredMooRProgram(moor_ref) => decode_fb_program(moor_ref),
    }
}

/// Convert a FlatBuffer StoredProgramRef directly to a Program (runtime format).
/// Avoids the intermediate owned struct allocation that decode_stored_program_struct does.
pub fn decode_stored_program_ref(fb_ref: fb::StoredProgramRef<'_>) -> Result<Program, DecodeError> {
    let language = fb_ref
        .language()
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read language union: {e}")))?;

    match language {
        fb::StoredProgramLanguageRef::StoredMooRProgram(moor_ref) => decode_fb_program(moor_ref),
    }
}

/// Convert a Program to a StoredProgram (FlatBuffer format)
pub fn program_to_stored(program: &Program) -> Result<StoredProgram, EncodeError> {
    let mut builder = planus::Builder::new();

    // Encode to wrapped FlatBuffer format
    let stored_program = encode_program_to_fb(program)?;

    let offset = stored_program.prepare(&mut builder);
    let bytes = builder.finish(offset, None);

    Ok(StoredProgram::from_bytes(ByteView::from(bytes)))
}

/// Convert a StoredProgram to a Program (runtime format)
pub fn stored_to_program(stored: &StoredProgram) -> Result<Program, DecodeError> {
    // 1. Read FlatBuffer wrapper
    let fb_program = fb::StoredProgramRef::read_as_root(stored.as_bytes())
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read FlatBuffer: {e}")))?;

    // 2. Extract language union
    let language = fb_program
        .language()
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read language union: {e}")))?;

    // 3. Match on union variant and decode
    match language {
        fb::StoredProgramLanguageRef::StoredMooRProgram(moor_ref) => decode_fb_program(moor_ref),
    }
}

// Helper functions for decoding

fn decode_stored_name(name_ref: fb::StoredNameRef) -> Result<Name, DecodeError> {
    Ok(Name(
        fb_decode!(name_ref, offset),
        fb_decode!(name_ref, scope_depth),
        fb_decode!(name_ref, scope_id),
    ))
}

fn decode_names_from_fb(
    fb_var_names: fb::StoredNamesRef,
    error_prefix: &str,
) -> Result<moor_var::program::names::Names, DecodeError> {
    let global_width = fb_decode_ctx!(fb_var_names, global_width, error_prefix) as usize;
    let decls_vec = fb_decode_ctx!(fb_var_names, decls, error_prefix);

    let mut decls = std::collections::HashMap::new();
    for pair_result in decls_vec {
        let pair = pair_result.map_err(|e| {
            DecodeError::DecodeFailed(format!("{error_prefix} name-decl pair: {e}"))
        })?;

        let name_ref = fb_decode_ctx!(pair, name, error_prefix);
        let name = decode_stored_name(name_ref)?;

        let decl_ref = fb_decode_ctx!(pair, decl, error_prefix);
        let fb_decl_type = fb_decode_ctx!(decl_ref, decl_type, error_prefix);
        let decl_type: moor_var::program::DeclType = fb_decl_type.into();

        let identifier_ref = fb_decode_ctx!(decl_ref, identifier, error_prefix);
        let var_name_union = fb_decode_ctx!(identifier_ref, var_name, error_prefix);

        let var_name = match var_name_union {
            fb::StoredVarNameUnionRef::StoredNamedVar(named) => {
                let symbol_ref = fb_decode_ctx!(named, symbol, error_prefix);
                let symbol_str = fb_decode_ctx!(symbol_ref, value, error_prefix);
                VarName::Named(moor_var::Symbol::mk(symbol_str))
            }
            fb::StoredVarNameUnionRef::StoredRegisterVar(reg) => {
                VarName::Register(fb_decode_ctx!(reg, register_num, error_prefix))
            }
        };

        let identifier = moor_var::program::names::Variable {
            id: fb_decode_ctx!(identifier_ref, id, error_prefix),
            scope_id: fb_decode_ctx!(identifier_ref, scope_id, error_prefix),
            nr: var_name,
        };

        let decl = moor_var::program::Decl {
            decl_type,
            identifier,
            depth: fb_decode_ctx!(decl_ref, depth, error_prefix) as usize,
            constant: fb_decode_ctx!(decl_ref, constant, error_prefix),
            scope_id: fb_decode_ctx!(decl_ref, scope_id, error_prefix),
        };

        decls.insert(name, decl);
    }

    Ok(moor_var::program::names::Names {
        global_width,
        decls,
    })
}

pub fn decode_fb_program(fb_prog_ref: fb::StoredMooRProgramRef) -> Result<Program, DecodeError> {
    let version = fb_decode!(fb_prog_ref, version);
    if version != STORED_PROGRAM_VERSION {
        return Err(DecodeError::DecodeFailed(format!(
            "Stored program version {version} does not match expected {STORED_PROGRAM_VERSION}"
        )));
    }
    let expected_builtin_signature = fb_decode!(fb_prog_ref, builtin_signature);

    // Decode main_vector
    let main_words = fb_decode!(fb_prog_ref, main_vector)
        .to_vec()
        .map_err(|e| decode_err!("Failed to convert main_vector to vec: {}", e))?;
    let main_stream = OpStream::from_words(main_words);
    let mut main_vector = Vec::new();
    let mut pc = 0;
    while pc < main_stream.len() {
        let op = main_stream.decode_at(&mut pc).map_err(|e| {
            DecodeError::DecodeFailed(format!("Failed to decode lambda opcode: {e}"))
        })?;
        main_vector.push(op);
    }

    // Decode fork_vectors
    let fb_fork_vectors = fb_decode!(fb_prog_ref, fork_vectors);
    let fork_vectors: Result<Vec<(usize, Vec<moor_var::program::opcode::Op>)>, DecodeError> =
        fb_fork_vectors
            .iter()
            .map(|fv_result| {
                let fv = fv_result.map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read lambda fork_vector: {e}"))
                })?;
                let offset = fv.offset().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read lambda fork offset: {e}"))
                })? as usize;
                let words = fv
                    .opcodes()
                    .map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read lambda fork opcodes: {e}"
                        ))
                    })?
                    .to_vec()
                    .map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to convert lambda fork opcodes to vec: {e}"
                        ))
                    })?;
                let stream = OpStream::from_words(words);
                let mut ops = Vec::new();
                let mut pc = 0;
                while pc < stream.len() {
                    let op = stream.decode_at(&mut pc).map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to decode lambda fork opcode: {e}"
                        ))
                    })?;
                    ops.push(op);
                }
                Ok((offset, ops))
            })
            .collect();
    let fork_vectors = fork_vectors?;

    // Decode literals
    let fb_literals = fb_decode!(fb_prog_ref, literals);
    let literals: Result<Vec<moor_var::Var>, DecodeError> = fb_literals
        .iter()
        .map(|lit_result| {
            let lit_ref = lit_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read lambda literal: {e}"))
            })?;
            var_from_db_flatbuffer_ref(lit_ref).map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to decode lambda literal: {e}"))
            })
        })
        .collect();
    let literals = literals?;

    // Decode jump labels
    let fb_jump_labels = fb_decode!(fb_prog_ref, jump_labels);
    let jump_labels: Result<Vec<moor_var::program::labels::JumpLabel>, DecodeError> =
        fb_jump_labels
            .iter()
            .map(|jl_result| {
                let jl = jl_result.map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read lambda jump label: {e}"))
                })?;
                Ok(JumpLabel {
                    id: Label(jl.id().map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read lambda jump label id: {e}"
                        ))
                    })?),
                    position: Offset(jl.position().map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read lambda jump label position: {e}"
                        ))
                    })?),
                    name: jl
                        .name()
                        .ok()
                        .flatten()
                        .map(|n| decode_stored_name(n))
                        .transpose()?,
                })
            })
            .collect();
    let jump_labels = jump_labels?;

    // Decode variable names (full Names structure with decls HashMap)
    let fb_var_names = fb_decode!(fb_prog_ref, var_names);
    let var_names = decode_names_from_fb(fb_var_names, "Failed to read lambda")?;

    // Decode operand tables
    let scatter_tables = decode_scatter_tables(&fb_prog_ref)?;
    let for_sequence_operands = decode_for_sequence_operands(&fb_prog_ref)?;
    let for_range_operands = decode_for_range_operands(&fb_prog_ref)?;
    let range_comprehensions = decode_range_comprehensions(&fb_prog_ref)?;
    let list_comprehensions = decode_list_comprehensions(&fb_prog_ref)?;

    // Decode error operands - require full format with error symbols
    let fb_error_operands_full = fb_decode!(fb_prog_ref, error_operands_full);
    let error_operands = fb_error_operands_full
        .iter()
        .map(|operand_result| {
            let operand = operand_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read error operand: {e}"))
            })?;

            let disc = operand.discriminant().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read discriminant: {e}"))
            })?;

            if disc == 255 {
                // Custom error - must have symbol
                let symbol_str = operand
                    .symbol()
                    .map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read custom error symbol: {e}"
                        ))
                    })?
                    .ok_or_else(|| {
                        DecodeError::DecodeFailed("Custom error missing symbol field".to_string())
                    })?;
                Ok(moor_var::ErrorCode::ErrCustom(moor_var::Symbol::mk(
                    symbol_str,
                )))
            } else {
                error_code_from_discriminant(disc).ok_or_else(|| {
                    DecodeError::DecodeFailed(format!("Invalid error code discriminant: {disc}"))
                })
            }
        })
        .collect::<Result<Vec<_>, DecodeError>>()?;

    // Decode lambda programs (recursive)
    let fb_lambda_programs = fb_decode!(fb_prog_ref, lambda_programs);
    let lambda_programs: Result<Vec<Program>, DecodeError> = fb_lambda_programs
        .iter()
        .map(|lp_result| {
            let lp = lp_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read nested lambda program: {e}"))
            })?;
            decode_fb_program(lp)
        })
        .collect();
    let lambda_programs = lambda_programs?;

    // Decode line number spans
    let fb_line_spans = fb_decode!(fb_prog_ref, line_number_spans);
    let line_number_spans: Result<Vec<(usize, usize)>, DecodeError> = fb_line_spans
        .iter()
        .map(|ls_result| {
            let ls = ls_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read lambda line span: {e}"))
            })?;
            Ok((
                ls.offset().map_err(|e| {
                    DecodeError::DecodeFailed(format!(
                        "Failed to read lambda line span offset: {e}"
                    ))
                })? as usize,
                ls.line_number().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read lambda line number: {e}"))
                })? as usize,
            ))
        })
        .collect();
    let line_number_spans = line_number_spans?;

    // Decode fork line number spans
    let fb_fork_line_spans = fb_decode!(fb_prog_ref, fork_line_number_spans);
    let fork_line_number_spans: Result<Vec<Vec<(usize, usize)>>, DecodeError> = fb_fork_line_spans
        .iter()
        .map(|fls_result| {
            let fls = fls_result.map_err(|e| {
                DecodeError::DecodeFailed(format!(
                    "Failed to read lambda fork line span group: {e}"
                ))
            })?;
            let spans = fls.spans().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read lambda fork line spans: {e}"))
            })?;
            spans
                .iter()
                .map(|ls_result| {
                    let ls = ls_result.map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read lambda fork line span: {e}"
                        ))
                    })?;
                    Ok((
                        ls.offset().map_err(|e| {
                            DecodeError::DecodeFailed(format!(
                                "Failed to read lambda fork line span offset: {e}"
                            ))
                        })? as usize,
                        ls.line_number().map_err(|e| {
                            DecodeError::DecodeFailed(format!(
                                "Failed to read lambda fork line number: {e}"
                            ))
                        })? as usize,
                    ))
                })
                .collect()
        })
        .collect();
    let fork_line_number_spans = fork_line_number_spans?;

    let program = Program(Arc::new(PrgInner {
        literals,
        jump_labels,
        var_names,
        scatter_tables,
        for_sequence_operands,
        for_range_operands,
        range_comprehensions,
        list_comprehensions,
        error_operands,
        lambda_programs,
        main_vector,
        fork_vectors,
        line_number_spans,
        fork_line_number_spans,
    }));

    let actual_builtin_signature = builtin_signature_for_ids(used_builtin_ids(&program));
    if actual_builtin_signature != expected_builtin_signature {
        return Err(DecodeError::DecodeFailed(
            "Stored program builtin signature mismatch; \
             program data is corrupt or builtin table changed"
                .to_string(),
        ));
    }

    Ok(program)
}

fn used_builtin_ids(program: &Program) -> Vec<BuiltinId> {
    let mut ids = HashSet::new();
    collect_builtin_ids(&program.0.main_vector, &mut ids);
    for (_, ops) in &program.0.fork_vectors {
        collect_builtin_ids(ops, &mut ids);
    }
    ids.into_iter().collect()
}

fn collect_builtin_ids(ops: &[Op], ids: &mut HashSet<BuiltinId>) {
    for op in ops {
        if let Op::FuncCall { id } = op {
            ids.insert(*id);
        }
    }
}

fn decode_one_scatter_label(
    sl_result: Result<fb::StoredScatterLabelRef, planus::Error>,
) -> Result<ScatterLabel, DecodeError> {
    let sl = sl_result.map_err(|e| decode_err!("Failed to read scatter label: {}", e))?;
    let fb_label = fb_decode!(sl, label);

    match fb_label {
        fb::StoredScatterLabelUnionRef::StoredScatterRequired(req) => {
            let name = decode_stored_name(fb_decode!(req, name))?;
            Ok(ScatterLabel::Required(name))
        }
        fb::StoredScatterLabelUnionRef::StoredScatterOptional(opt) => {
            let name = decode_stored_name(fb_decode!(opt, name))?;
            let has_default = fb_decode!(opt, has_default);
            let default_label = if has_default {
                Some(Label(fb_decode!(opt, default_label)))
            } else {
                None
            };
            Ok(ScatterLabel::Optional(name, default_label))
        }
        fb::StoredScatterLabelUnionRef::StoredScatterRest(rest) => {
            let name = decode_stored_name(fb_decode!(rest, name))?;
            Ok(ScatterLabel::Rest(name))
        }
    }
}

fn decode_scatter_tables(
    fb_program: &fb::StoredMooRProgramRef,
) -> Result<Vec<ScatterArgs>, DecodeError> {
    let fb_scatter_tables = fb_decode!(fb_program, scatter_tables);

    fb_scatter_tables
        .iter()
        .map(|st_result| {
            let st = st_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read scatter table: {e}"))
            })?;
            let fb_labels = st.labels().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read scatter labels: {e}"))
            })?;

            let labels: Result<Vec<ScatterLabel>, DecodeError> =
                fb_labels.iter().map(decode_one_scatter_label).collect();

            let done_val = st.done().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read scatter done: {e}"))
            })?;

            Ok(ScatterArgs {
                labels: labels?,
                done: Label(done_val),
            })
        })
        .collect()
}

fn decode_for_sequence_operands(
    fb_program: &fb::StoredMooRProgramRef,
) -> Result<Vec<moor_var::program::opcode::ForSequenceOperand>, DecodeError> {
    let fb_operands = fb_decode!(fb_program, for_sequence_operands);

    fb_operands
        .iter()
        .map(|fso_result| {
            let fso = fso_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read for_sequence_operand: {e}"))
            })?;
            Ok(ForSequenceOperand {
                value_bind: decode_stored_name(fso.value_bind().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read value_bind: {e}"))
                })?)?,
                key_bind: fso
                    .key_bind()
                    .ok()
                    .flatten()
                    .map(|n| decode_stored_name(n))
                    .transpose()?,
                end_label: Label(fso.end_label().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read end_label: {e}"))
                })?),
                environment_width: fso.environment_width().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read environment_width: {e}"))
                })?,
            })
        })
        .collect()
}

fn decode_for_range_operands(
    fb_program: &fb::StoredMooRProgramRef,
) -> Result<Vec<moor_var::program::opcode::ForRangeOperand>, DecodeError> {
    use moor_var::program::{labels::Label, opcode::ForRangeOperand};

    let fb_operands = fb_decode!(fb_program, for_range_operands);

    fb_operands
        .iter()
        .map(|fro_result| {
            let fro = fro_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read for_range_operand: {e}"))
            })?;
            Ok(ForRangeOperand {
                loop_variable: decode_stored_name(fro.loop_variable().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read loop_variable: {e}"))
                })?)?,
                end_label: Label(fro.end_label().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read end_label: {e}"))
                })?),
                environment_width: fro.environment_width().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read environment_width: {e}"))
                })?,
            })
        })
        .collect()
}

fn decode_range_comprehensions(
    fb_program: &fb::StoredMooRProgramRef,
) -> Result<Vec<moor_var::program::opcode::RangeComprehend>, DecodeError> {
    use moor_var::program::{labels::Label, opcode::RangeComprehend};

    let fb_comprehensions = fb_decode!(fb_program, range_comprehensions);

    fb_comprehensions
        .iter()
        .map(|rc_result| {
            let rc = rc_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read range_comprehension: {e}"))
            })?;
            Ok(RangeComprehend {
                position: decode_stored_name(rc.position().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read position: {e}"))
                })?)?,
                end_of_range_register: decode_stored_name(rc.end_of_range_register().map_err(
                    |e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read end_of_range_register: {e}"
                        ))
                    },
                )?)?,
                end_label: Label(rc.end_label().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read end_label: {e}"))
                })?),
            })
        })
        .collect()
}

fn decode_list_comprehensions(
    fb_program: &fb::StoredMooRProgramRef,
) -> Result<Vec<moor_var::program::opcode::ListComprehend>, DecodeError> {
    use moor_var::program::{labels::Label, opcode::ListComprehend};

    let fb_comprehensions = fb_decode!(fb_program, list_comprehensions);

    fb_comprehensions
        .iter()
        .map(|lc_result| {
            let lc = lc_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read list_comprehension: {e}"))
            })?;
            Ok(ListComprehend {
                position_register: decode_stored_name(lc.position_register().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read position_register: {e}"))
                })?)?,
                list_register: decode_stored_name(lc.list_register().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read list_register: {e}"))
                })?)?,
                item_variable: decode_stored_name(lc.item_variable().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read item_variable: {e}"))
                })?)?,
                end_label: Label(lc.end_label().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read end_label: {e}"))
                })?),
            })
        })
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("Failed to encode: {0}")]
    EncodeFailed(String),
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("Failed to decode: {0}")]
    DecodeFailed(String),
}
