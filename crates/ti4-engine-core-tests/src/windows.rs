//! Small windows of the kit.

use ti4_engine_core::choice::{Choice, Pick};
use ti4_engine_core::kernel::stage::{Illegal, Sx, Window};
use ti4_engine_core::kernel::{Kernel, Next};
use ti4_engine_core::state::{CoreState, EventRec};

/// A window that emits one event through the resolver and ends: the way to run the reaction windows of the registered abilities around it
/// (`Rig::with_abilities`).
#[derive(Clone)]
pub struct EmitEvent {
    pub rec: EventRec,
    done: bool,
}

impl EmitEvent {
    #[must_use]
    pub const fn new(rec: EventRec) -> Self {
        Self { rec, done: false }
    }
}

impl Window for EmitEvent {
    type Stage = bool;

    fn stage(&self) -> bool {
        self.done
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if !self.done {
            self.done = true;
            let _ = sx.emit(self.rec.clone());
        }
        Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}

// -- the exploration kit ---------------------------------------------------------------------------------------


use ti4_engine_core::kernel::ability::ExploreCodes;
use ti4_engine_core::kernel::fx_explore::{RowIn, RowOut};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::state::Code;

/// The words a log row is closed with in the tests: how the card resolved, the goods around it, the tokens it placed, the card kept
/// and the attachment put on the planet (the adapter's `row_words` is the string's).
#[must_use]
pub fn explore_words(core: &mut ti4_engine_core::state::CoreState, r: &RowIn) -> Option<RowOut> {
    let mut mint = |text: &str| -> Option<Code> { core.texts.codes.intern(text.as_bytes(), ti4_engine_core::state::CODE_CAPACITY).and_then(|i| u16::try_from(i).ok()).map(Code) };
    let outcome = mint(&format!("{:?}", r.how))?;
    let result = mint(&format!("goods {}>{} cmd {}>{} gamma {:?} ion {:?} kept {:?} attached {:?}", r.before.trade_goods, r.after.trade_goods, r.before.command_tokens, r.after.command_tokens, r.gamma, r.ion, r.kept, r.attached))?;
    Some(RowOut { outcome, result: Some(result), gains: Vec::new(), placed: Vec::new() })
}

/// The source `trade_agreement` the kit's table names.
pub const AGREEMENT: Code = Code(9);
/// The source `exploration` of the trade goods an exploration gains.
pub const EXPLORATION_GOODS: Code = Code(5);

/// A table that carries what an exploration needs of the adapter: the event texts and the words of a log row.
#[must_use]
pub fn explore_table() -> AbilityTable {
    let mut table = AbilityTable::default();
    table.facts.explore_words = Some(explore_words);
    let mut codes = ExploreCodes { filled: true, goods: EXPLORATION_GOODS, trade_agreement: AGREEMENT, ..ExploreCodes::default() };
    for (i, slot) in codes.decks.iter_mut().enumerate() {
        *slot = Code(100 + i as u16);
    }
    for (i, slot) in codes.cards.iter_mut().enumerate() {
        *slot = Code(200 + i as u16);
    }
    table.codes.explore = codes;
    table
}

/// The text of a code of the position.
#[must_use]
pub fn text_of(core: &ti4_engine_core::state::CoreState, code: Code) -> String {
    core.texts.codes.get(u32::from(code.0)).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default()
}
