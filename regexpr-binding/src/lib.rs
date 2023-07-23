#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::error::Error;
use std::ffi::CString;
use std::fmt::{Debug, Display, Formatter};

use once_cell::sync::Lazy;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub struct Pattern {
    // Need to hold here to keep allocated.
    _pattern_str: CString,
    pattern_ptr: *mut re_pattern_buffer,
    fastmap_ptr: *mut [i8; 256],
}

impl Drop for Pattern {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.pattern_ptr));
            drop(Box::from_raw(self.fastmap_ptr));
        }
    }
}

#[derive(Eq, PartialEq, Clone)]
pub enum CompileError {
    FailedCompile(String),
}

impl Debug for CompileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self))
    }
}

impl Display for CompileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::FailedCompile(msg) => write!(f, "Failed to compile pattern: {}", msg),
        }
    }
}

impl Error for CompileError {}

#[derive(Eq, PartialEq, Clone)]
pub enum MatchError {
    Failed,
    Aborted,
}

impl Debug for MatchError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self))
    }
}

impl Display for MatchError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchError::Failed => write!(f, "Failed"),
            MatchError::Aborted => write!(f, "Aborted"),
        }
    }
}

static CASEFOLD_ARRAY: Lazy<[u8; 256]> = Lazy::new(mk_casefold);
fn mk_casefold() -> [u8; 256] {
    let mut casefold = [0u8; 256];
    for (i, c) in casefold.iter_mut().enumerate() {
        let char = i as u8;
        if char.is_ascii_uppercase() {
            *c = char.to_ascii_lowercase();
        } else {
            *c = char;
        }
    }
    casefold
}

impl Error for MatchError {}

impl Pattern {
    pub fn new(pattern_string: &str, case_matters: bool) -> Result<Self, CompileError> {
        let pattern_string = translate_pattern(pattern_string);
        let pattern_str_len = pattern_string.len();
        let fastmap = Box::new([0i8; 256]);
        let fastmap_ptr = Box::into_raw(fastmap);
        let mut pattern = Box::new(re_pattern_buffer {
            buffer: std::ptr::null_mut(),
            allocated: 0,
            used: 0,
            fastmap: std::ptr::null_mut(),
            translate: std::ptr::null_mut(),
            fastmap_accurate: 0,
            can_be_null: 0,
            uses_registers: 0,
            anchor: 0,
        });
        if !case_matters {
            pattern.translate = CASEFOLD_ARRAY.as_ptr() as _;
        }
        let pattern_ptr = Box::into_raw(pattern);
        let pattern_str_c = CString::new(pattern_string).unwrap();
        unsafe {
            // Need to call this to make sure that the regex library is initialized.
            re_set_syntax(RE_CONTEXT_INDEP_OPS as _);

            let compile_result = re_compile_pattern(
                pattern_str_c.as_ptr() as _,
                pattern_str_len as _,
                pattern_ptr,
            );
            // (If the result is non-null, it's an error message in a string.)
            if !compile_result.is_null() {
                let c_str = std::ffi::CStr::from_ptr(compile_result);
                let str_slice = c_str.to_str().unwrap();
                let err_msg = str_slice.to_string();
                return Err(CompileError::FailedCompile(err_msg));
            }
            (*pattern_ptr).fastmap = fastmap_ptr as _;

            re_compile_fastmap(pattern_ptr);
        }
        Ok(Pattern {
            _pattern_str: pattern_str_c,
            pattern_ptr,
            fastmap_ptr,
        })
    }

    pub fn match_pattern(&self, string: &str) -> Result<Vec<(isize, isize)>, MatchError> {
        let mut regs = re_registers {
            start: [0; 100],
            end: [0; 100],
        };
        let string_c_str = CString::new(string).unwrap();
        let len = string.len() as _;
        let match_result = unsafe {
            re_search(
                self.pattern_ptr,
                string_c_str.as_ptr() as _,
                len,
                0,
                len,
                &mut regs as *mut _,
            )
        };
        if match_result >= 0 {
            let mut indices = Vec::new();
            for i in 0..10 {
                // Convert from 0-based open interval to 1-based closed one. */
                let start = regs.start[i as usize] + 1;
                let end = regs.end[i as usize];
                indices.push((start as isize, end as isize));
            }
            Ok(indices)
        } else {
            match match_result {
                -1 => Err(MatchError::Failed),
                -2 => Err(MatchError::Aborted),
                _ => panic!("Unexpected return value from re_search: {}", match_result),
            }
        }
    }
}

/// Translate a MOO pattern into a more standard syntax.  Effectively, this
/// just involves converting from `%' escapes into `\' escapes.
fn translate_pattern(pattern: &str) -> String {
    let mut s = String::with_capacity(pattern.len());
    let mut idx = 0;
    while idx < pattern.len() {
        let c = pattern.chars().nth(idx).unwrap();
        match c {
            '%' => {
                idx += 1;
                let c = pattern.chars().nth(idx).unwrap();
                match c {
                    '.'
                    | '*'
                    | '+'
                    | '?'
                    | '['
                    | '^'
                    | '$'
                    | '|'
                    | '('
                    | ')'
                    | '1'..='9'
                    | 'b'
                    | 'B'
                    | '<'
                    | '>'
                    | 'w'
                    | 'W' => {
                        s.push('\\');
                    }
                    _ => {}
                }
                s.push(c);
            }
            '\\' => {
                s.push_str("\\\\");
            }
            '[' => {
                // Any '%' or '\' characters inside a charset should be copied
                // over without translation.
                s.push('[');
                idx += 1;
                let c = pattern.chars().nth(idx).unwrap();
                if c == '^' {
                    s.push('^');
                    idx += 1;
                }
                // This is the only place a ']' can appear and not be the end of
                //  the charset.
                if c == ']' {
                    s.push(']');
                    idx += 1;
                }
                while idx < pattern.len() {
                    let c = pattern.chars().nth(idx).unwrap();
                    if c == ']' {
                        s.push(']');
                        idx += 1;
                        break;
                    }
                    s.push(c);
                    idx += 1;
                }
            }
            _ => {
                s.push(c);
            }
        }
        idx += 1;
    }
    s
}

#[cfg(test)]
mod tests {
    use std::ffi::{c_int, CString};

    use crate::{
        re_compile_fastmap, re_compile_pattern, re_pattern_buffer, re_registers, re_search,
        re_set_syntax, CompileError, Pattern, RE_CONTEXT_INDEP_OPS,
    };

    #[no_mangle]
    #[used]
    pub static mut task_timed_out: u64 = 0;

    #[test]
    fn raw_bindings_sanity_test() {
        let mut fastmap = [0i8; 256];
        let mut pattern = re_pattern_buffer {
            buffer: std::ptr::null_mut(),
            allocated: 0,
            used: 0,
            fastmap: std::ptr::null_mut(),
            translate: std::ptr::null_mut(),
            fastmap_accurate: 0,
            can_be_null: 0,
            uses_registers: 0,
            anchor: 0,
        };
        unsafe {
            // Need to call this to make sure that the regex library is initialized.
            re_set_syntax(RE_CONTEXT_INDEP_OPS as _);

            let pattern_str = r#"^The.*Spain$"#;
            let pattern_string = CString::new(pattern_str).unwrap();
            let pattern_ptr = &mut pattern as *mut _;
            let compile_result = re_compile_pattern(
                pattern_string.as_ptr() as _,
                pattern_str.len() as _,
                pattern_ptr,
            );
            // (If the result is non-null, it's an error message in a string.)
            if !compile_result.is_null() {
                let c_str = std::ffi::CStr::from_ptr(compile_result);
                let str_slice = c_str.to_str().unwrap();
                assert!(false, "re_compile_pattern failed: {}", str_slice);
            }
            pattern.fastmap = fastmap.as_mut_ptr();

            re_compile_fastmap(pattern_ptr);
            assert_eq!((*pattern_ptr).fastmap_accurate, 1);
            // simple match in the pattern
            let str = "The rain in Spain";
            let match_string = CString::new(str).unwrap();

            let mut regs = re_registers {
                start: [0; 100],
                end: [0; 100],
            };
            let len: c_int = str.len() as _;
            let match_result = re_search(
                pattern_ptr,
                match_string.as_ptr() as _,
                len,
                0,
                len,
                &mut regs as *mut _,
            );
            assert!(match_result >= 0, "re_search failed: {}", match_result);
            let (start, end) = (regs.start[0], regs.end[0]);
            assert_eq!((start, end), (0, 17));
        }
    }

    #[test]
    fn test_compile_pattern_wrapper() {
        Pattern::new(r#"^The.*Spain$"#, false).unwrap();
        assert_eq!(
            Pattern::new(r#"*"#, false).err().unwrap(),
            CompileError::FailedCompile("Badly placed special character".to_string())
        );
    }

    #[test]
    fn test_match_case_sensitive() {
        let pattern = Pattern::new(r#"^The.*Spain$"#, false).unwrap();
        let match_result = pattern.match_pattern("The rain in Spain").unwrap();
        assert_eq!(match_result[0], (1, 17));
    }

    #[test]
    fn test_match_case_insensitive() {
        let pattern = Pattern::new(r#"^The.*Spain$"#, false).unwrap();
        let match_result = pattern.match_pattern("the rain in spain").unwrap();
        assert_eq!(match_result[0], (1, 17));
    }
}
