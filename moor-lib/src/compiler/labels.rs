use bincode::{Decode, Encode};

// Fixup for a jump label
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct JumpLabel {
    // The unique id for the jump label, which is also its offset in the jump vector.
    pub(crate) id: Label,

    // If there's a unique identifier assigned to this label, it goes here.
    pub(crate) label: Option<Name>,

    // The temporary and then final resolved position of the label in terms of PC offsets.
    pub(crate) position: Offset,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Name(pub Label);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Names {
    pub names: Vec<String>,
}

impl Default for Names {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct Offset(pub u32);

impl From<i32> for Offset {
    fn from(value: i32) -> Self {
        Offset(value as u32)
    }
}

impl From<usize> for Offset {
    fn from(value: usize) -> Self {
        Offset(value as u32)
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
                Name(pos.into())
            }
            Some(n) => Name(n.into()),
        }
    }
    pub fn find_label(&self, label: &Label) -> Option<Name> {
        if label.0 as usize >= self.names.len() {
            return None;
        }
        Some(Name(*label))
    }

    pub fn find_name(&self, name: &str) -> Option<Name> {
        self.find_name_offset(name).map(|x| Name(x.into()))
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
        if name.0 .0 as usize >= self.names.len() {
            return None;
        }
        Some(&self.names[name.0 .0 as usize])
    }
}
