use ti4_engine_core::state::{CoreState, Rng as CoreRng, RngDomain, Slice, Texts};
use ti4_model::state::GameState;

use super::{Sources, SliceCodec};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};
use crate::wire::rng::RngPositions;

const S: Slice = Slice::Rng;

/// The seed (`GameState.rng_seed`) and, when the caller passes the positions of the game's random
/// source in [`Sources`], the ChaCha8 streams. The streams live in `Game`'s `GameRng`, not in
/// `GameState`: `store` writes the seed back and [`positions`] reads the streams out for the caller
/// to hand to `GameRng::set_positions`.
///
/// The streams are `ti4_chance::Streams` (lane C2): every stream is one of the literal domains and is keyed by the bottom crate
/// itself (`SHA-256(seed_le || text)`). A stream name that is no literal is refused at load (decision D3: old saves with dynamically
/// named streams are not supported).
pub struct Rng;

/// The typed domain of a stream name.
///
/// # Errors
/// A name that is no literal stream name (D3).
pub fn domain_in(_texts: &mut Texts, text: &str) -> R<RngDomain> {
    RngDomain::of_text(text).ok_or_else(|| bad(S, "rng domain", text.to_owned()))
}

/// The stream name of a domain.
///
/// # Errors
/// Never (every domain is a literal); kept fallible for the callers.
pub fn domain_text_in(_texts: &Texts, d: RngDomain) -> R<String> {
    Ok(d.text().to_owned())
}

/// [`domain_in`] on the lineage's staged tables (adapter code that has no core yet).
///
/// # Errors
/// As [`domain_in`].
pub fn domain(cx: &mut Ctx, text: &str) -> R<RngDomain> {
    domain_in(cx.texts.tables_mut(), text)
}

/// [`domain_text_in`] on the lineage's staged tables.
///
/// # Errors
/// As [`domain_text_in`].
pub fn domain_text(cx: &Ctx, d: RngDomain) -> R<String> {
    domain_text_in(cx.texts.tables(), d)
}

/// [`positions_in`] on the lineage's staged tables.
///
/// # Errors
/// As [`positions_in`].
pub fn positions(cx: &Ctx, rng: &CoreRng) -> R<RngPositions> {
    positions_in(cx.texts.tables(), rng)
}

/// The stream positions of the typed generator as the string engine's map.
///
/// # Errors
/// Never; kept fallible for the callers.
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
            rng.set_positions(&typed);
        }
        core.rng = rng;
        Ok(())
    }

    fn store(_: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        gs.rng_seed = core.rng().seed();
        Ok(())
    }
}
