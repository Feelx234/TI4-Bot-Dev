//! From `exploring_dual.rs` of the legacy crate: the words of a log row of an exploration and the event texts, as the core window needs
//! them (copies; the lockstep half stays in the legacy crate).

use ti4_engine_core::generated::{Explore, ExploreType, Wormhole};
use ti4_engine_core::kernel::ability::ExploreCodes;
use ti4_engine_core::kernel::fx_explore::{RowIn, RowOut};
use ti4_engine_core::state::{CoreState, EHow};
use ti4_model::id::PlanetId;

use super::exploration::{Explored, Holdings, row_words};
use crate::core_sync::conv::explore_key;
use crate::generated::IdStr;
use crate::render::Render;

/// The words of the log row of a card: the string's `row_words` over what the core window read, minted in the position's text table.
#[must_use]
pub fn words(core: &mut CoreState, r: &RowIn) -> Option<RowOut> {
    let planet = r.planet.map(|p| PlanetId::new(IdStr::as_str(p)));
    let card = IdStr::as_str(r.card).to_owned();
    let outcome = match r.how {
        EHow::Fragment(deck) => Explored::Fragment { trait_name: explore_key(deck) },
        EHow::Attached => Explored::Attached { card },
        EHow::Resolved => Explored::Resolved { card },
        EHow::Unresolved => Explored::Unresolved { card },
        EHow::Discarded => Explored::Discarded { card },
    };
    let mut tokens = Vec::new();
    if let Some(system) = r.gamma {
        tokens.push(format!("wormhole:GAMMA:{}", IdStr::as_str(system)));
    }
    if let Some((system, face)) = r.ion {
        tokens.push(format!("ion_storm:{}:{}", IdStr::as_str(system), if face == Wormhole::Alpha { "ALPHA" } else { "BETA" }));
    }
    let kept = r.kept.map(IdStr::as_str);
    let attached = r.attached.map(IdStr::as_str);
    let w = row_words(&Holdings::from_core(r.before), &Holdings::from_core(r.after), planet.as_ref(), &outcome, tokens, kept, attached);
    let mut mint = |text: &str| -> Option<ti4_engine_core::state::Code> {
        core.texts.codes.intern(text.as_bytes(), ti4_engine_core::state::CODE_CAPACITY).and_then(|i| u16::try_from(i).ok()).map(ti4_engine_core::state::Code)
    };
    let outcome = mint(&w.text)?;
    // The string writes an empty `result` as the empty text and the core keeps `None` for it.
    let result = if w.result.is_empty() { None } else { Some(mint(&w.result)?) };
    let gains = w.gains.iter().map(|(key, n)| Some((mint(key)?, *n))).collect::<Option<Vec<_>>>()?;
    let placed = w.placed.iter().map(|text| mint(text)).collect::<Option<Vec<_>>>()?;
    Some(RowOut { outcome, result, gains, placed })
}

/// The event texts of an exploration, in the render table.
#[must_use]
pub fn codes(r: &Render) -> ExploreCodes {
    let mut codes = ExploreCodes { filled: true, goods: r.code("exploration"), trade_agreement: r.code("trade_agreement"), ..ExploreCodes::default() };
    for deck in ExploreType::ALL {
        codes.decks[deck as usize] = r.code(&explore_key(deck));
    }
    for card in Explore::ALL {
        codes.cards[card as usize] = r.code(IdStr::as_str(card));
    }
    codes.freelancers = r.code("Freelancers");
    codes
}

