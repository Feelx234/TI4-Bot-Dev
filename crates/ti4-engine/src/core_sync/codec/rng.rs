use ti4_engine_core::state::{CoreState, Rng as CoreRng, RngDomain, SeedSource, Slice, Texts};
use ti4_model::state::GameState;

use super::{Sources, SliceCodec};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::texts::TextCodes;
use crate::core_sync::error::{R, bad};
use crate::rng::{GameRng, RngPositions};

const S: Slice = Slice::Rng;

/// The seed (`GameState.rng_seed`) and, when the caller passes the positions of the game's random
/// source in [`Sources`], the ChaCha8 streams. The streams live in `Game`'s `GameRng`, not in
/// `GameState`: `store` writes the seed back and [`positions`] reads the streams out for the caller
/// to hand to `GameRng::set_positions`.
pub struct Rng;

const LITERALS: [(RngDomain, &str); 22] = [
    (RngDomain::Dice, "dice"),
    (RngDomain::Objectives, "deck:objectives"),
    (RngDomain::Agendas, "deck:agendas"),
    (RngDomain::ActionCards, "deck:action_cards"),
    (RngDomain::Secrets, "deck:secrets"),
    (RngDomain::Relics, "deck:relics"),
    (RngDomain::Galaxy, "galaxy"),
    (RngDomain::Exploration, "deck:exploration"),
    (RngDomain::Map, "map"),
    (RngDomain::Speaker, "setup:speaker"),
    (RngDomain::Fracture, "fracture-breakthrough"),
    (RngDomain::Neuraloop, "neuraloop"),
    (RngDomain::YssarilHero, "yssarilhero"),
    (RngDomain::Direct, "direct"),
    (RngDomain::DeckA, "deck:a"),
    (RngDomain::DeckB, "deck:b"),
    (RngDomain::RetryProbe, "retry probe"),
    (RngDomain::SeatingFactionAssignment, "seating:faction-assignment:v1"),
    (RngDomain::SeatingLobbyDraw, "seating:lobby-faction-draw:v1"),
    (RngDomain::WideRoster, "seating:wide-roster:v1"),
    (RngDomain::KeleresVariant, "seating:keleres-variant:v1"),
    (RngDomain::YinAscendant, "yinbt:unused_commander"),
];

/// The typed domain of a stream name; a name outside the literals is `Dyn(code, 0)` with the name
/// interned.
pub fn domain_in(texts: &mut Texts, text: &str) -> R<RngDomain> {
    if let Some((d, _)) = LITERALS.iter().find(|(_, t)| *t == text) {
        return Ok(*d);
    }
    Ok(RngDomain::Dyn(texts.intern_code(S, text)?.0, 0))
}

pub fn domain_text_in(texts: &Texts, d: RngDomain) -> R<String> {
    if let Some((_, t)) = LITERALS.iter().find(|(x, _)| *x == d) {
        return Ok((*t).to_owned());
    }
    match d {
        RngDomain::Dyn(code, 0) => Ok(texts.code_text(S, ti4_engine_core::state::Code(code))?.to_owned()),
        other => Err(bad(S, "rng domain", format!("{other:?}"))),
    }
}

/// [`domain_in`] on the lineage's staged tables (adapter code that has no core yet).
pub fn domain(cx: &mut Ctx, text: &str) -> R<RngDomain> {
    domain_in(cx.texts.tables_mut(), text)
}

/// [`domain_text_in`] on the lineage's staged tables.
pub fn domain_text(cx: &Ctx, d: RngDomain) -> R<String> {
    domain_text_in(cx.texts.tables(), d)
}

/// [`positions_in`] on the lineage's staged tables.
///
/// # Errors
/// A domain without a text.
pub fn positions(cx: &Ctx, rng: &CoreRng) -> R<RngPositions> {
    positions_in(cx.texts.tables(), rng)
}

/// `SHA-256(seed_le || domain text)`, the stream seeding of `GameRng`. Holds a **clone** of the state's
/// code table taken before the call (the core cannot mint during a call): a `Dyn` domain minted later
/// has no text here, which would silently seed the stream wrong, so a missing text is a debug failure.
/// Production rules mint no `Dyn` domain (all 22 are literals).
pub struct TextSeeds(pub Texts);

impl TextSeeds {
    /// A clone of the staged tables of `cx`.
    #[must_use]
    pub fn of(cx: &Ctx) -> Self {
        Self(cx.texts.tables().clone())
    }
}

impl SeedSource for TextSeeds {
    fn seed_for(&self, seed: u64, domain: RngDomain) -> [u8; 32] {
        let text = domain_text_in(&self.0, domain);
        debug_assert!(text.is_ok(), "rng domain {domain:?} has no text in the code table the stream is seeded from");
        GameRng::derive_seed(seed, &text.unwrap_or_default())
    }
}

/// The stream positions of the typed generator as the string engine's map.
///
/// # Errors
/// A domain without a text.
pub fn positions_in(texts: &Texts, rng: &CoreRng) -> R<RngPositions> {
    rng.positions().map(|(d, p)| Ok((domain_text_in(texts, d)?, p))).collect()
}

impl SliceCodec for Rng {
    const SLICE: Slice = S;

    fn load(_: &mut Ctx, gs: &GameState, src: Sources<'_>, core: &mut CoreState) -> R<()> {
        let mut rng = CoreRng::new(gs.rng_seed);
        if let Some(pos) = src.rng {
            let typed: Vec<(RngDomain, u128)> =
                pos.iter().map(|(t, p)| Ok((domain_in(&mut core.texts, t)?, *p))).collect::<R<_>>()?;
            rng.set_positions(&typed, &TextSeeds(core.texts.clone()));
        }
        core.rng = rng;
        Ok(())
    }

    fn store(_: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        gs.rng_seed = core.rng().seed();
        Ok(())
    }
}
