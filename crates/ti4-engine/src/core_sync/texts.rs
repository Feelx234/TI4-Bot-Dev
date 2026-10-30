//! The text codes of the state (`Code(u16)`) as `str`: the adapter side of `CoreState::texts`.
//!
//! The core holds bytes (`Interner`); the codecs, which speak `str`, intern and resolve through this
//! trait on the state's own tables. A code resolves in the lineage of the state that minted it.

use ti4_engine_core::state::{CODE_CAPACITY, Code, Slice, Texts};

use super::error::{R, bad};

/// Intern and resolve the open texts of a state.
pub trait TextCodes {
    /// The code of `text`, minted when new.
    ///
    /// # Errors
    /// More than 65 535 distinct texts.
    fn intern_code(&mut self, slice: Slice, text: &str) -> R<Code>;

    /// The text of a code.
    ///
    /// # Errors
    /// A code this table never issued.
    fn code_text(&self, slice: Slice, code: Code) -> R<&str>;
}

impl TextCodes for Texts {
    fn intern_code(&mut self, slice: Slice, text: &str) -> R<Code> {
        match self.codes.intern(text.as_bytes(), CODE_CAPACITY) {
            Some(i) => Ok(Code(i as u16)),
            None => Err(bad(slice, "text table full", text)),
        }
    }

    fn code_text(&self, slice: Slice, code: Code) -> R<&str> {
        let bytes = self.codes.get(u32::from(code.0)).ok_or_else(|| bad(slice, "unknown text code", code.0))?;
        std::str::from_utf8(bytes).map_err(|_| bad(slice, "text code is not UTF-8", code.0))
    }
}
