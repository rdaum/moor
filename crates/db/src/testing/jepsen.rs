//! For loading Jepsen-produced history files.

#[derive(Debug, serde::Deserialize, Copy, Clone, PartialEq)]
#[allow(non_camel_case_types)]
pub enum Type {
    invoke,
    ok,
    fail,
}
impl Type {
    pub fn to_keyword(&self) -> &str {
        match self {
            Type::invoke => "invoke",
            Type::ok => "ok",
            Type::fail => "fail",
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct History {
    pub f: String,
    pub index: i64,
    pub process: i64,
    pub time: i64,
    pub r#type: Type,
    pub value: Vec<Value>,
}

// ["append",9,1]
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
#[allow(non_camel_case_types)]
pub enum Value {
    append(String, i64, i64),
    r(String, i64, Option<Vec<i64>>),
}
