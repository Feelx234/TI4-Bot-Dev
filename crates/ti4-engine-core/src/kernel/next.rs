//! `Next`: what a rule area returns instead of calling another area (docs/ENGINE_CORE_STATE.md 4.1).
//!
//! L4 areas do not import each other (65 module pairs in the string engine do). An action card that
//! "produces 1 unit" returns `Next::Open(Window{ kind: Production, .. })`; the driver (L5) dispatches
//! the window to the production area. Pure helpers several areas share are [`super::fx`] primitives.

use crate::choice::Ask;
use crate::containers::SmallList;
use crate::state::{EventRec, Window};

/// The continuation of a rule step.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant, reason = "one value per step, never stored in bulk; boxing the event would cost an allocation per emission")]
pub enum Next {
    /// Nothing more to do.
    Done,
    /// Open a window of another area; the driver dispatches it.
    Open(Window),
    /// Emit an event through the timing windows.
    Emit(EventRec),
    /// Put this question to its actor.
    Continue(Ask),
    /// Several continuations, in order.
    Chain(Vec<Next>),
}

impl Next {
    /// The continuations of `self` flattened in order (`Done` dropped).
    #[must_use]
    pub fn flatten(self) -> Vec<Self> {
        match self {
            Self::Done => Vec::new(),
            Self::Chain(items) => items.into_iter().flat_map(Self::flatten).collect(),
            other => vec![other],
        }
    }

    /// Whether nothing is left to do.
    #[must_use]
    pub fn is_done(&self) -> bool {
        match self {
            Self::Done => true,
            Self::Chain(items) => items.iter().all(Self::is_done),
            _ => false,
        }
    }

    /// `self` then `next`.
    #[must_use]
    pub fn then(self, next: Self) -> Self {
        match (self, next) {
            (Self::Done, n) | (n, Self::Done) => n,
            (Self::Chain(mut a), Self::Chain(b)) => {
                a.extend(b);
                Self::Chain(a)
            }
            (Self::Chain(mut a), n) => {
                a.push(n);
                Self::Chain(a)
            }
            (a, n) => Self::Chain(vec![a, n]),
        }
    }
}

/// A short list of asks (kept for areas that return several questions).
pub type Asks = SmallList<Ask, 3>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Window, WindowKind};

    fn w(kind: WindowKind) -> Next {
        Next::Open(Window::Untyped { kind, actor: None, data: [0; 3] })
    }

    #[test]
    fn done_is_the_identity_and_chains_flatten_in_order() {
        let n = Next::Done.then(w(WindowKind::Tokens)).then(Next::Done).then(w(WindowKind::Trade));
        assert!(!n.is_done());
        let flat = n.flatten();
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0], w(WindowKind::Tokens));
        assert_eq!(flat[1], w(WindowKind::Trade));
        assert!(Next::Chain(vec![Next::Done, Next::Chain(vec![])]).is_done());
    }
}
