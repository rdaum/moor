// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::{CompileOptions, objdef_literal};
use moor_common::{
    model::{CompileContext, CompileError, ObjFlag, PropPerms, VerbArgsSpec, VerbFlag},
    util::BitEnum,
};
use moor_var::{Obj, Symbol, Var, VarType, program::ProgramType};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ObjFileContext {
    constants: HashMap<Symbol, Var>,
    base_path: Option<PathBuf>,
    /// Root directory used as the security boundary for `include!` / `include_bin!`.
    /// Paths that escape this directory are rejected. Falls back to `base_path` if unset.
    root_path: Option<PathBuf>,
}

impl Default for ObjFileContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjFileContext {
    pub fn new() -> Self {
        Self {
            constants: HashMap::new(),
            base_path: None,
            root_path: None,
        }
    }

    pub fn add_constant(&mut self, name: Symbol, value: Var) {
        self.constants.insert(name, value);
    }

    pub fn constants(&self) -> &HashMap<Symbol, Var> {
        &self.constants
    }

    /// Set the base directory for resolving `include!` / `include_bin!` paths.
    /// Typically the parent directory of the `.moo` source file being compiled.
    pub fn set_base_path(&mut self, source_file: &Path) {
        self.base_path = source_file.parent().map(|p| p.to_path_buf());
    }

    pub fn base_path(&self) -> Option<&Path> {
        self.base_path.as_deref()
    }

    /// Set the root directory that serves as the security boundary for includes.
    pub fn set_root_path(&mut self, root: &Path) {
        self.root_path = Some(root.to_path_buf());
    }

    /// The security boundary for include path validation.
    /// Falls back to `base_path` if no explicit root was set.
    pub fn root_path(&self) -> Option<&Path> {
        self.root_path.as_deref().or(self.base_path.as_deref())
    }
}

pub struct ObjectDefinition {
    pub oid: Obj,
    pub name: String,
    pub parent: Obj,
    pub owner: Obj,
    pub location: Obj,
    pub flags: BitEnum<ObjFlag>,

    pub verbs: Vec<ObjVerbDef>,
    pub property_definitions: Vec<ObjPropDef>,
    pub property_overrides: Vec<ObjPropOverride>,
}

pub struct ObjVerbDef {
    pub names: Vec<Symbol>,
    pub argspec: VerbArgsSpec,
    pub owner: Obj,
    pub flags: BitEnum<VerbFlag>,
    pub program: ProgramType,
}

pub struct ObjPropDef {
    pub name: Symbol,
    pub perms: PropPerms,
    pub value: Option<Var>,
}

pub struct ObjPropOverride {
    pub name: Symbol,
    pub perms_update: Option<PropPerms>,
    pub value: Option<Var>,
}

#[derive(thiserror::Error, Debug)]
pub enum ObjDefParseError {
    #[error("Failed to parse object definition: {0}")]
    ParseError(CompileError),
    #[error("Failed to compile verb: {0}")]
    VerbCompileError(CompileError, String),
    #[error("Failed to parse verb flags: {0}")]
    BadVerbFlags(String),
    #[error("Failed to parse verb argspec: {0}")]
    BadVerbArgspec(String),
    #[error("Failed to parse propflags: {0}")]
    BadPropFlags(String),
    #[error("Constant not found: {0}")]
    ConstantNotFound(String),
    #[error("Bad attribute type: {0:?}")]
    BadAttributeType(VarType),
    #[error("Invalid object ID: {0}")]
    InvalidObjectId(String),
    #[error("Include error for '{0}': {1}")]
    IncludeError(String, String),
    #[error("Duplicate constant '{0}': already defined as {1}")]
    DuplicateConstant(String, String),
}

impl ObjDefParseError {
    pub fn compile_error(&self) -> Option<(&CompileError, &str)> {
        match self {
            ObjDefParseError::VerbCompileError(error, source) => Some((error, source)),
            _ => None,
        }
    }
}

/// Offset all line numbers in a CompileError by the given amount
pub(crate) fn offset_compile_error(error: CompileError, line_offset: usize) -> CompileError {
    match error {
        CompileError::ParseError {
            error_position,
            context,
            end_line_col,
            message,
            details,
        } => {
            let (line, col) = error_position.line_col;
            CompileError::ParseError {
                error_position: CompileContext::new((line + line_offset, col)),
                context,
                end_line_col: end_line_col.map(|(l, c)| (l + line_offset, c)),
                message,
                details,
            }
        }
        CompileError::StringLexError(context, msg) => {
            let (line, col) = context.line_col;
            CompileError::StringLexError(CompileContext::new((line + line_offset, col)), msg)
        }
        CompileError::UnknownBuiltinFunction(context, name) => {
            let (line, col) = context.line_col;
            CompileError::UnknownBuiltinFunction(
                CompileContext::new((line + line_offset, col)),
                name,
            )
        }
        CompileError::UnknownTypeConstant(context, name) => {
            let (line, col) = context.line_col;
            CompileError::UnknownTypeConstant(CompileContext::new((line + line_offset, col)), name)
        }
        CompileError::UnknownLoopLabel(context, label) => {
            let (line, col) = context.line_col;
            CompileError::UnknownLoopLabel(CompileContext::new((line + line_offset, col)), label)
        }
        CompileError::DuplicateVariable(context, var) => {
            let (line, col) = context.line_col;
            CompileError::DuplicateVariable(CompileContext::new((line + line_offset, col)), var)
        }
        CompileError::AssignToConst(context, var) => {
            let (line, col) = context.line_col;
            CompileError::AssignToConst(CompileContext::new((line + line_offset, col)), var)
        }
        CompileError::DisabledFeature(context, feature) => {
            let (line, col) = context.line_col;
            CompileError::DisabledFeature(CompileContext::new((line + line_offset, col)), feature)
        }
        CompileError::BadSlotName(context, name) => {
            let (line, col) = context.line_col;
            CompileError::BadSlotName(CompileContext::new((line + line_offset, col)), name)
        }
        CompileError::InvalidAssignmentTarget(context) => {
            let (line, col) = context.line_col;
            CompileError::InvalidAssignmentTarget(CompileContext::new((line + line_offset, col)))
        }
        CompileError::InvalidTypeLiteralAssignment(type_name, context) => {
            let (line, col) = context.line_col;
            CompileError::InvalidTypeLiteralAssignment(
                type_name,
                CompileContext::new((line + line_offset, col)),
            )
        }
        CompileError::AssignmentToCapturedVariable(context, var) => {
            let (line, col) = context.line_col;
            CompileError::AssignmentToCapturedVariable(
                CompileContext::new((line + line_offset, col)),
                var,
            )
        }
    }
}
/// Parse a single MOO literal value from a string
/// Example: "123", "\"hello\"", "{1, 2, 3}", "[1 -> \"a\"]"
pub fn parse_literal_value(literal_str: &str) -> Result<Var, ObjDefParseError> {
    let mut context = ObjFileContext::new();
    objdef_literal::parse_literal_value(literal_str, &mut context)
}

pub fn compile_object_definitions(
    objdef: &str,
    options: &CompileOptions,
    context: &mut ObjFileContext,
) -> Result<Vec<ObjectDefinition>, ObjDefParseError> {
    objdef_literal::compile_object_definitions(objdef, options, context)
}
#[cfg(test)]
mod tests {
    use super::*;
    use moor_common::{
        matching::Preposition,
        model::{ArgSpec, PrepSpec, PropFlag},
    };
    use moor_var::{
        E_INVIND, List, NOTHING, v_err, v_float, v_flyweight, v_int, v_list, v_map, v_obj, v_str,
    };

    /// Just a simple objdef no verbs or props
    #[test]
    fn simple_object_def() {
        let spec = r#"
        object #1
            parent: #1
            name: "Test Object"
            location: #3
            wizard: false
            programmer: false
            player: false
            fertile: true
            readable: true
        endobject
        "#;

        let mut context = ObjFileContext::new();
        let odef =
            &compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap()[0];
        assert_eq!(odef.oid, Obj::mk_id(1));
        assert_eq!(odef.name, "Test Object");
        assert_eq!(odef.parent, Obj::mk_id(1));
        assert_eq!(odef.location, Obj::mk_id(3));
        assert!(!odef.flags.contains(ObjFlag::Wizard));
        assert!(!odef.flags.contains(ObjFlag::Programmer));
        assert!(!odef.flags.contains(ObjFlag::User));
        assert!(odef.flags.contains(ObjFlag::Fertile));
        assert!(odef.flags.contains(ObjFlag::Read));
        assert!(!odef.flags.contains(ObjFlag::Write));
    }

    // Verify that verb definitions are working
    #[test]
    fn object_with_verbdefs() {
        let spec = r#"
                object #1
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    verb "look_self look_*" (this to any) owner: #2 flags: "rxd"
                        return 5;
                    endverb

                    verb another_test (this none this) owner: #2 flags: "r"
                        player:tell("here is something");
                    endverb
                endobject"#;
        let mut context = ObjFileContext::new();
        let odef =
            &compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap()[0];

        assert_eq!(odef.verbs.len(), 2);

        assert_eq!(odef.verbs[0].names.len(), 2);
        assert_eq!(odef.verbs[0].names[0], Symbol::mk("look_self"));
        assert_eq!(odef.verbs[0].names[1], Symbol::mk("look_*"));
        assert_eq!(odef.verbs[0].argspec.dobj, ArgSpec::This);
        assert_eq!(
            odef.verbs[0].argspec.prep,
            PrepSpec::Other(Preposition::AtTo)
        );
        assert_eq!(odef.verbs[0].argspec.iobj, ArgSpec::Any);
        assert_eq!(odef.verbs[0].owner, Obj::mk_id(2));
        assert_eq!(odef.verbs[0].flags, VerbFlag::rxd());

        assert_eq!(odef.verbs[1].names.len(), 1);
        assert_eq!(odef.verbs[1].names[0], Symbol::mk("another_test"));
        assert_eq!(odef.verbs[1].argspec.dobj, ArgSpec::This);
        assert_eq!(odef.verbs[1].argspec.prep, PrepSpec::None);
        assert_eq!(odef.verbs[1].argspec.iobj, ArgSpec::This);
        assert_eq!(odef.verbs[1].owner, Obj::mk_id(2));
        assert_eq!(odef.verbs[1].flags, VerbFlag::r());
    }

    // Verify property definition / setting
    #[test]
    fn object_with_property_defs() {
        let spec = r#"
                object #1
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    // Declares a property named "description" with owner #2 and flags "rc"
                    property description (owner: #2, flags: "rc") = "This is a test object";
                    // empty flags are also permitted
                    property other (owner: #2, flags: "");
                endobject"#;

        let mut context = ObjFileContext::new();
        let odef =
            &compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap()[0];

        assert_eq!(odef.property_definitions.len(), 2);
        assert_eq!(odef.property_definitions[0].name, Symbol::mk("description"));
        assert_eq!(odef.property_definitions[0].perms.owner(), Obj::mk_id(2));
        assert_eq!(odef.property_definitions[0].perms.flags(), PropFlag::rc());
        let Some(s) = odef.property_definitions[0]
            .value
            .as_ref()
            .unwrap()
            .as_string()
        else {
            panic!("Expected string value");
        };
        assert_eq!(s, "This is a test object");

        assert_eq!(odef.property_definitions[1].name, Symbol::mk("other"));
        assert_eq!(odef.property_definitions[1].perms.owner(), Obj::mk_id(2));
        assert_eq!(odef.property_definitions[1].perms.flags(), BitEnum::new());
        assert!(odef.property_definitions[1].value.is_none());
    }

    #[test]
    fn object_with_property_override() {
        let spec = r#"
                object #1
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    // Overrides the description property from the parent
                    override description = "This is a test object";
                    override other (owner: #2, flags: "rc") = "test";
                endobject"#;
        let mut context = ObjFileContext::new();
        let odef =
            &compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap()[0];

        assert_eq!(odef.property_overrides.len(), 2);
        assert_eq!(odef.property_overrides[0].name, Symbol::mk("description"));
        let Some(s) = odef.property_overrides[0]
            .value
            .as_ref()
            .unwrap()
            .as_string()
        else {
            panic!("Expected string value");
        };
        assert_eq!(s, "This is a test object");

        assert_eq!(odef.property_overrides[1].name, Symbol::mk("other"));
        let Some(s) = odef.property_overrides[1]
            .value
            .as_ref()
            .unwrap()
            .as_string()
        else {
            panic!("Expected string value");
        };
        assert_eq!(s, "test");
    }

    #[test]
    fn a_mix_of_the_above_parses() {
        let spec = r#"
                object #1
                    // Testing a C++ Style comment
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    /* C style comment
                    *
                    */
                    property description (owner: #2, flags: "rc") = "This is a test object";
                    property other (owner: #2, flags: "rc");
                    override description = "This is a test object";
                    override other (owner: #2, flags: "rc") = "test";

                    override "@funky_prop_name" = "test";
                    
                    verb "look_self look_*" (this to any) owner: #2 flags: "rxd"
                        return 5;
                    endverb

                    verb another_test (this none this) owner: #2 flags: "r"
                        player:tell("here is something");
                    endverb


                endobject"#;
        let mut context = ObjFileContext::new();
        compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
    }

    #[test]
    fn test_various_literals() {
        let spec = r#"
                object #1
                    // Testing a C++ Style comment
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    override objid = #-1;
                    override string = "Hello World";
                    override integer = 12345;
                    override float = 123.45;
                    override error = E_INVIND;
                    override list = {1, 2, 3, 4};
                    override map = [ 1 -> 2, "test" -> 4 ];
                    override nested_list = { 1,2, { 5, 6, 7 }};
                    override nested_map = [ 1 -> [ 2 -> 3, 4 -> 5 ], 6 -> 7 ];
                    override flyweight = <#1, .a = 1, .b = 2, { 1,2, 3}>;
                endobject"#;
        let mut context = ObjFileContext::new();
        let odef =
            &compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();

        assert_eq!(
            odef[0].property_overrides[0]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_obj(NOTHING)
        );
        assert_eq!(
            odef[0].property_overrides[1]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_str("Hello World")
        );
        assert_eq!(
            odef[0].property_overrides[2]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_int(12345)
        );
        assert_eq!(
            odef[0].property_overrides[3]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_float(123.45)
        );
        assert_eq!(
            odef[0].property_overrides[4]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_err(E_INVIND)
        );
        assert_eq!(
            odef[0].property_overrides[5]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_list(&[v_int(1), v_int(2), v_int(3), v_int(4)])
        );
        assert_eq!(
            odef[0].property_overrides[6]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_map(&[(v_int(1), v_int(2)), (v_str("test"), v_int(4))])
        );
        assert_eq!(
            odef[0].property_overrides[7]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_list(&[v_int(1), v_int(2), v_list(&[v_int(5), v_int(6), v_int(7)])])
        );
        assert_eq!(
            odef[0].property_overrides[8]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_map(&[
                (
                    v_int(1),
                    v_map(&[(v_int(2), v_int(3)), (v_int(4), v_int(5))])
                ),
                (v_int(6), v_int(7))
            ])
        );
        assert_eq!(
            odef[0].property_overrides[9]
                .value
                .as_ref()
                .unwrap()
                .clone(),
            v_flyweight(
                Obj::mk_id(1),
                &[(Symbol::mk("a"), v_int(1)), (Symbol::mk("b"), v_int(2))],
                List::mk_list(&[v_int(1), v_int(2), v_int(3)]),
            )
        );
    }

    #[test]
    fn test_exotic_verbnames() {
        let spec = r#"
                object #1
                    // Testing a C++ Style comment
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    verb "@building-o*ptions @buildingo*ptions" (any any any) owner: #184 flags: "rd"
                        return 5;
                    endverb

                    verb "modname_# @recycle!" (any any any) owner: #184 flags: "rd"
                        return 5;
                    endverb

                    verb "contains_\"quote" (this none this) owner: #184 flags: "rxd"
                        player:tell("here is something");
                    endverb
                endobject"#;
        let mut context = ObjFileContext::new();
        compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
    }

    #[test]
    fn test_constants_usage() {
        let spec = r#"
                define ROOT = #1;
                define SYS_OBJ = #2;
                define MAGIC = "magic constant";
                define NESTED = { ROOT, SYS_OBJ, MAGIC };

                object #1
                    parent: ROOT
                    name: MAGIC
                    location: SYS_OBJ
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    property description (owner: SYS_OBJ, flags: "rc") = "This is a test object";

                    verb "contains_\"quote" (this none this) owner: ROOT flags: "rxd"
                        player:tell("here is something");
                    endverb
                endobject"#;

        let mut context = ObjFileContext::new();
        compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
    }

    /// Regression on quoted string constants for property names
    #[test]
    fn test_quoted_string_propname() {
        let spec = r#"object #1
                    // Testing a C++ Style comment
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    property " " (owner: #1, flags: "") = {"", "", {}, {}};
                    property just_regular (owner: #1, flags: "") = {"", "", {}, {}};
                    property "quoted normal" (owner: #1, flags: "") = {"", "", {}, {}};
                endobject"#;

        let mut context = ObjFileContext::new();
        let objs =
            compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
        let obj = &objs[0];

        assert_eq!(obj.property_definitions[0].name, Symbol::mk(" "));
        assert_eq!(obj.property_definitions[1].name, Symbol::mk("just_regular"));
        assert_eq!(
            obj.property_definitions[2].name,
            Symbol::mk("quoted normal")
        );
    }

    /// Test that binary literals in objdef format are parsed correctly
    #[test]
    fn test_binary_literal_in_objdef() {
        let spec = r#"object #1
                    parent: #1
                    name: "Test Object"
                    location: #3
                    wizard: false
                    programmer: false
                    player: false
                    fertile: true
                    readable: true

                    property binary_data (owner: #1, flags: "") = b"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk-M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
                endobject"#;

        let mut context = ObjFileContext::new();
        let objs =
            compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
        let obj = &objs[0];

        assert_eq!(obj.property_definitions.len(), 1);
        assert_eq!(obj.property_definitions[0].name, Symbol::mk("binary_data"));

        // Verify the value is a binary type
        let value = obj.property_definitions[0].value.as_ref().unwrap();
        let binary_data = value.as_binary().expect("Expected binary value");

        // Verify the decoded content is correct (this is a tiny 1x1 PNG)
        assert!(!binary_data.is_empty(), "Binary data should not be empty");
    }

    /// Test include! macro reads a text file and produces a String value.
    #[test]
    fn test_include_text_macro() {
        let dir = tempfile::tempdir().unwrap();
        let text_file = dir.path().join("greeting.txt");
        std::fs::write(&text_file, "Hello, world!").unwrap();

        // set_base_path takes a file path (uses .parent()), so give it a fake .moo path
        let fake_moo = dir.path().join("test.moo");

        let spec = r#"object #1
                    parent: #1
                    name: "Test"
                    wizard: false
                    programmer: false
                    player: false
                    fertile: false
                    readable: true

                    property greeting (owner: #1, flags: "") = include!("greeting.txt");
                endobject"#;

        let mut context = ObjFileContext::new();
        context.set_base_path(&fake_moo);
        let objs =
            compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
        let obj = &objs[0];

        assert_eq!(obj.property_definitions.len(), 1);
        let value = obj.property_definitions[0].value.as_ref().unwrap();
        assert_eq!(value.as_string().unwrap(), "Hello, world!");
    }

    /// Test include_bin! macro reads a binary file and produces a Binary value.
    #[test]
    fn test_include_bin_macro() {
        let dir = tempfile::tempdir().unwrap();
        let bin_file = dir.path().join("data.bin");
        let raw_bytes: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG header
        std::fs::write(&bin_file, &raw_bytes).unwrap();

        let fake_moo = dir.path().join("test.moo");

        let spec = r#"object #1
                    parent: #1
                    name: "Test"
                    wizard: false
                    programmer: false
                    player: false
                    fertile: false
                    readable: true

                    property sprite_data (owner: #1, flags: "") = include_bin!("data.bin");
                endobject"#;

        let mut context = ObjFileContext::new();
        context.set_base_path(&fake_moo);
        let objs =
            compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
        let obj = &objs[0];

        assert_eq!(obj.property_definitions.len(), 1);
        let value = obj.property_definitions[0].value.as_ref().unwrap();
        let binary_data = value.as_binary().expect("Expected binary value");
        assert_eq!(binary_data.as_bytes(), &raw_bytes[..]);
    }

    /// Test that include! fails without a base path (no file context).
    #[test]
    fn test_include_without_base_path_fails() {
        let spec = r#"object #1
                    parent: #1
                    name: "Test"
                    wizard: false
                    programmer: false
                    player: false
                    fertile: false
                    readable: true

                    property data (owner: #1, flags: "") = include!("some_file.txt");
                endobject"#;

        let mut context = ObjFileContext::new();
        let result = compile_object_definitions(spec, &CompileOptions::default(), &mut context);
        let err = result.err().expect("expected an error");
        assert!(
            err.to_string().contains("file-based compilation context"),
            "Expected context error, got: {err}"
        );
    }

    /// Test that include! rejects paths escaping the source directory.
    #[test]
    fn test_include_directory_traversal_rejected() {
        let dir = tempfile::tempdir().unwrap();
        // Create a file outside the base dir to attempt traversal to
        let parent_file = dir.path().join("secret.txt");
        std::fs::write(&parent_file, "secret").unwrap();

        // Base path is a subdirectory
        let sub_dir = dir.path().join("src");
        std::fs::create_dir(&sub_dir).unwrap();
        let fake_moo = sub_dir.join("test.moo");

        let spec = r#"object #1
                    parent: #1
                    name: "Test"
                    wizard: false
                    programmer: false
                    player: false
                    fertile: false
                    readable: true

                    property data (owner: #1, flags: "") = include!("../secret.txt");
                endobject"#;

        let mut context = ObjFileContext::new();
        context.set_base_path(&fake_moo);
        let result = compile_object_definitions(spec, &CompileOptions::default(), &mut context);
        let err = result.err().expect("expected an error");
        assert!(
            err.to_string().contains("escapes the source directory"),
            "Expected traversal error, got: {err}"
        );
    }

    /// Test that include! with a nonexistent file gives a clear error.
    #[test]
    fn test_include_missing_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        let fake_moo = dir.path().join("test.moo");

        let spec = r#"object #1
                    parent: #1
                    name: "Test"
                    wizard: false
                    programmer: false
                    player: false
                    fertile: false
                    readable: true

                    property data (owner: #1, flags: "") = include!("nonexistent.txt");
                endobject"#;

        let mut context = ObjFileContext::new();
        context.set_base_path(&fake_moo);
        let result = compile_object_definitions(spec, &CompileOptions::default(), &mut context);
        let err = result.err().expect("expected an error");
        assert!(
            err.to_string().contains("No such file")
                || err.to_string().contains("not found")
                || err.to_string().contains("IncludeError"),
            "Expected file-not-found error, got: {err}"
        );
    }

    /// Test include! in a subdirectory path (e.g. "assets/sprite.png").
    #[test]
    fn test_include_bin_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let assets_dir = dir.path().join("assets");
        std::fs::create_dir(&assets_dir).unwrap();
        let sprite_file = assets_dir.join("sprite.png");
        let png_bytes = vec![0x89, 0x50, 0x4E, 0x47]; // partial PNG header
        std::fs::write(&sprite_file, &png_bytes).unwrap();

        let fake_moo = dir.path().join("test.moo");

        let spec = r#"object #1
                    parent: #1
                    name: "Test"
                    wizard: false
                    programmer: false
                    player: false
                    fertile: false
                    readable: true

                    property sprite (owner: #1, flags: "") = include_bin!("assets/sprite.png");
                endobject"#;

        let mut context = ObjFileContext::new();
        context.set_base_path(&fake_moo);
        let objs =
            compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
        let obj = &objs[0];

        let value = obj.property_definitions[0].value.as_ref().unwrap();
        let binary_data = value.as_binary().expect("Expected binary value");
        assert_eq!(binary_data.as_bytes(), &png_bytes[..]);
    }

    /// Test include_bin! inside a list literal, e.g. {"image/png", include_bin!("...")}.
    #[test]
    fn test_include_bin_in_list() {
        let dir = tempfile::tempdir().unwrap();
        let bin_file = dir.path().join("tile.png");
        let png_bytes = vec![0x89, 0x50, 0x4E, 0x47];
        std::fs::write(&bin_file, &png_bytes).unwrap();

        let fake_moo = dir.path().join("test.moo");

        let spec = r#"object #1
                    parent: #1
                    name: "Test"
                    wizard: false
                    programmer: false
                    player: false
                    fertile: false
                    readable: true

                    property sprite (owner: #1, flags: "") = {"image/png", include_bin!("tile.png")};
                endobject"#;

        let mut context = ObjFileContext::new();
        context.set_base_path(&fake_moo);
        let objs =
            compile_object_definitions(spec, &CompileOptions::default(), &mut context).unwrap();
        let obj = &objs[0];

        let value = obj.property_definitions[0].value.as_ref().unwrap();
        // Should be a list: {"image/png", <binary>}
        let items = value.as_list().expect("Expected list value");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_string().unwrap(), "image/png");
        let binary_data = items[1].as_binary().expect("Expected binary in list");
        assert_eq!(binary_data.as_bytes(), &png_bytes[..]);
    }
}
