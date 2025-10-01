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

//! Conversion between Program (runtime) and StoredProgram (FlatBuffer wire format).

use crate::opcode_stream::{OpStream, error_code_from_discriminant, error_code_to_discriminant};
use byteview::ByteView;
use moor_common::schema::program as fb;
use moor_var::{
    AsByteBuffer, ErrorCode,
    program::{
        labels::Label,
        names::Name,
        opcode::{ScatterArgs, ScatterLabel},
        program::Program,
        stored_program::StoredProgram,
    },
};
use planus::WriteAsOffset;

// Helper to encode a Name into FlatBuffer StoredName
fn encode_name(name: &Name) -> fb::StoredName {
    fb::StoredName {
        offset: name.0,
        scope_depth: name.1,
        scope_id: name.2,
    }
}

// Helper to convert a Program to FlatBuffer StoredProgram struct (for recursive encoding)
fn encode_program_to_fb(program: &Program) -> Result<fb::StoredProgram, EncodeError> {
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

    // 3. Encode literals as VarBytes
    let literals: Result<Vec<moor_common::schema::common::VarBytes>, EncodeError> = program
        .literals()
        .iter()
        .map(|lit| {
            lit.with_byte_buffer(|bytes| moor_common::schema::common::VarBytes {
                data: bytes.to_vec(),
            })
            .map_err(|e| EncodeError::EncodeFailed(format!("Failed to encode literal: {}", e)))
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
            use moor_var::program::names::VarName;

            // Encode DeclType
            let decl_type = match decl.decl_type {
                moor_var::program::DeclType::Global => fb::StoredDeclType::Global,
                moor_var::program::DeclType::Let => fb::StoredDeclType::Let,
                moor_var::program::DeclType::Assign => fb::StoredDeclType::Assign,
                moor_var::program::DeclType::For => fb::StoredDeclType::For,
                moor_var::program::DeclType::Unknown => fb::StoredDeclType::Unknown,
                moor_var::program::DeclType::Register => fb::StoredDeclType::Register,
                moor_var::program::DeclType::Except => fb::StoredDeclType::Except,
                moor_var::program::DeclType::WhileLabel => fb::StoredDeclType::WhileLabel,
                moor_var::program::DeclType::ForkLabel => fb::StoredDeclType::ForkLabel,
            };

            // Encode VarName
            let var_name_union = match decl.identifier.nr {
                VarName::Named(sym) => {
                    fb::StoredVarNameUnion::StoredNamedVar(Box::new(fb::StoredNamedVar {
                        symbol: Box::new(moor_common::schema::common::Symbol {
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

    // 6. Build symbol table (for now, empty - could be used for deduplication in future)
    let symbol_table: Vec<moor_common::schema::common::Symbol> = vec![];

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

    // 12. Encode error operands
    let error_operands: Vec<u8> = program
        .0
        .error_operands
        .iter()
        .map(error_code_to_discriminant)
        .collect();

    // 13. Encode lambda programs (recursive)
    let lambda_programs: Result<Vec<fb::StoredProgram>, EncodeError> = program
        .0
        .lambda_programs
        .iter()
        .map(encode_program_to_fb)
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
    Ok(fb::StoredProgram {
        version: 1,
        main_vector,
        fork_vectors,
        literals,
        jump_labels,
        var_names,
        symbol_table,
        scatter_tables,
        for_sequence_operands,
        for_range_operands,
        range_comprehensions,
        list_comprehensions,
        error_operands,
        lambda_programs,
        line_number_spans,
        fork_line_number_spans,
    })
}

/// Convert a Program to a StoredProgram (FlatBuffer format)
pub fn program_to_stored(program: &Program) -> Result<StoredProgram, EncodeError> {
    let mut builder = planus::Builder::new();

    let stored_program = encode_program_to_fb(program)?;
    let offset = stored_program.prepare(&mut builder);
    let bytes = builder.finish(offset, None);

    Ok(StoredProgram::from_bytes(ByteView::from(bytes)))
}

/// Convert a StoredProgram to a Program (runtime format)
pub fn stored_to_program(stored: &StoredProgram) -> Result<Program, DecodeError> {
    use moor_var::program::program::PrgInner;
    use planus::ReadAsRoot;
    use std::sync::Arc;

    // 1. Read FlatBuffer
    let fb_program = fb::StoredProgramRef::read_as_root(stored.as_bytes())
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read FlatBuffer: {}", e)))?;

    // 2. Decode main_vector using OpStream
    let main_words: Vec<u16> = fb_program
        .main_vector()
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read main_vector: {}", e)))?
        .to_vec()
        .map_err(|e| {
            DecodeError::DecodeFailed(format!("Failed to convert main_vector to vec: {}", e))
        })?;
    let main_stream = OpStream::from_words(main_words);
    let mut main_vector = Vec::new();
    let mut pc = 0;
    while pc < main_stream.len() {
        let op = main_stream
            .decode_at(&mut pc)
            .map_err(|e| DecodeError::DecodeFailed(format!("Failed to decode opcode: {}", e)))?;
        main_vector.push(op);
    }

    // 3. Decode fork_vectors
    let fb_fork_vectors = fb_program
        .fork_vectors()
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read fork_vectors: {}", e)))?;
    let fork_vectors: Result<Vec<(usize, Vec<moor_var::program::opcode::Op>)>, DecodeError> =
        fb_fork_vectors
            .iter()
            .map(|fv_result| {
                let fv = fv_result.map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read fork_vector: {}", e))
                })?;
                let offset = fv.offset().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read fork offset: {}", e))
                })? as usize;
                let words = fv
                    .opcodes()
                    .map_err(|e| {
                        DecodeError::DecodeFailed(format!("Failed to read fork opcodes: {}", e))
                    })?
                    .to_vec()
                    .map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to convert fork opcodes to vec: {}",
                            e
                        ))
                    })?;
                let stream = OpStream::from_words(words);
                let mut ops = Vec::new();
                let mut pc = 0;
                while pc < stream.len() {
                    let op = stream.decode_at(&mut pc).map_err(|e| {
                        DecodeError::DecodeFailed(format!("Failed to decode fork opcode: {}", e))
                    })?;
                    ops.push(op);
                }
                Ok((offset, ops))
            })
            .collect();
    let fork_vectors = fork_vectors?;

    // 4. Decode literals
    let fb_literals = fb_program
        .literals()
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read literals: {}", e)))?;
    let literals: Result<Vec<moor_var::Var>, DecodeError> = fb_literals
        .iter()
        .map(|lit_result| {
            let lit = lit_result
                .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read literal: {}", e)))?;
            let bytes = lit.data().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read literal data: {}", e))
            })?;
            moor_var::Var::from_bytes(ByteView::from(bytes.to_vec()))
                .map_err(|e| DecodeError::DecodeFailed(format!("Failed to decode literal: {}", e)))
        })
        .collect();
    let literals = literals?;

    // 5. Decode jump labels
    let fb_jump_labels = fb_program
        .jump_labels()
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read jump_labels: {}", e)))?;
    let jump_labels: Result<Vec<moor_var::program::labels::JumpLabel>, DecodeError> =
        fb_jump_labels
            .iter()
            .map(|jl_result| {
                use moor_var::program::labels::{JumpLabel, Label, Offset};
                let jl = jl_result.map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read jump label: {}", e))
                })?;
                Ok(JumpLabel {
                    id: Label(jl.id().map_err(|e| {
                        DecodeError::DecodeFailed(format!("Failed to read jump label id: {}", e))
                    })?),
                    position: Offset(jl.position().map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read jump label position: {}",
                            e
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

    // 6. Decode variable names (full Names structure with decls HashMap)
    let fb_var_names = fb_program
        .var_names()
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read var_names: {}", e)))?;
    let var_names = decode_names_from_fb(fb_var_names, "Failed to read")?;

    // 7-11. Decode all the operand tables
    let scatter_tables = decode_scatter_tables(&fb_program)?;
    let for_sequence_operands = decode_for_sequence_operands(&fb_program)?;
    let for_range_operands = decode_for_range_operands(&fb_program)?;
    let range_comprehensions = decode_range_comprehensions(&fb_program)?;
    let list_comprehensions = decode_list_comprehensions(&fb_program)?;

    // 12. Decode error operands
    let fb_error_operands = fb_program
        .error_operands()
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read error_operands: {}", e)))?;
    let error_operands = fb_error_operands
        .iter()
        .map(|&code| {
            error_code_from_discriminant(code).ok_or(DecodeError::DecodeFailed(format!(
                "Invalid opcode in program stream: {code}"
            )))
        })
        .collect::<Result<Vec<ErrorCode>, DecodeError>>()?;

    // 13. Decode lambda programs (recursive)
    let fb_lambda_programs = fb_program
        .lambda_programs()
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read lambda_programs: {}", e)))?;
    let lambda_programs: Result<Vec<Program>, DecodeError> = fb_lambda_programs
        .iter()
        .map(|lp_result| {
            let lp = lp_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read lambda program: {}", e))
            })?;
            decode_fb_program(lp)
        })
        .collect();
    let lambda_programs = lambda_programs?;

    // 14. Decode line number spans
    let fb_line_spans = fb_program.line_number_spans().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read line_number_spans: {}", e))
    })?;
    let line_number_spans: Result<Vec<(usize, usize)>, DecodeError> = fb_line_spans
        .iter()
        .map(|ls_result| {
            let ls = ls_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read line span: {}", e))
            })?;
            Ok((
                ls.offset().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read line span offset: {}", e))
                })? as usize,
                ls.line_number().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read line number: {}", e))
                })? as usize,
            ))
        })
        .collect();
    let line_number_spans = line_number_spans?;

    // 15. Decode fork line number spans
    let fb_fork_line_spans = fb_program.fork_line_number_spans().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read fork_line_number_spans: {}", e))
    })?;
    let fork_line_number_spans: Result<Vec<Vec<(usize, usize)>>, DecodeError> = fb_fork_line_spans
        .iter()
        .map(|fls_result| {
            let fls = fls_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read fork line span: {}", e))
            })?;
            let spans = fls.spans().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read fork line spans: {}", e))
            })?;
            spans
                .iter()
                .map(|ls_result2| {
                    let ls = ls_result2.map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read fork line span element: {}",
                            e
                        ))
                    })?;
                    Ok((
                        ls.offset().map_err(|e| {
                            DecodeError::DecodeFailed(format!(
                                "Failed to read fork line span offset: {}",
                                e
                            ))
                        })? as usize,
                        ls.line_number().map_err(|e| {
                            DecodeError::DecodeFailed(format!(
                                "Failed to read fork line number: {}",
                                e
                            ))
                        })? as usize,
                    ))
                })
                .collect()
        })
        .collect();
    let fork_line_number_spans = fork_line_number_spans?;

    // Build Program
    Ok(Program(Arc::new(PrgInner {
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
    })))
}

// Helper functions for decoding

fn decode_stored_name(name_ref: fb::StoredNameRef) -> Result<Name, DecodeError> {
    Ok(Name(
        name_ref
            .offset()
            .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read name offset: {}", e)))?,
        name_ref
            .scope_depth()
            .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read scope_depth: {}", e)))?,
        name_ref
            .scope_id()
            .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read scope_id: {}", e)))?,
    ))
}

fn decode_names_from_fb(
    fb_var_names: fb::StoredNamesRef,
    error_prefix: &str,
) -> Result<moor_var::program::names::Names, DecodeError> {
    let global_width = fb_var_names
        .global_width()
        .map_err(|e| DecodeError::DecodeFailed(format!("{} global_width: {}", error_prefix, e)))?
        as usize;

    let decls_vec = fb_var_names
        .decls()
        .map_err(|e| DecodeError::DecodeFailed(format!("{} decls: {}", error_prefix, e)))?;

    let mut decls = std::collections::HashMap::new();
    for pair_result in decls_vec {
        let pair = pair_result.map_err(|e| {
            DecodeError::DecodeFailed(format!("{} name-decl pair: {}", error_prefix, e))
        })?;

        let name_ref = pair
            .name()
            .map_err(|e| DecodeError::DecodeFailed(format!("{} pair name: {}", error_prefix, e)))?;
        let name = decode_stored_name(name_ref)?;

        let decl_ref = pair
            .decl()
            .map_err(|e| DecodeError::DecodeFailed(format!("{} pair decl: {}", error_prefix, e)))?;

        let decl_type = match decl_ref
            .decl_type()
            .map_err(|e| DecodeError::DecodeFailed(format!("{} decl_type: {}", error_prefix, e)))?
        {
            fb::StoredDeclType::Global => moor_var::program::DeclType::Global,
            fb::StoredDeclType::Let => moor_var::program::DeclType::Let,
            fb::StoredDeclType::Assign => moor_var::program::DeclType::Assign,
            fb::StoredDeclType::For => moor_var::program::DeclType::For,
            fb::StoredDeclType::Unknown => moor_var::program::DeclType::Unknown,
            fb::StoredDeclType::Register => moor_var::program::DeclType::Register,
            fb::StoredDeclType::Except => moor_var::program::DeclType::Except,
            fb::StoredDeclType::WhileLabel => moor_var::program::DeclType::WhileLabel,
            fb::StoredDeclType::ForkLabel => moor_var::program::DeclType::ForkLabel,
        };

        let identifier_ref = decl_ref.identifier().map_err(|e| {
            DecodeError::DecodeFailed(format!("{} identifier: {}", error_prefix, e))
        })?;
        let var_name_union = identifier_ref
            .var_name()
            .map_err(|e| DecodeError::DecodeFailed(format!("{} var_name: {}", error_prefix, e)))?;

        use moor_var::program::names::VarName;
        let var_name = match var_name_union {
            fb::StoredVarNameUnionRef::StoredNamedVar(named) => {
                let symbol_ref = named.symbol().map_err(|e| {
                    DecodeError::DecodeFailed(format!("{} symbol: {}", error_prefix, e))
                })?;
                let symbol_str = symbol_ref.value().map_err(|e| {
                    DecodeError::DecodeFailed(format!("{} symbol value: {}", error_prefix, e))
                })?;
                VarName::Named(moor_var::Symbol::mk(symbol_str))
            }
            fb::StoredVarNameUnionRef::StoredRegisterVar(reg) => {
                let reg_num = reg.register_num().map_err(|e| {
                    DecodeError::DecodeFailed(format!("{} register_num: {}", error_prefix, e))
                })?;
                VarName::Register(reg_num)
            }
        };

        let identifier = moor_var::program::names::Variable {
            id: identifier_ref.id().map_err(|e| {
                DecodeError::DecodeFailed(format!("{} variable id: {}", error_prefix, e))
            })?,
            scope_id: identifier_ref.scope_id().map_err(|e| {
                DecodeError::DecodeFailed(format!("{} variable scope_id: {}", error_prefix, e))
            })?,
            nr: var_name,
        };

        let decl = moor_var::program::Decl {
            decl_type,
            identifier,
            depth: decl_ref
                .depth()
                .map_err(|e| DecodeError::DecodeFailed(format!("{} depth: {}", error_prefix, e)))?
                as usize,
            constant: decl_ref.constant().map_err(|e| {
                DecodeError::DecodeFailed(format!("{} constant: {}", error_prefix, e))
            })?,
            scope_id: decl_ref.scope_id().map_err(|e| {
                DecodeError::DecodeFailed(format!("{} decl scope_id: {}", error_prefix, e))
            })?,
        };

        decls.insert(name, decl);
    }

    Ok(moor_var::program::names::Names {
        global_width,
        decls,
    })
}

fn decode_fb_program(fb_prog_ref: fb::StoredProgramRef) -> Result<Program, DecodeError> {
    // This is essentially the same as stored_to_program but takes a Ref
    // For simplicity, we can convert to owned and then encode/decode
    // Or we can duplicate the logic - let's duplicate for now
    use moor_var::program::program::PrgInner;
    use std::sync::Arc;

    // Decode main_vector
    let main_words = fb_prog_ref
        .main_vector()
        .map_err(|e| {
            DecodeError::DecodeFailed(format!("Failed to read lambda main_vector: {}", e))
        })?
        .to_vec()
        .map_err(|e| {
            DecodeError::DecodeFailed(format!(
                "Failed to convert lambda main_vector to vec: {}",
                e
            ))
        })?;
    let main_stream = OpStream::from_words(main_words);
    let mut main_vector = Vec::new();
    let mut pc = 0;
    while pc < main_stream.len() {
        let op = main_stream.decode_at(&mut pc).map_err(|e| {
            DecodeError::DecodeFailed(format!("Failed to decode lambda opcode: {}", e))
        })?;
        main_vector.push(op);
    }

    // Decode fork_vectors
    let fb_fork_vectors = fb_prog_ref.fork_vectors().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read lambda fork_vectors: {}", e))
    })?;
    let fork_vectors: Result<Vec<(usize, Vec<moor_var::program::opcode::Op>)>, DecodeError> =
        fb_fork_vectors
            .iter()
            .map(|fv_result| {
                let fv = fv_result.map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read lambda fork_vector: {}", e))
                })?;
                let offset = fv.offset().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read lambda fork offset: {}", e))
                })? as usize;
                let words = fv
                    .opcodes()
                    .map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read lambda fork opcodes: {}",
                            e
                        ))
                    })?
                    .to_vec()
                    .map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to convert lambda fork opcodes to vec: {}",
                            e
                        ))
                    })?;
                let stream = OpStream::from_words(words);
                let mut ops = Vec::new();
                let mut pc = 0;
                while pc < stream.len() {
                    let op = stream.decode_at(&mut pc).map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to decode lambda fork opcode: {}",
                            e
                        ))
                    })?;
                    ops.push(op);
                }
                Ok((offset, ops))
            })
            .collect();
    let fork_vectors = fork_vectors?;

    // Decode literals
    let fb_literals = fb_prog_ref
        .literals()
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read lambda literals: {}", e)))?;
    let literals: Result<Vec<moor_var::Var>, DecodeError> = fb_literals
        .iter()
        .map(|lit_result| {
            let lit = lit_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read lambda literal: {}", e))
            })?;
            let bytes = lit.data().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read lambda literal data: {}", e))
            })?;
            moor_var::Var::from_bytes(ByteView::from(bytes.to_vec())).map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to decode lambda literal: {}", e))
            })
        })
        .collect();
    let literals = literals?;

    // Decode jump labels
    let fb_jump_labels = fb_prog_ref.jump_labels().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read lambda jump_labels: {}", e))
    })?;
    let jump_labels: Result<Vec<moor_var::program::labels::JumpLabel>, DecodeError> =
        fb_jump_labels
            .iter()
            .map(|jl_result| {
                let jl = jl_result.map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read lambda jump label: {}", e))
                })?;
                use moor_var::program::labels::{JumpLabel, Label, Offset};
                Ok(JumpLabel {
                    id: Label(jl.id().map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read lambda jump label id: {}",
                            e
                        ))
                    })?),
                    position: Offset(jl.position().map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read lambda jump label position: {}",
                            e
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
    let fb_var_names = fb_prog_ref.var_names().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read lambda var_names: {}", e))
    })?;
    let var_names = decode_names_from_fb(fb_var_names, "Failed to read lambda")?;

    // Decode operand tables
    let scatter_tables = decode_scatter_tables(&fb_prog_ref)?;
    let for_sequence_operands = decode_for_sequence_operands(&fb_prog_ref)?;
    let for_range_operands = decode_for_range_operands(&fb_prog_ref)?;
    let range_comprehensions = decode_range_comprehensions(&fb_prog_ref)?;
    let list_comprehensions = decode_list_comprehensions(&fb_prog_ref)?;

    // Decode error operands
    let fb_error_operands = fb_prog_ref.error_operands().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read lambda error_operands: {}", e))
    })?;
    let error_operands = fb_error_operands
        .iter()
        .map(|&code| {
            error_code_from_discriminant(code).ok_or(DecodeError::DecodeFailed(format!(
                "Invalid opcode in program stream: {code}"
            )))
        })
        .collect::<Result<Vec<_>, DecodeError>>()?;

    // Decode lambda programs (recursive)
    let fb_lambda_programs = fb_prog_ref.lambda_programs().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read nested lambda_programs: {}", e))
    })?;
    let lambda_programs: Result<Vec<Program>, DecodeError> = fb_lambda_programs
        .iter()
        .map(|lp_result| {
            let lp = lp_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read nested lambda program: {}", e))
            })?;
            decode_fb_program(lp)
        })
        .collect();
    let lambda_programs = lambda_programs?;

    // Decode line number spans
    let fb_line_spans = fb_prog_ref.line_number_spans().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read lambda line_number_spans: {}", e))
    })?;
    let line_number_spans: Result<Vec<(usize, usize)>, DecodeError> = fb_line_spans
        .iter()
        .map(|ls_result| {
            let ls = ls_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read lambda line span: {}", e))
            })?;
            Ok((
                ls.offset().map_err(|e| {
                    DecodeError::DecodeFailed(format!(
                        "Failed to read lambda line span offset: {}",
                        e
                    ))
                })? as usize,
                ls.line_number().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read lambda line number: {}", e))
                })? as usize,
            ))
        })
        .collect();
    let line_number_spans = line_number_spans?;

    // Decode fork line number spans
    let fb_fork_line_spans = fb_prog_ref.fork_line_number_spans().map_err(|e| {
        DecodeError::DecodeFailed(format!(
            "Failed to read lambda fork_line_number_spans: {}",
            e
        ))
    })?;
    let fork_line_number_spans: Result<Vec<Vec<(usize, usize)>>, DecodeError> = fb_fork_line_spans
        .iter()
        .map(|fls_result| {
            let fls = fls_result.map_err(|e| {
                DecodeError::DecodeFailed(format!(
                    "Failed to read lambda fork line span group: {}",
                    e
                ))
            })?;
            let spans = fls.spans().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read lambda fork line spans: {}", e))
            })?;
            spans
                .iter()
                .map(|ls_result| {
                    let ls = ls_result.map_err(|e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read lambda fork line span: {}",
                            e
                        ))
                    })?;
                    Ok((
                        ls.offset().map_err(|e| {
                            DecodeError::DecodeFailed(format!(
                                "Failed to read lambda fork line span offset: {}",
                                e
                            ))
                        })? as usize,
                        ls.line_number().map_err(|e| {
                            DecodeError::DecodeFailed(format!(
                                "Failed to read lambda fork line number: {}",
                                e
                            ))
                        })? as usize,
                    ))
                })
                .collect()
        })
        .collect();
    let fork_line_number_spans = fork_line_number_spans?;

    Ok(Program(Arc::new(PrgInner {
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
    })))
}

fn decode_one_scatter_label(
    sl_result: Result<fb::StoredScatterLabelRef, planus::Error>,
) -> Result<ScatterLabel, DecodeError> {
    let sl = sl_result
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read scatter label: {}", e)))?;
    let fb_label = sl.label().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read scatter label union: {}", e))
    })?;

    match fb_label {
        fb::StoredScatterLabelUnionRef::StoredScatterRequired(req) => {
            let name_ref = req.name().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read required name: {}", e))
            })?;
            let name = decode_stored_name(name_ref)?;
            Ok(ScatterLabel::Required(name))
        }
        fb::StoredScatterLabelUnionRef::StoredScatterOptional(opt) => {
            let name_ref = opt.name().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read optional name: {}", e))
            })?;
            let name = decode_stored_name(name_ref)?;

            let has_default = opt.has_default().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read has_default: {}", e))
            })?;

            let default_label = if has_default {
                let label_val = opt.default_label().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read default_label: {}", e))
                })?;
                Some(Label(label_val))
            } else {
                None
            };

            Ok(ScatterLabel::Optional(name, default_label))
        }
        fb::StoredScatterLabelUnionRef::StoredScatterRest(rest) => {
            let name_ref = rest.name().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read rest name: {}", e))
            })?;
            let name = decode_stored_name(name_ref)?;
            Ok(ScatterLabel::Rest(name))
        }
    }
}

fn decode_scatter_tables(
    fb_program: &fb::StoredProgramRef,
) -> Result<Vec<moor_var::program::opcode::ScatterArgs>, DecodeError> {
    let fb_scatter_tables = fb_program
        .scatter_tables()
        .map_err(|e| DecodeError::DecodeFailed(format!("Failed to read scatter_tables: {}", e)))?;

    fb_scatter_tables
        .iter()
        .map(|st_result| {
            let st = st_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read scatter table: {}", e))
            })?;
            let fb_labels = st.labels().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read scatter labels: {}", e))
            })?;

            let labels: Result<Vec<ScatterLabel>, DecodeError> =
                fb_labels.iter().map(decode_one_scatter_label).collect();

            let done_val = st.done().map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read scatter done: {}", e))
            })?;

            Ok(ScatterArgs {
                labels: labels?,
                done: Label(done_val),
            })
        })
        .collect()
}

fn decode_for_sequence_operands(
    fb_program: &fb::StoredProgramRef,
) -> Result<Vec<moor_var::program::opcode::ForSequenceOperand>, DecodeError> {
    use moor_var::program::{labels::Label, opcode::ForSequenceOperand};

    let fb_operands = fb_program.for_sequence_operands().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read for_sequence_operands: {}", e))
    })?;

    fb_operands
        .iter()
        .map(|fso_result| {
            let fso = fso_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read for_sequence_operand: {}", e))
            })?;
            Ok(ForSequenceOperand {
                value_bind: decode_stored_name(fso.value_bind().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read value_bind: {}", e))
                })?)?,
                key_bind: fso
                    .key_bind()
                    .ok()
                    .flatten()
                    .map(|n| decode_stored_name(n))
                    .transpose()?,
                end_label: Label(fso.end_label().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read end_label: {}", e))
                })?),
                environment_width: fso.environment_width().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read environment_width: {}", e))
                })?,
            })
        })
        .collect()
}

fn decode_for_range_operands(
    fb_program: &fb::StoredProgramRef,
) -> Result<Vec<moor_var::program::opcode::ForRangeOperand>, DecodeError> {
    use moor_var::program::{labels::Label, opcode::ForRangeOperand};

    let fb_operands = fb_program.for_range_operands().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read for_range_operands: {}", e))
    })?;

    fb_operands
        .iter()
        .map(|fro_result| {
            let fro = fro_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read for_range_operand: {}", e))
            })?;
            Ok(ForRangeOperand {
                loop_variable: decode_stored_name(fro.loop_variable().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read loop_variable: {}", e))
                })?)?,
                end_label: Label(fro.end_label().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read end_label: {}", e))
                })?),
                environment_width: fro.environment_width().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read environment_width: {}", e))
                })?,
            })
        })
        .collect()
}

fn decode_range_comprehensions(
    fb_program: &fb::StoredProgramRef,
) -> Result<Vec<moor_var::program::opcode::RangeComprehend>, DecodeError> {
    use moor_var::program::{labels::Label, opcode::RangeComprehend};

    let fb_comprehensions = fb_program.range_comprehensions().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read range_comprehensions: {}", e))
    })?;

    fb_comprehensions
        .iter()
        .map(|rc_result| {
            let rc = rc_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read range_comprehension: {}", e))
            })?;
            Ok(RangeComprehend {
                position: decode_stored_name(rc.position().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read position: {}", e))
                })?)?,
                end_of_range_register: decode_stored_name(rc.end_of_range_register().map_err(
                    |e| {
                        DecodeError::DecodeFailed(format!(
                            "Failed to read end_of_range_register: {}",
                            e
                        ))
                    },
                )?)?,
                end_label: Label(rc.end_label().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read end_label: {}", e))
                })?),
            })
        })
        .collect()
}

fn decode_list_comprehensions(
    fb_program: &fb::StoredProgramRef,
) -> Result<Vec<moor_var::program::opcode::ListComprehend>, DecodeError> {
    use moor_var::program::{labels::Label, opcode::ListComprehend};

    let fb_comprehensions = fb_program.list_comprehensions().map_err(|e| {
        DecodeError::DecodeFailed(format!("Failed to read list_comprehensions: {}", e))
    })?;

    fb_comprehensions
        .iter()
        .map(|lc_result| {
            let lc = lc_result.map_err(|e| {
                DecodeError::DecodeFailed(format!("Failed to read list_comprehension: {}", e))
            })?;
            Ok(ListComprehend {
                position_register: decode_stored_name(lc.position_register().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read position_register: {}", e))
                })?)?,
                list_register: decode_stored_name(lc.list_register().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read list_register: {}", e))
                })?)?,
                item_variable: decode_stored_name(lc.item_variable().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read item_variable: {}", e))
                })?)?,
                end_label: Label(lc.end_label().map_err(|e| {
                    DecodeError::DecodeFailed(format!("Failed to read end_label: {}", e))
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
