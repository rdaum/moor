pub mod bitenum;

/// Check `names` for matches with wildcard prefixes.
/// e.g. "dname*c" will match for any of 'dname', 'dnamec'
pub fn verbname_cmp(vname: &str, candidate: &str) -> bool {
    let mut vname_iter = vname.chars();
    let mut candidate_iter = candidate.chars();

    loop {
        match (vname_iter.next(), candidate_iter.next()) {
            (Some(v), Some(c)) => {
                if v == '*' {
                    return true;
                }
                if v != c {
                    return false;
                }
            }
            (Some(v), None) => {
                if v == '*' {
                    return true;
                }
                return false;
            }
            (None, Some(_)) => {
                return false;
            }
            (None, None) => {
                return true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::util::verbname_cmp;

    #[test]
    fn test_verb_match() {
        assert!(verbname_cmp("foo", "foo"));
        assert_eq!(verbname_cmp("foo", "foof"), false);
        assert!(verbname_cmp("foo*d", "foo"));
        assert!(verbname_cmp("foo*d", "food"));
    }
}
