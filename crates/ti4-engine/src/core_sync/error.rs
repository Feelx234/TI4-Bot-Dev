use ti4_engine_core::state::Slice;

/// Something the typed core cannot express; the caller falls back to the string engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmapped {
    pub slice: Slice,
    pub what: &'static str,
    pub text: String,
}

impl std::fmt::Display for Unmapped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {} {:?}", self.slice, self.what, self.text)
    }
}

impl std::error::Error for Unmapped {}

pub type R<T> = Result<T, Unmapped>;

#[must_use]
pub fn bad(slice: Slice, what: &'static str, text: impl ToString) -> Unmapped {
    Unmapped { slice, what, text: text.to_string() }
}
