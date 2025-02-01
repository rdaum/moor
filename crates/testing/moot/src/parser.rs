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

use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "moot.pest"]
struct MootParser;

#[derive(Debug, PartialEq)]
pub struct MootBlockSpan<'a> {
    pub line_no: usize,
    pub expr: MootBlock<'a>,
}

#[derive(Debug, PartialEq)]
pub enum MootBlock<'a> {
    ChangePlayer(MootBlockChangePlayer<'a>),
    Test(MootBlockTest<'a>),
}

#[derive(Debug, PartialEq)]
pub struct MootBlockChangePlayer<'a> {
    pub name: &'a str,
}

#[derive(Debug, PartialEq)]
pub struct MootBlockTest<'a> {
    pub kind: MootBlockTestKind,
    pub prog_lines: Vec<&'a str>,
    pub expected_output: Vec<MootBlockTestExpectedOutput<'a>>,
}
impl MootBlockTest<'_> {
    pub fn prog(&self) -> String {
        self.prog_lines.join("\n")
    }
}

#[derive(Debug, PartialEq)]
pub struct MootBlockTestExpectedOutput<'a> {
    pub expected_output: &'a str,
    pub verbatim: bool,
    pub line_no: usize,
}

#[derive(Debug, PartialEq)]
pub enum MootBlockTestKind {
    Eval,
    Command,
    EvalBg,
}

pub fn parse(input: &str) -> eyre::Result<Vec<MootBlockSpan>> {
    let mut expressions = vec![];
    for pair in MootParser::parse(Rule::file, input)? {
        let line_no = pair.as_span().start_pos().line_col().0;

        let content = match pair.as_rule() {
            Rule::block => None,
            Rule::change_player_name => Some(MootBlock::ChangePlayer(MootBlockChangePlayer {
                name: pair.as_str(),
            })),

            Rule::eval | Rule::cmd | Rule::eval_bg => {
                let kind = match pair.as_rule() {
                    Rule::eval => MootBlockTestKind::Eval,
                    Rule::cmd => MootBlockTestKind::Command,
                    Rule::eval_bg => MootBlockTestKind::EvalBg,
                    r => unreachable!("{:?}", r),
                };
                let mut test = MootBlockTest {
                    kind,
                    prog_lines: vec![],
                    expected_output: vec![],
                };
                for pair in pair.into_inner() {
                    match pair.as_rule() {
                        Rule::test_line => {
                            test.prog_lines.push(pair.as_str());
                        }
                        Rule::expect_eval_line | Rule::expect_verbatim_line => {
                            if test.kind == MootBlockTestKind::EvalBg {
                                return Err(eyre::eyre!(
                                    "background eval blocks cannot have expected output"
                                ));
                            }
                            test.expected_output.push(MootBlockTestExpectedOutput {
                                expected_output: pair.as_str(),
                                verbatim: pair.as_rule() == Rule::expect_verbatim_line,
                                line_no: pair.as_span().start_pos().line_col().0,
                            });
                        }
                        Rule::EOI => {}
                        r => unreachable!("{:?}", r),
                    }
                }
                Some(MootBlock::Test(test))
            }

            Rule::EOI => None,
            r => unreachable!("{:?}", r),
        };

        if let Some(expr) = content {
            expressions.push(MootBlockSpan { line_no, expr });
        }
    }

    Ok(expressions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_player() {
        assert_eq!(
            parse("  \n//comment\n\n@wizard\n\n//comment\n@programmer").unwrap(),
            vec![
                MootBlockSpan {
                    line_no: 4,
                    expr: MootBlock::ChangePlayer(MootBlockChangePlayer { name: "wizard" })
                },
                MootBlockSpan {
                    line_no: 7,
                    expr: MootBlock::ChangePlayer(MootBlockChangePlayer { name: "programmer" })
                }
            ]
        );
    }

    #[test]
    fn test_eval() {
        assert_eq!(
            parse("; 1 + \n> 2;\n//comment\n=3\n\n").unwrap(),
            vec![MootBlockSpan {
                line_no: 1,
                expr: MootBlock::Test(MootBlockTest {
                    kind: MootBlockTestKind::Eval,
                    prog_lines: vec!["1 + ", " 2;"],
                    expected_output: vec![MootBlockTestExpectedOutput {
                        expected_output: "3",
                        verbatim: true,
                        line_no: 4,
                    }],
                })
            }]
        );
    }

    #[test]
    fn test_eval_multiple_statements() {
        assert_eq!(
            parse("; 1; 2; 3;\n; 4;\n4").unwrap(),
            vec![
                MootBlockSpan {
                    line_no: 1,
                    expr: MootBlock::Test(MootBlockTest {
                        kind: MootBlockTestKind::Eval,
                        prog_lines: vec!["1; 2; 3;"],
                        expected_output: vec![]
                    })
                },
                MootBlockSpan {
                    line_no: 2,
                    expr: MootBlock::Test(MootBlockTest {
                        kind: MootBlockTestKind::Eval,
                        prog_lines: vec!["4;"],
                        expected_output: vec![MootBlockTestExpectedOutput {
                            expected_output: "4",
                            verbatim: false,
                            line_no: 3,
                        }],
                    })
                }
            ]
        )
    }
}
