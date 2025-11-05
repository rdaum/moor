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

use strsim::damerau_levenshtein;
use strum::FromRepr;

/// The set of prepositions that are valid for verbs, corresponding to the set of string constants
/// defined in LambdaMOO 1.8.1.
/// TODO: Refactor/rethink preposition enum.
///   Long run a proper table with some sort of dynamic look up and a way to add new ones and
///   internationalize and so on.
#[repr(u16)]
#[derive(Copy, Clone, Debug, FromRepr, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Preposition {
    WithUsing = 0,
    AtTo = 1,
    InFrontOf = 2,
    IntoIn = 3,
    OnTopOfOn = 4,
    OutOf = 5,
    Over = 6,
    Through = 7,
    Under = 8,
    Behind = 9,
    Beside = 10,
    ForAbout = 11,
    Is = 12,
    As = 13,
    OffOf = 14,
    /// mooR extension: not present in LambdaMOO
    NamedCalled = 15,
}

impl Preposition {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "with/using" | "with" | "using" => Some(Self::WithUsing),
            "at/to" | "at" | "to" => Some(Self::AtTo),
            "in front of" | "in-front-of" => Some(Self::InFrontOf),
            "in/inside/into" | "in" | "inside" | "into" => Some(Self::IntoIn),
            "on top of/on/onto/upon" | "on top of" | "on" | "onto" | "upon" => {
                Some(Self::OnTopOfOn)
            }
            "out of/from inside/from" | "out of" | "from inside" | "from" => Some(Self::OutOf),
            "over" => Some(Self::Over),
            "through" => Some(Self::Through),
            "under/underneath/beneath" | "under" | "underneath" | "beneath" => Some(Self::Under),
            "behind" => Some(Self::Behind),
            "beside" => Some(Self::Beside),
            "for/about" | "for" | "about" => Some(Self::ForAbout),
            "is" => Some(Self::Is),
            "as" => Some(Self::As),
            "off/off of" | "off" | "off of" => Some(Self::OffOf),
            "named/called/known as" | "named" | "called" | "known as" => Some(Self::NamedCalled),
            _ => None,
        }
    }
    pub fn to_string(&self) -> &str {
        match self {
            Self::WithUsing => "with/using",
            Self::AtTo => "at/to",
            Self::InFrontOf => "in front of",
            Self::IntoIn => "in/inside/into",
            Self::OnTopOfOn => "on top of/on/onto/upon",
            Self::OutOf => "out of/from inside/from",
            Self::Over => "over",
            Self::Through => "through",
            Self::Under => "under/underneath/beneath",
            Self::Behind => "behind",
            Self::Beside => "beside",
            Self::ForAbout => "for/about",
            Self::Is => "is",
            Self::As => "as",
            Self::OffOf => "off/off of",
            Self::NamedCalled => "named/called/known as",
        }
    }

    /// Output only one preposition, instead of the full break down.
    /// For output in objdefs, etc where space-separation is required
    pub fn to_string_single(&self) -> &str {
        match self {
            Self::WithUsing => "with",
            Self::AtTo => "at",
            Self::InFrontOf => "in-front-of",
            Self::IntoIn => "in",
            Self::OnTopOfOn => "on",
            Self::OutOf => "from",
            Self::Over => "over",
            Self::Through => "through",
            Self::Under => "under",
            Self::Behind => "behind",
            Self::Beside => "beside",
            Self::ForAbout => "for",
            Self::Is => "is",
            Self::As => "as",
            Self::OffOf => "off",
            Self::NamedCalled => "named",
        }
    }
}

pub fn find_preposition(prep: &str) -> Option<Preposition> {
    // If the string is a number, treat it as a preposition ID.
    if let Ok(id) = prep.parse::<u16>() {
        return Preposition::from_repr(id);
    }

    // Try exact match first
    if let Some(preposition) = Preposition::parse(prep) {
        return Some(preposition);
    }

    // If no exact match, try fuzzy matching
    find_preposition_fuzzy(prep)
}

/// Find preposition for command parsing - doesn't treat numbers as preposition IDs
/// and only uses exact matches (no fuzzy matching to avoid false positives)
pub fn find_preposition_for_command(prep: &str) -> Option<Preposition> {
    // Only try exact match - no fuzzy matching for command parsing
    // to avoid false positives like "thing" matching "with"
    Preposition::parse(prep)
}

/// Find preposition using fuzzy matching with edit distance
fn find_preposition_fuzzy(prep: &str) -> Option<Preposition> {
    let prep_lower = prep.to_lowercase();
    let max_distance = if prep_lower.len() <= 3 { 1 } else { 2 };

    let all_prepositions = [
        Preposition::WithUsing,
        Preposition::AtTo,
        Preposition::InFrontOf,
        Preposition::IntoIn,
        Preposition::OnTopOfOn,
        Preposition::OutOf,
        Preposition::Over,
        Preposition::Through,
        Preposition::Under,
        Preposition::Behind,
        Preposition::Beside,
        Preposition::ForAbout,
        Preposition::Is,
        Preposition::As,
        Preposition::OffOf,
        Preposition::NamedCalled,
    ];

    let mut best_match = None;
    let mut best_distance = usize::MAX;

    for preposition in &all_prepositions {
        // Get all possible forms for this preposition
        let forms = get_preposition_forms(*preposition);

        for form in forms {
            let form_lower = form.to_lowercase();
            let distance = damerau_levenshtein(&prep_lower, &form_lower);

            if distance <= max_distance && distance < best_distance {
                best_match = Some(*preposition);
                best_distance = distance;

                // If we find a perfect match, return immediately
                if distance == 0 {
                    return best_match;
                }
            }
        }
    }

    best_match
}

/// Get all possible string forms for a preposition (for fuzzy matching)
fn get_preposition_forms(prep: Preposition) -> Vec<&'static str> {
    match prep {
        Preposition::WithUsing => vec!["with", "using"],
        Preposition::AtTo => vec!["at", "to"],
        Preposition::InFrontOf => vec!["in front of", "in-front-of"],
        Preposition::IntoIn => vec!["in", "inside", "into"],
        Preposition::OnTopOfOn => vec!["on top of", "on", "onto", "upon"],
        Preposition::OutOf => vec!["out of", "from inside", "from"],
        Preposition::Over => vec!["over"],
        Preposition::Through => vec!["through"],
        Preposition::Under => vec!["under", "underneath", "beneath"],
        Preposition::Behind => vec!["behind"],
        Preposition::Beside => vec!["beside"],
        Preposition::ForAbout => vec!["for", "about"],
        Preposition::Is => vec!["is"],
        Preposition::As => vec!["as"],
        Preposition::OffOf => vec!["off", "off of"],
        Preposition::NamedCalled => vec!["named", "called", "known as"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_preposition_exact_match() {
        assert_eq!(find_preposition("with"), Some(Preposition::WithUsing));
        assert_eq!(find_preposition("at"), Some(Preposition::AtTo));
        assert_eq!(find_preposition("behind"), Some(Preposition::Behind));
        assert_eq!(find_preposition("through"), Some(Preposition::Through));
    }

    #[test]
    fn test_find_preposition_fuzzy_simple_typos() {
        // Single character substitution
        assert_eq!(find_preposition("woth"), Some(Preposition::WithUsing)); // "with"
        assert_eq!(find_preposition("ot"), Some(Preposition::AtTo)); // "at"
        assert_eq!(find_preposition("behond"), Some(Preposition::Behind)); // "behind"

        // Single character deletion
        assert_eq!(find_preposition("wit"), Some(Preposition::WithUsing)); // "with"
        assert_eq!(find_preposition("a"), Some(Preposition::AtTo)); // "at"

        // Single character insertion
        assert_eq!(find_preposition("withe"), Some(Preposition::WithUsing)); // "with"
        assert_eq!(find_preposition("att"), Some(Preposition::AtTo)); // "at"
    }

    #[test]
    fn test_debug_problematic_inputs() {
        // Test the specific inputs that are causing issues in command parsing
        assert_eq!(find_preposition_for_command("+"), None); // Should not match any preposition
        assert_eq!(find_preposition_for_command("1"), None); // Should not match any preposition
        assert_eq!(find_preposition_for_command("as"), Some(Preposition::As)); // Should work
        assert_eq!(find_preposition_for_command("thing"), None); // Should not match

        // But numeric IDs should still work for direct preposition lookup
        assert_eq!(find_preposition("1"), Some(Preposition::AtTo));
    }

    #[test]
    fn test_find_preposition_fuzzy_transposition() {
        // Adjacent character swap
        assert_eq!(find_preposition("iwth"), Some(Preposition::WithUsing)); // "with"
        assert_eq!(find_preposition("beihnd"), Some(Preposition::Behind)); // "behind"
    }

    #[test]
    fn test_find_preposition_fuzzy_longer_words() {
        // Longer words allow distance 2
        assert_eq!(find_preposition("thrugh"), Some(Preposition::Through)); // "through"
        assert_eq!(find_preposition("undernath"), Some(Preposition::Under)); // "underneath"
        assert_eq!(find_preposition("besid"), Some(Preposition::Beside)); // "beside"
    }

    #[test]
    fn test_find_preposition_fuzzy_prioritizes_shorter_forms() {
        // Should match "in" rather than "inside" when fuzzy matching "ni"
        assert_eq!(find_preposition("ni"), Some(Preposition::IntoIn)); // closer to "in"
        // "no" matches both "to" and "on" with distance 1, algorithm picks first found
        assert_eq!(find_preposition("no"), Some(Preposition::AtTo)); // matches "to"
    }

    #[test]
    fn test_find_preposition_fuzzy_no_match() {
        // Too many changes - should not match
        assert_eq!(find_preposition("xyz"), None);
        assert_eq!(find_preposition("qwerty"), None);
    }

    #[test]
    fn test_find_preposition_exact_beats_fuzzy() {
        // Exact matches should still work
        assert_eq!(find_preposition("in"), Some(Preposition::IntoIn));
        assert_eq!(find_preposition("on"), Some(Preposition::OnTopOfOn));
        assert_eq!(find_preposition("as"), Some(Preposition::As));
        assert_eq!(find_preposition("is"), Some(Preposition::Is));
    }

    #[test]
    fn test_find_preposition_numeric_ids() {
        // Numeric IDs should still work for direct preposition lookup
        assert_eq!(find_preposition("0"), Some(Preposition::WithUsing));
        assert_eq!(find_preposition("1"), Some(Preposition::AtTo));
        assert_eq!(find_preposition("7"), Some(Preposition::Through));
    }

    #[test]
    fn test_find_preposition_for_command() {
        // Command parsing should work with word prepositions
        assert_eq!(
            find_preposition_for_command("with"),
            Some(Preposition::WithUsing)
        );
        assert_eq!(find_preposition_for_command("at"), Some(Preposition::AtTo));

        // But should NOT treat numbers as preposition IDs
        assert_eq!(find_preposition_for_command("0"), None);
        assert_eq!(find_preposition_for_command("1"), None);
        assert_eq!(find_preposition_for_command("7"), None);

        // Command parsing should NOT do fuzzy matching to avoid false positives
        assert_eq!(find_preposition_for_command("wit"), None);
        assert_eq!(find_preposition_for_command("ot"), None);
    }

    #[test]
    fn test_find_preposition_compound_forms() {
        // Multi-word prepositions
        assert_eq!(
            find_preposition("in front of"),
            Some(Preposition::InFrontOf)
        );
        assert_eq!(find_preposition("out of"), Some(Preposition::OutOf));
        assert_eq!(find_preposition("on top of"), Some(Preposition::OnTopOfOn));

        // Fuzzy matching on compound forms
        assert_eq!(find_preposition("in frnt of"), Some(Preposition::InFrontOf)); // "front" typo
        assert_eq!(find_preposition("ou of"), Some(Preposition::OutOf)); // "out" typo
    }

    #[test]
    fn test_find_preposition_named_called() {
        // mooR extension: named/called/known as preposition
        assert_eq!(find_preposition("named"), Some(Preposition::NamedCalled));
        assert_eq!(find_preposition("called"), Some(Preposition::NamedCalled));
        assert_eq!(
            find_preposition("known as"),
            Some(Preposition::NamedCalled)
        );
        assert_eq!(
            find_preposition("named/called/known as"),
            Some(Preposition::NamedCalled)
        );

        // Numeric ID
        assert_eq!(find_preposition("15"), Some(Preposition::NamedCalled));

        // Command parsing should work
        assert_eq!(
            find_preposition_for_command("named"),
            Some(Preposition::NamedCalled)
        );
        assert_eq!(
            find_preposition_for_command("called"),
            Some(Preposition::NamedCalled)
        );
        assert_eq!(
            find_preposition_for_command("known as"),
            Some(Preposition::NamedCalled)
        );

        // But should NOT treat number as ID in command parsing
        assert_eq!(find_preposition_for_command("15"), None);
    }
}
