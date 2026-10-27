use ti4_engine_core::state::{CoreState, Rng as CoreRng, RngDomain, SeedSource, Slice};
use ti4_model::state::GameState;

use super::{Sources, SliceCodec};
use crate::core_sync::ctx::Ctx;
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
pub fn domain(cx: &mut Ctx, text: &str) -> R<RngDomain> {
    if let Some((d, _)) = LITERALS.iter().find(|(_, t)| *t == text) {
        return Ok(*d);
    }
    Ok(RngDomain::Dyn(cx.texts.intern(S, text)?.0, 0))
}

pub fn domain_text(cx: &Ctx, d: RngDomain) -> R<String> {
    if let Some((_, t)) = LITERALS.iter().find(|(x, _)| *x == d) {
        return Ok((*t).to_owned());
    }
    match d {
        RngDomain::Dyn(code, 0) => Ok(cx.texts.text(S, ti4_engine_core::state::Code(code))?.to_owned()),
        other => Err(bad(S, "rng domain", format!("{other:?}"))),
    }
}

/// `SHA-256(seed_le || domain text)`, the stream seeding of `GameRng`.
pub struct TextSeeds<'a>(pub &'a Ctx);

impl SeedSource for TextSeeds<'_> {
    fn seed_for(&self, seed: u64, domain: RngDomain) -> [u8; 32] {
        let text = domain_text(self.0, domain).unwrap_or_default();
        GameRng::derive_seed(seed, &text)
    }
}

/// The stream positions of the typed generator as the string engine's map.
///
/// # Errors
/// A domain without a text.
pub fn positions(cx: &Ctx, rng: &CoreRng) -> R<RngPositions> {
    rng.positions().map(|(d, p)| Ok((domain_text(cx, d)?, p))).collect()
}

impl SliceCodec for Rng {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, src: Sources<'_>, core: &mut CoreState) -> R<()> {
        let mut rng = CoreRng::new(gs.rng_seed);
        if let Some(pos) = src.rng {
            let typed: Vec<(RngDomain, u128)> =
                pos.iter().map(|(t, p)| Ok((domain(cx, t)?, *p))).collect::<R<_>>()?;
            rng.set_positions(&typed, &TextSeeds(cx));
        }
        core.rng = rng;
        Ok(())
    }

    fn store(_: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        gs.rng_seed = core.rng().seed();
        Ok(())
    }
}
