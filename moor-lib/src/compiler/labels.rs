use bincode::{Decode, Encode};

/// A JumpLabel is what a labels resolve to in the program.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct JumpLabel {
    // The unique id for the jump label, which is also its offset in the jump vector.
    pub(crate) id: Label,

    // If there's a unique identifier assigned to this label, it goes here.
    pub(crate) name: Option<Name>,

    // The temporary and then final resolved position of the label in terms of PC offsets.
    pub(crate) position: Offset,
}

/// A Label is a unique identifier for a jump position in the program.
/// A committed, compiled, Label can be resolved to a program offset by looking it up in program's
/// jump vector at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct Label(pub u32);

impl From<usize> for Label {
    fn from(value: usize) -> Self {
        Label(value as u32)
    }
}

impl From<i32> for Label {
    fn from(value: i32) -> Self {
        Label(value as u32)
    }
}

/// A Name is a unique identifier for a variable in the program's environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, Hash)]
pub struct Name(pub u32);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Names {
    pub names: Vec<String>,
}

impl Default for Names {
    fn default() -> Self {
        Self::new()
    }
}

/// An offset is a program offset; a bit like a jump label, but represents a *relative* program
/// position
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Offset(pub usize);

impl From<usize> for Offset {
    fn from(value: usize) -> Self {
        Offset(value)
    }
}

impl Names {
    pub fn new() -> Self {
        let mut names = Self { names: vec![] };

        names.find_or_add_name("NUM");
        names.find_or_add_name("OBJ");
        names.find_or_add_name("STR");
        names.find_or_add_name("LIST");
        names.find_or_add_name("ERR");
        names.find_or_add_name("INT");
        names.find_or_add_name("FLOAT");
        names.find_or_add_name("player");
        names.find_or_add_name("this");
        names.find_or_add_name("caller");
        names.find_or_add_name("verb");
        names.find_or_add_name("args");
        names.find_or_add_name("argstr");
        names.find_or_add_name("dobj");
        names.find_or_add_name("dobjstr");
        names.find_or_add_name("prepstr");
        names.find_or_add_name("iobj");
        names.find_or_add_name("iobjstr");
        names
    }

    pub fn find_or_add_name(&mut self, name: &str) -> Name {
        match self
            .names
            .iter()
            .position(|n| n.to_lowercase().as_str() == name.to_lowercase())
        {
            None => {
                let pos = self.names.len();
                self.names.push(String::from(name));
                Name(pos as u32)
            }
            Some(n) => Name(n as u32),
        }
    }

    pub fn find_name(&self, name: &str) -> Option<Name> {
        self.find_name_offset(name).map(|x| Name(x as u32))
    }

    pub fn find_name_offset(&self, name: &str) -> Option<usize> {
        self.names
            .iter()
            .position(|x| x.to_lowercase() == name.to_lowercase())
    }
    pub fn width(&self) -> usize {
        self.names.len()
    }

    pub fn name_of(&self, name: &Name) -> Option<&str> {
        if name.0 as usize >= self.names.len() {
            return None;
        }
        Some(&self.names[name.0 as usize])
    }
}
