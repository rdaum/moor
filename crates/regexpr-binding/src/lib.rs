#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::convert::TryInto;
use std::error::Error;
use std::ffi::{c_int, CString};
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
        f.write_fmt(format_args!("{self}"))
    }
}

impl Display for CompileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FailedCompile(msg) => write!(f, "Failed to compile pattern: {msg}"),
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
        f.write_fmt(format_args!("{self}"))
    }
}

impl Display for MatchError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed => write!(f, "Failed"),
            Self::Aborted => write!(f, "Aborted"),
        }
    }
}

static CASEFOLD_ARRAY: Lazy<[u8; 256]> = Lazy::new(mk_casefold);
fn mk_casefold() -> [u8; 256] {
    (0..=255)
        .map(|c: u8| c.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

impl Error for MatchError {}
type Span = (isize, isize);
type MatchSpans = (Span, Vec<Span>);

impl Pattern {
    pub fn new(pattern_string: &str, case_matters: bool) -> Result<Self, CompileError> {
        let Some(pattern_string) = translate_pattern(pattern_string) else {
            return Err(CompileError::FailedCompile(
                "bad pattern translation".to_string(),
            ));
        };
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
                pattern_str_c.as_ptr().cast_mut(),
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
            (*pattern_ptr).fastmap = fastmap_ptr.cast();

            re_compile_fastmap(pattern_ptr);
        }
        Ok(Self {
            _pattern_str: pattern_str_c,
            pattern_ptr,
            fastmap_ptr,
        })
    }

    fn do_match_pattern(&self, string: &str, is_reverse: bool) -> Result<MatchSpans, MatchError> {
        let mut regs = re_registers {
            start: [0; 100],
            end: [0; 100],
        };
        let len = string.len() as c_int;
        let string_c_str = CString::new(string).unwrap();
        let (startpos, range) = if is_reverse { (len, -len) } else { (0, len) };
        let match_result = unsafe {
            re_search(
                self.pattern_ptr,
                string_c_str.as_ptr().cast_mut(),
                len,
                startpos,
                range,
                std::ptr::addr_of_mut!(regs),
            )
        };
        if match_result >= 0 {
            // First indices are the overall match. The rest are the submatches.
            let overall = (regs.start[0] as isize + 1, regs.end[0] as isize);
            let mut indices = Vec::new();
            for i in 1..10 {
                // Convert from 0-based open interval to 1-based closed one.
                let start = regs.start[i as usize] + 1;
                let end = regs.end[i as usize];
                indices.push((start as isize, end as isize));
            }

            Ok((overall, indices))
        } else {
            match match_result {
                -1 => Err(MatchError::Failed),
                -2 => Err(MatchError::Aborted),
                _ => panic!("Unexpected return value from re_search: {match_result}"),
            }
        }
    }

    pub fn match_pattern(&self, string: &str) -> Result<MatchSpans, MatchError> {
        self.do_match_pattern(string, false)
    }

    pub fn reverse_match_pattern(&self, string: &str) -> Result<MatchSpans, MatchError> {
        self.do_match_pattern(string, true)
    }
}

/// Translate a MOO pattern into a more standard syntax.  Effectively, this
/// just involves remove `%' escapes into `\' escapes.
fn translate_pattern(pattern: &str) -> Option<String> {
    let mut s = String::with_capacity(pattern.len());
    let mut c_iter = pattern.chars();
    loop {
        let Some(mut c) = c_iter.next() else {
            break;
        };
        if c == '%' {
            let Some(escape) = c_iter.next() else {
                return None;
            };
            if ".*+?[^$|()123456789bB<>wW".contains(escape) {
                s.push('\\');
            }
            s.push(escape);
            continue;
        }
        if c == '\\' {
            s.push_str("\\\\");
            continue;
        }
        if c == '[' {
            /* Any '%' or '\' characters inside a charset should be copied
             * over without translation. */
            s.push(c);
            let Some(next) = c_iter.next() else {
                return None;
            };
            c = next;
            if c == '^' {
                s.push(c);
                let Some(next) = c_iter.next() else {
                    return None;
                };
                c = next;
            }
            if c == ']' {
                s.push(c);
                let Some(next) = c_iter.next() else {
                    return None;
                };
                c = next;
            }
            while c != ']' {
                s.push(c);
                let Some(next) = c_iter.next() else {
                    return None;
                };
                c = next;
            }
            s.push(c);
            continue;
        }
        s.push(c);
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use std::ffi::{c_int, CString};

    use crate::{
        re_compile_fastmap, re_compile_pattern, re_pattern_buffer, re_registers, re_search,
        re_set_syntax, translate_pattern, CompileError, Pattern, RE_CONTEXT_INDEP_OPS,
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
            let pattern_ptr = std::ptr::addr_of_mut!(pattern);
            let compile_result = re_compile_pattern(
                pattern_string.as_ptr().cast_mut(),
                pattern_str.len() as _,
                pattern_ptr,
            );
            // (If the result is non-null, it's an error message in a string.)
            if !compile_result.is_null() {
                let c_str = std::ffi::CStr::from_ptr(compile_result);
                let str_slice = c_str.to_str().unwrap();
                panic!("re_compile_pattern failed: {str_slice}");
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
                match_string.as_ptr().cast_mut(),
                len,
                0,
                len,
                std::ptr::addr_of_mut!(regs),
            );
            assert!(match_result >= 0, "re_search failed: {match_result}");
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
        let (overall, match_result) = pattern.match_pattern("The rain in Spain").unwrap();
        assert_eq!(overall, (1, 17));
        assert_eq!(match_result[0], (0, -1));
    }

    #[test]
    fn test_match_case_insensitive() {
        let pattern = Pattern::new(r#"^The.*Spain$"#, false).unwrap();
        let (overall, match_result) = pattern.match_pattern("the rain in spain").unwrap();
        assert_eq!(overall, (1, 17));
        assert_eq!(match_result[0], (0, -1));
    }

    #[test]
    fn test_subs_match() {
        //    match("foobar", "f%(o*%)b")
        //             =>  {1, 4, {{2, 3}, {0, -1}, ...}, "foobar"}
        let pattern = Pattern::new(r#"f%(o*%)b"#, false).unwrap();
        let (overall, match_result) = pattern.match_pattern("foobar").unwrap();
        assert_eq!(overall, (1, 4));
        assert_eq!(match_result[0], (2, 3));
        assert_eq!(match_result[1], (0, -1));
    }

    #[test]
    fn test_reverse_match() {
        // from `help`:
        // rmatch("foobar", "o*b")      =>  {4, 4, {{0, -1}, ...}, "foobar"}
        let pattern = Pattern::new(r#"o*b"#, false).unwrap();
        let (overall, _match_result) = pattern.reverse_match_pattern("foobar").unwrap();
        assert_eq!(overall, (4, 4));
    }

    #[test]
    fn test_pattern_translation() {
        let pattern = "^.* %(from%|to%) %([^, ]+%)";
        let translated = translate_pattern(pattern);
        assert_eq!(
            translated,
            Some("^.* \\(from\\|to\\) \\([^, ]+\\)".to_string())
        );
    }

    #[test]
    fn test_regression_hostname_pattern() {
        // Based on a real-world example. Was fixed by fixing `translate_pattern`.

        let pattern = Pattern::new("^.* %(from%|to%) %([^, ]+%)", false).unwrap();
        let (overall, match_result) = pattern
            .match_pattern("port 7777 from 127.0.0.1, port 48610")
            .unwrap();
        // MOO is returning:
        //   {1, 24, {{11, 14}, {16, 24}, {0, -1}, {0, -1}, {0, -1}, {0, -1}, {0, -1}, {0, -1}, {0, -1},  "port 7777 from 127.0.0.1, port 48610"}
        // But our version was returning:
        //   {1, 16, {{11, 14}, {16, 16}, {0, -1}, {0, -1}, {0, -1}, {0, -1}, {0, -1}, {0, -1}, {0, -1}}, "port 7777 from 127.0.0.1, port 48610"}
        assert_eq!(overall, (1, 24));
        assert_eq!(match_result[0], (11, 14));
        assert_eq!(match_result[1], (16, 24));
        assert_eq!(match_result[2], (0, -1));
    }
}
