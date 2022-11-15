/// TODO verb-matching
///
pub enum ArgSpec {
    None,
    Any,
    This
}

pub enum PrepSpec {
    Any,
    None,
}

pub struct VerbArgsSpec {
    dobj: ArgSpec,
    prep: PrepSpec,
    iobj: ArgSpec
}

pub trait Match {

}
