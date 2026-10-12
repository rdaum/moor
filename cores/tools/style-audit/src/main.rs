// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com>
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.

//! Report source-level style debt using the objdef compiler and MOO concrete syntax tree.

use moor_compiler::{
    CompileOptions, SyntaxKind, lex, parse_program_frontend, parse_to_syntax_node,
};
use moor_objdef::{ObjDefSet, ObjDefSource};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

/// Discover nested objdef sources without following directory symlinks.
fn collect_sources(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_sources(&entry.path(), files)?;
        } else if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "moo") {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn read_baseline(source: &str) -> Result<BTreeMap<String, usize>, Box<dyn Error>> {
    let mut lines = source.lines();
    if lines.next() != Some("count\tfile\tverb\tkind\tdetail") {
        return Err("invalid style baseline header".into());
    }
    let mut debt = BTreeMap::new();
    for line in lines {
        let (count, key) = line.split_once('\t').ok_or("invalid baseline record")?;
        if key.split('\t').count() != 4 || debt.insert(key.to_owned(), count.parse()?).is_some() {
            return Err("invalid or duplicate baseline record".into());
        }
    }
    Ok(debt)
}

fn run() -> Result<bool, Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let directory = PathBuf::from(args.next().ok_or(
        "usage: core-style-audit SRC_DIR [--strict] [--check RELATIVE_PATH] [--baseline FILE] [--write-baseline FILE]",
    )?);
    let mut strict = false;
    let mut selected = Vec::new();
    let mut baseline = None;
    let mut write_baseline = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--strict" => strict = true,
            "--check" => selected.push(PathBuf::from(args.next().ok_or("--check needs a path")?)),
            "--baseline" => {
                baseline = Some(PathBuf::from(args.next().ok_or("--baseline needs a path")?))
            }
            "--write-baseline" => {
                write_baseline = Some(PathBuf::from(
                    args.next().ok_or("--write-baseline needs a path")?,
                ))
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    if baseline.is_some() && write_baseline.is_some() {
        return Err("cannot check and write a baseline together".into());
    }
    let mut files = Vec::new();
    collect_sources(&directory, &mut files)?;
    files.sort();
    for selection in &selected {
        if !files.iter().any(|path| {
            path.strip_prefix(&directory).is_ok_and(|relative| {
                relative.starts_with(
                    selection
                        .to_string_lossy()
                        .split_once(':')
                        .map_or(selection.as_path(), |(path, _)| Path::new(path)),
                )
            })
        }) {
            return Err(format!("selection has no sources: {}", selection.display()).into());
        }
    }
    if let Some(index) = files
        .iter()
        .position(|path| path.file_name().is_some_and(|name| name == "constants.moo"))
    {
        let constants = files.remove(index);
        files.insert(0, constants);
    }
    let mut sources = Vec::new();
    for path in &files {
        sources.push(ObjDefSource {
            label: path
                .strip_prefix(&directory)?
                .to_string_lossy()
                .into_owned(),
            contents: fs::read_to_string(path)?,
            path: Some(path.clone()),
        });
    }
    let options = CompileOptions {
        call_unsupported_builtins: true,
        ..Default::default()
    };
    let definitions = ObjDefSet::parse_sources(&options, Some(&directory), None, sources)
        .map_err(|error| format!("objdef compile failed: {error}"))?;
    let expected_verbs: usize = definitions
        .graph()
        .object_definitions()
        .values()
        .map(|(_, object)| object.verbs.len())
        .sum();
    let mut verbs = 0;
    let mut findings = BTreeMap::new();
    let mut declarations = 0;
    let mut debt = BTreeMap::<String, usize>::new();
    let mut checked_verbs = 0;
    let mut matched_selections = vec![false; selected.len()];
    println!("file\tline\tverb\tkind\tdetail");
    for path in files {
        let relative = path.strip_prefix(&directory)?;

        let source = fs::read_to_string(&path)?;
        let mut body = None;
        // The lexer locates declaration boundaries; both the full objdef and each body
        // are checked by the compiler. Core declarations use the guide's two-space indent.
        let mut tokens = lex(&source).into_iter();
        while let Some(token) = tokens.next() {
            if token.kind != SyntaxKind::Ident {
                continue;
            }
            let word = &source[token.span.clone()];
            let line = source[..token.span.start]
                .rfind('\n')
                .map_or(0, |offset| offset + 1);
            if &source[line..token.span.start] != "  " {
                continue;
            }
            if matches!(word, "verb" | "method") {
                if body.is_some() {
                    return Err(format!("nested declaration in {}", path.display()).into());
                }
                let mut metadata_depth = 0;
                let mut metadata_start = None;
                let start = loop {
                    let next = tokens.next().ok_or("unterminated declaration")?;
                    match next.kind {
                        SyntaxKind::LBracket => {
                            metadata_start.get_or_insert(next.span.start);
                            metadata_depth += 1;
                        }
                        SyntaxKind::RBracket => metadata_depth -= 1,
                        SyntaxKind::Newline if metadata_depth == 0 => break next.span.end,
                        _ => {}
                    }
                };
                // Metadata belongs to the declaration, not the executable body or debt key.
                let header = source[token.span.end..metadata_start.unwrap_or(start)]
                    .trim()
                    .to_owned();
                body = Some((
                    start,
                    header,
                    if word == "verb" {
                        "endverb"
                    } else {
                        "endmethod"
                    },
                ));
                continue;
            }
            let Some((start, header, endword)) = body.as_ref() else {
                continue;
            };
            if word != *endword {
                continue;
            }
            let (start, header) = (*start, header.clone());
            let code = &source[start..line];
            let (root, errors) = parse_to_syntax_node(code);
            if !errors.is_empty() {
                return Err(format!("{}: {errors:?}", path.display()).into());
            }
            verbs += 1;
            let mut checked = selected.is_empty();
            for (index, selection) in selected.iter().enumerate() {
                let text = selection.to_string_lossy();
                let matches = match text.split_once(':') {
                    Some((file, verb)) => {
                        relative == Path::new(file)
                            && header
                                .split_whitespace()
                                .next()
                                .is_some_and(|name| name.trim_matches('"') == verb)
                    }
                    None => relative.starts_with(selection),
                };
                matched_selections[index] |= matches;
                checked |= matches;
            }
            checked_verbs += usize::from(checked);
            let mut report = |offset: usize, kind: &str, detail: String| {
                let line_number = source[..start + offset]
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                    + 1;
                if !checked {
                    return;
                }
                let detail = detail.replace(['\n', '\t'], " ");
                *debt
                    .entry(format!(
                        "{}\t{header}\t{kind}\t{detail}",
                        relative.display()
                    ))
                    .or_default() += 1;
                *findings.entry(kind.to_owned()).or_insert(0_usize) += 1;
                println!(
                    "{}\t{line_number}\t{header}\t{kind}\t{}",
                    path.display(),
                    detail
                );
            };
            let statements = root
                .children()
                .find(|node| node.kind() == SyntaxKind::StmtList);
            if let Some(first) = statements.and_then(|node| node.children().next()) {
                let first_token = first
                    .descendants_with_tokens()
                    .filter_map(|element| element.into_token())
                    .find(|token| {
                        !matches!(token.kind(), SyntaxKind::Whitespace | SyntaxKind::Newline)
                    });
                if !first_token.is_some_and(|token| token.kind() == SyntaxKind::StringLit) {
                    report(
                        0,
                        "missing-docstring",
                        "Describe the contract before executable statements.".to_owned(),
                    );
                }
            }
            let explicit_scatter_names: std::collections::BTreeSet<String> = root
                .descendants()
                .filter(|node| node.kind() == SyntaxKind::ScatterItem)
                .filter(|node| {
                    node.ancestors().any(|ancestor| {
                        matches!(
                            ancestor.kind(),
                            SyntaxKind::LetStmt
                                | SyntaxKind::ConstStmt
                                | SyntaxKind::LambdaExpr
                                | SyntaxKind::FnStmt
                        )
                    })
                })
                .filter_map(|node| {
                    node.children_with_tokens()
                        .filter_map(|element| element.into_token())
                        .find(|token| token.kind() == SyntaxKind::Ident)
                })
                .map(|token| token.text().to_string())
                .collect();
            let parsed = parse_program_frontend(code, options.clone())
                .map_err(|error| format!("{}: {error:?}", path.display()))?;
            for declaration in &parsed.variables.variables {
                if matches!(
                    declaration.decl_type,
                    moor_var::program::DeclType::Assign | moor_var::program::DeclType::Unknown
                ) && let moor_var::program::names::VarName::Named(name) =
                    declaration.identifier.nr
                    && !explicit_scatter_names.contains(&name.to_string())
                {
                    report(0, "implicit-local", name.to_string());
                }
            }
            for node in root.descendants() {
                if matches!(node.kind(), SyntaxKind::ConstStmt | SyntaxKind::LetStmt) {
                    declarations += 1;
                }
                if node.kind() != SyntaxKind::AssignExpr {
                    continue;
                }
                for ancestor in node.ancestors().skip(1) {
                    if ancestor.kind() == SyntaxKind::StmtList {
                        break;
                    }
                    if matches!(
                        ancestor.kind(),
                        SyntaxKind::IfStmt | SyntaxKind::ElseIfClause | SyntaxKind::WhileStmt
                    ) {
                        report(
                            u32::from(node.text_range().start()) as usize,
                            "conditional-assignment",
                            node.text().to_string(),
                        );
                        break;
                    }
                }
            }
            body = None;
        }
        if body.is_some() {
            return Err(format!("unterminated verb in {}", path.display()).into());
        }
    }
    if verbs != expected_verbs {
        return Err(
            format!("body coverage mismatch: {verbs} examined, {expected_verbs} compiled").into(),
        );
    }
    eprintln!("{verbs} verb bodies; {declarations} explicit declarations; findings: {findings:?}");
    for (selection, matched) in selected.iter().zip(matched_selections) {
        if !matched {
            return Err(format!("selection has no verb bodies: {}", selection.display()).into());
        }
    }
    if checked_verbs == 0 {
        return Err("no selected verb bodies examined".into());
    }
    let mut baseline_passes = true;
    if let Some(path) = baseline {
        let expected = read_baseline(&fs::read_to_string(path)?)?;
        for (key, count) in &debt {
            let allowed = expected.get(key).copied().unwrap_or_default();
            if *count > allowed {
                eprintln!("style debt increased: {key} ({count}, baseline {allowed})");
                baseline_passes = false;
            }
        }
    }
    if let Some(path) = write_baseline {
        let mut output = String::from("count\tfile\tverb\tkind\tdetail\n");
        for (key, count) in &debt {
            output.push_str(&format!("{count}\t{key}\n"));
        }
        fs::write(path, output)?;
    }
    Ok(baseline_passes && (!strict || findings.is_empty()))
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(true) => std::process::ExitCode::SUCCESS,
        Ok(false) => std::process::ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
