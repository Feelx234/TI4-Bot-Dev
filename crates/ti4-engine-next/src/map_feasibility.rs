//! Objectives a particular map can never satisfy.
//!
//! **Project setup rule (not a printed rule).** Under the printed rules a revealed or dealt
//! objective that nobody can score simply stays in the game. This project decides otherwise: an
//! unwinnable draw spoils a game, and the ordinary board generators make that likely. The seeded
//! filler pool ([`crate::seating::neutral_systems`]) deliberately holds only plain planet systems,
//! so a generated map has no empty system, no wormhole, no anomaly, and no gamma source unless a
//! template places such a tile by hand. Explore Deep Space, Become the Gatekeeper and a handful of
//! others are then not merely hard but impossible.
//!
//! [`unsatisfiable_public`] and [`unsatisfiable_secret`] list the objectives that the *map alone*
//! rules out, judged generously: every bound below is an upper bound on what any one player could
//! ever reach, so an objective is listed only when even a perfect game cannot score it.
//! [`apply`] then removes those cards from the decks at setup. Nothing else about an objective
//! changes, a map that satisfies everything is left byte-for-byte untouched, and the result is a
//! function of the seed and the map only.
//!
//! Deliberately **not** judged here (they depend on play, not on the map): exploration-dependent
//! cards beyond the traits check, factions, technologies, combats, relics, laws, and reaching the
//! Wormhole Nexus through a gamma token (Gamma Relay, a gamma frontier card, Creuss), which is why
//! Defy Space and Time stays in. Become the Gatekeeper ignores that same long detour: it asks for
//! an alpha and a beta *on the map*.

use std::collections::BTreeSet;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{ObjectiveId, SystemId};
use ti4_model::state::GameState;

use crate::deck::{Exclusions, build_starting_decks_excluding};

pub use ti4_engine_core_setup::feasibility::{MapCensus, census, unsatisfiable_public, unsatisfiable_secret};

/// Remove from the decks every objective the map rules out. Returns the aliases removed from
/// play as `(public, secret)`, both empty (and the state untouched) when the map rules out nothing.
///
/// Call once at setup, after the board exists and before any card is played. Public objectives are
/// redrawn from the same shuffled pools minus the excluded cards, so the two revealed cards and
/// the rest of the deck change only where a card was excluded. Secret hands keep every card they
/// hold that is still legal; an excluded one is replaced from the top of the deck.
pub fn apply(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &Galaxy,
) -> (Vec<&'static str>, Vec<&'static str>) {
    let homes: Vec<SystemId> = state
        .players
        .iter()
        .filter_map(|seat| seat.home_system.clone())
        .collect();
    let c = census(content, sources, galaxy, &homes);
    let public = unsatisfiable_public(&c);
    let secret = unsatisfiable_secret(&c);
    if public.is_empty() && secret.is_empty() { // mutants:skip (on the corpus every board rules out objectives of both kinds: 1 359 hand-made and 600+ drawn boards in the parity tests)
        return (public, secret);
    }

    let revealed = state.revealed_objectives.len();
    let decks = build_starting_decks_excluding(
        content,
        sources,
        state.rng_seed,
        &Exclusions {
            public: public.iter().map(|a| (*a).to_owned()).collect(),
            secret: secret.iter().map(|a| (*a).to_owned()).collect(),
        },
    );
    let public_set: BTreeSet<&str> = public.iter().copied().collect();
    state.objective_deck = decks.objectives;
    state.revealed_objectives.clear();
    for _ in 0..revealed {
        let _ = state.reveal_objective();
    }
    debug_assert!(
        state
            .revealed_objectives
            .iter()
            .chain(&state.objective_deck)
            .all(|id: &ObjectiveId| !public_set.contains(id.as_str()))
    );

    let secret_set: BTreeSet<&str> = secret.iter().copied().collect();
    state
        .secret_deck
        .retain(|id| !secret_set.contains(id.as_str()));
    for index in 0..state.players.len() {
        for slot in 0..state.players[index].secret_objectives.len() {
            if secret_set.contains(state.players[index].secret_objectives[slot].as_str())
                && !state.secret_deck.is_empty()
            {
                state.players[index].secret_objectives[slot] = state.secret_deck.remove(0);
            }
        }
    }
    (public, secret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::build_starting_decks;
    use crate::setup::start_game_seeded;
    use ti4_content::galaxy::{Galaxy, System, all_systems};
    use ti4_model::content_types::POK;
    use ti4_model::id::{FactionId, PlayerId};

    fn content() -> &'static ContentStore {
        ContentStore::embedded()
    }

    fn seated(n: usize, seed: u64) -> (GameState, Vec<SystemId>) {
        let ids: Vec<PlayerId> = (0..n).map(|i| PlayerId::new(format!("p{i}"))).collect();
        let mut state = start_game_seeded(content(), &ids, POK, None, seed).unwrap();
        let factions = seating::IN_SCOPE_FACTIONS;
        for (i, id) in ids.iter().enumerate() {
            seating::deploy(&mut state, content(), id, &FactionId::new(factions[i]), POK).unwrap();
        }
        let homes = state
            .players
            .iter()
            .filter_map(|p| p.home_system.clone())
            .collect();
        (state, homes)
    }

    use crate::seating;

    /// A plain generated board: filler only, as the seeded generator makes it.
    fn plain_board(state: &GameState, seed: u64) -> Galaxy {
        let by_seat = state
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| (PlayerId::new(format!("seat_{i:02}")), p.faction.clone()))
            .collect();
        let filler = seating::map_filler(content(), 36, POK, seed);
        let refs: Vec<&str> = filler.iter().map(SystemId::as_str).collect();
        seating::build_board(content(), &by_seat, &refs, POK).unwrap()
    }

    /// A board that offers everything: empty systems, anomalies, both wormholes, planets.
    fn rich_board(state: &GameState) -> Galaxy {
        let own_homes: Vec<String> = state
            .players
            .iter()
            .filter_map(|p| p.home_system.as_ref().map(ToString::to_string))
            .collect();
        let systems = all_systems(content(), POK);
        let homes = ti4_content::galaxy::home_systems(content(), POK);
        let pick = |want: &dyn Fn(&System) -> bool, n: usize| -> Vec<&str> {
            systems
                .iter()
                .filter(|(id, s)| {
                    want(s)
                        && !homes.contains(*id)
                        && !s.is_hyperlane()
                        && !crate::seating::is_mecatol(id)
                        && !crate::builders::support::is_fracture_system(
                            content(),
                            POK,
                            &SystemId::new(**id),
                        )
                        && s.id() != seating::LOCKED_NEXUS
                        && s.id() != seating::OPEN_NEXUS
                        && !["17", "51", "117", "118"].contains(id)
                })
                .map(|(id, _)| *id)
                .take(n)
                .collect()
        };
        let mut ids: Vec<&str> = vec!["18"];
        ids.extend(pick(
            &|s| s.planets().is_empty() && !s.is_anomaly() && s.wormholes().is_empty(),
            6,
        ));
        ids.extend(pick(&|s| s.is_anomaly() && !s.planets().is_empty(), 6));
        ids.extend(pick(&|s| s.wormholes().contains("ALPHA"), 1));
        ids.extend(pick(&|s| s.wormholes().contains("BETA"), 1));
        let mut seen = BTreeSet::new();
        ids.retain(|id| seen.insert(*id));
        let mut planets: Vec<&str> = pick(
            &|s| s.planets().len() >= 2 && !s.is_anomaly() && s.wormholes().is_empty(),
            2,
        );
        planets.extend(pick(
            &|s| s.planets().len() == 1 && !s.is_anomaly() && s.wormholes().is_empty(),
            30,
        ));
        let planets: Vec<&str> = planets
            .into_iter()
            .filter(|id| !seen.contains(id))
            .collect();
        ids.extend(planets);
        // Homes go in the first ring after Mecatol's, so each touches a ring-two filler tile.
        let mut placed = vec![ids[0]];
        placed.extend(own_homes.iter().map(String::as_str));
        placed.extend(ids[1..].iter().take(32));
        Galaxy::build(content(), &placed, POK, 3).unwrap()
    }

    fn ten(state: &GameState) -> Vec<String> {
        state
            .revealed_objectives
            .iter()
            .chain(&state.objective_deck)
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn a_generated_board_rules_out_the_objectives_it_has_no_tiles_for() {
        let (state, homes) = seated(4, 11);
        let galaxy = plain_board(&state, 11);
        let c = census(content(), POK, &galaxy, &homes);
        assert_eq!(c.planetless, 0, "the filler pool holds no empty systems");
        assert_eq!((c.alpha, c.beta, c.anomalies), (0, 0, 0));
        let public = unsatisfiable_public(&c);
        let secret = unsatisfiable_secret(&c);
        for alias in ["deep_space", "vast_territories"] {
            assert!(public.contains(&alias), "{alias}");
        }
        for alias in ["btgk", "lsc", "btv"] {
            assert!(secret.contains(&alias), "{alias}");
        }
        // Cards the board can serve are not touched.
        for alias in ["expand_borders", "outer_rim", "intimidate"] {
            assert!(!public.contains(&alias), "{alias}");
        }
        assert!(!secret.contains(&"ose") && !secret.contains(&"te"));
    }

    #[test]
    fn a_board_with_everything_changes_nothing() {
        let (mut state, homes) = seated(4, 11);
        let galaxy = rich_board(&state);
        let c = census(content(), POK, &galaxy, &homes);
        assert!(
            c.planetless >= 5 && c.alpha >= 1 && c.beta >= 1 && c.beside_anomaly >= 3,
            "{c:?}"
        );
        assert_eq!(unsatisfiable_public(&c), Vec::<&str>::new(), "{c:?}");
        assert_eq!(unsatisfiable_secret(&c), Vec::<&str>::new(), "{c:?}");
        let before = state.clone();
        let (p, s) = apply(&mut state, content(), POK, &galaxy);
        assert!(p.is_empty() && s.is_empty());
        assert_eq!(state, before);
    }

    #[test]
    fn setup_deals_the_ruled_out_cards_out_and_keeps_the_rest_in_seeded_order() {
        for n in 3..=6 {
            for seed in 0..12 {
                let (mut state, _) = seated(n, seed);
                let galaxy = plain_board(&state, seed);
                let original = state.clone();
                let (public, secret) = apply(&mut state, content(), POK, &galaxy);
                assert!(!public.is_empty() && !secret.is_empty());

                // Public: still five of each stage, two revealed, none ruled out.
                assert_eq!(ten(&state).len(), 10);
                assert_eq!(state.revealed_objectives.len(), 2);
                assert!(ten(&state).iter().all(|id| !public.contains(&id.as_str())));
                let stage_one = |id: &String| {
                    content()
                        .get(ti4_model::content_types::ContentType::PublicObjectives, id)
                        .and_then(|r| r.int("points"))
                };
                assert!(ten(&state)[..5].iter().all(|id| stage_one(id) == Some(1)));
                assert!(ten(&state)[5..].iter().all(|id| stage_one(id) == Some(2)));
                // Cards that were not ruled out keep their relative order.
                let kept: Vec<String> = ten(&original)
                    .into_iter()
                    .filter(|id| !public.contains(&id.as_str()))
                    .collect();
                let now = ten(&state);
                let survivors: Vec<&str> = now
                    .iter()
                    .filter(|id| kept.contains(id))
                    .map(String::as_str)
                    .collect();
                let expected: Vec<&str> = kept
                    .iter()
                    .filter(|id| now.contains(id))
                    .map(String::as_str)
                    .collect();
                assert_eq!(survivors, expected);

                // Secrets: nobody holds a ruled-out card, the deck has none, hands keep size,
                // and a legal card in hand stays.
                assert!(
                    state
                        .secret_deck
                        .iter()
                        .all(|id| !secret.contains(&id.as_str()))
                );
                for (before, after) in original.players.iter().zip(&state.players) {
                    assert_eq!(
                        before.secret_objectives.len(),
                        after.secret_objectives.len()
                    );
                    assert!(
                        after
                            .secret_objectives
                            .iter()
                            .all(|id| !secret.contains(&id.as_str()))
                    );
                    for card in &before.secret_objectives {
                        if !secret.contains(&card.as_str()) {
                            assert!(after.secret_objectives.contains(card));
                        }
                    }
                }
                let total = state.secret_deck.len()
                    + state
                        .players
                        .iter()
                        .map(|p| p.secret_objectives.len())
                        .sum::<usize>();
                assert_eq!(
                    total,
                    build_starting_decks(content(), POK, seed).secrets.len() - secret.len()
                );

                // Deterministic: the same seed and board give the same decks.
                let (mut again, _) = seated(n, seed);
                let board = plain_board(&again, seed);
                apply(&mut again, content(), POK, &board);
                assert_eq!(again, state);
            }
        }
    }
}
