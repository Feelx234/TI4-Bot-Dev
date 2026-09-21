//! Faction technologies and faction unit abilities whose effect happens at a fixed point of the turn.
//!
//! Each is called from the one place in the engine where its printed timing happens, the same way
//! Minister of Peace and the Dominus Orb are, rather than through a registry: a faction technology is
//! owned by one seat and read by one call site.

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, SystemId, TechnologyId};
use ti4_model::state::{GameState, TokenPool};

use crate::choice::{Choice, ChoiceOption, Observed, Table};
use crate::decision_context::{DecisionContext, DecisionSource};

/// Whether `player` holds `alias` and it is not exhausted.
fn ready(state: &GameState, player: &PlayerId, alias: &str) -> bool {
    let tech = TechnologyId::new(alias);
    state.player(player).is_some_and(|seat| {
        seat.technologies.contains(&tech) && !seat.exhausted_technologies.contains(&tech)
    })
}

/// The other seats that have at least one ship in `system`, in seating order.
fn others_with_ships(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    active: &PlayerId,
) -> Vec<PlayerId> {
    state
        .players
        .iter()
        .map(|seat| seat.id.clone())
        .filter(|seat| seat != active)
        .filter(|seat| !crate::combat::ships_of(state, content, sources, seat, system).is_empty())
        .collect()
}

/// E-Res Siphons (Jol-Nar): "After another player activates a system that contains 1 or more of
/// your ships, gain 4 trade goods." Not optional and not exhausted. Returns who gained.
pub fn e_res_siphons(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    active: &PlayerId,
) -> Vec<PlayerId> {
    let gained: Vec<PlayerId> = others_with_ships(state, content, sources, system, active)
        .into_iter()
        .filter(|seat| {
            state
                .player(seat)
                .is_some_and(|holder| holder.technologies.contains(&TechnologyId::new("ers")))
        })
        .collect();
    for seat in &gained {
        if let Some(holder) = state.player_mut(seat) {
            holder.trade_goods += 4;
        }
    }
    gained
}

/// Nullification Field (Xxcha): "After another player activates a system that contains 1 or more
/// of your ships, you may exhaust this card and spend 1 token from your strategy pool; immediately
/// end that player's turn." Asks each eligible holder in seating order; returns the one who used it.
pub fn offer_nullification_field(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut Table,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    system: &SystemId,
    active: &PlayerId,
) -> Option<PlayerId> {
    let holders: Vec<PlayerId> = others_with_ships(state, content, sources, system, active)
        .into_iter()
        .filter(|seat| ready(state, seat, "nf"))
        .filter(|seat| {
            state
                .player(seat)
                .is_some_and(|holder| holder.strategic_tokens > 0)
        })
        .collect();
    for holder in holders {
        let choice = Choice::new(
            holder.clone(),
            format!(
                "Nullification Field: exhaust and spend a strategy token to end {active}'s turn"
            ),
            vec![
                ChoiceOption::labelled("use".to_owned(), "technology", "end their turn".to_owned()),
                ChoiceOption::decline(),
            ],
        )
        .contextualized(DecisionContext::new(
            holder.clone(),
            DecisionSource::Content("nf".to_owned()),
            "nullification_field_end_turn",
            state.phase,
            state.round,
        ));
        let Ok(answer) = table.ask_seeing(&choice, &Observed::new(state, content, sources, galaxy))
        else {
            continue;
        };
        if answer.is_decline() {
            continue;
        }
        let Some(seat) = state.player_mut(&holder) else {
            continue;
        };
        if !seat.spend_token(TokenPool::Strategic) {
            continue;
        }
        seat.exhausted_technologies.insert(TechnologyId::new("nf"));
        return Some(holder);
    }
    None
}

/// Genesis, the Sol flagship: "At the end of the status phase, place 1 infantry from your
/// reinforcements in this system's space area." One infantry per flagship, of the type the owner
/// currently builds (Spec Ops once upgraded), and only while the box has one left. Returns the
/// systems that got one.
pub fn genesis(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
) -> Vec<(PlayerId, SystemId)> {
    let flagships: Vec<(PlayerId, SystemId)> = state
        .board
        .iter()
        .flat_map(|(system, here)| {
            here.units
                .iter()
                .filter(|unit| unit.type_id.as_str() == "sol_flagship")
                .map(|unit| (unit.owner.clone(), system.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    let mut placed = Vec::new();
    for (owner, system) in flagships {
        let Some(infantry) = crate::production::buildable_for(state, content, sources, &owner)
            .into_iter()
            .find(|kind| {
                ti4_content::units::catalogue(content, sources)
                    .get(kind.as_str())
                    .is_some_and(|record| record.base_type() == "infantry")
            })
        else {
            continue;
        };
        let infantry = ti4_model::id::UnitTypeId::new(infantry);
        if crate::supply::remaining(state, content, sources, &owner, &infantry) < 1 {
            continue;
        }
        state
            .system_mut(&system)
            .units
            .push(ti4_model::units::Unit::new(infantry, owner.clone()));
        placed.push((owner, system));
    }
    placed
}

#[cfg(test)]
mod tests {
    use ti4_model::content_types::POK;

    use super::*;
    use crate::fixtures::{game, put};

    fn setup(tech: &str) -> (GameState, SystemId, PlayerId, PlayerId) {
        let mut state = game(&["a", "b"]);
        let system = SystemId::new("19");
        let (active, holder) = (PlayerId::new("a"), PlayerId::new("b"));
        put(&mut state, &system, "cruiser", &holder, 1);
        let seat = state.player_mut(&holder).unwrap();
        seat.technologies.insert(TechnologyId::new(tech));
        seat.strategic_tokens = 2;
        (state, system, active, holder)
    }

    #[test]
    fn e_res_siphons_pays_four_when_another_player_activates_your_ships_system() {
        let (mut state, system, active, holder) = setup("ers");
        let before = state.player(&holder).unwrap().trade_goods;
        let gained = e_res_siphons(&mut state, ContentStore::embedded(), POK, &system, &active);
        assert_eq!(gained, vec![holder.clone()]);
        assert_eq!(state.player(&holder).unwrap().trade_goods, before + 4);

        // Its own activation pays nothing, and neither does a system without its ships.
        let mut again = state.clone();
        assert!(
            e_res_siphons(&mut again, ContentStore::embedded(), POK, &system, &holder).is_empty()
        );
        assert!(
            e_res_siphons(
                &mut again,
                ContentStore::embedded(),
                POK,
                &SystemId::new("20"),
                &active
            )
            .is_empty()
        );
    }

    #[test]
    fn nullification_field_costs_a_token_and_the_exhausted_card() {
        let (mut state, system, active, holder) = setup("nf");
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        let used = offer_nullification_field(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            None,
            &system,
            &active,
        );
        assert_eq!(used, Some(holder.clone()));
        let seat = state.player(&holder).unwrap();
        assert_eq!(seat.strategic_tokens, 1);
        assert!(
            seat.exhausted_technologies
                .contains(&TechnologyId::new("nf"))
        );

        // Exhausted, it is not offered again.
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        assert_eq!(
            offer_nullification_field(
                &mut state,
                ContentStore::embedded(),
                POK,
                &mut table,
                None,
                &system,
                &active,
            ),
            None
        );
    }

    #[test]
    fn genesis_places_one_infantry_beside_each_sol_flagship() {
        let mut state = game(&["a"]);
        let sol = PlayerId::new("a");
        state.player_mut(&sol).unwrap().faction = ti4_model::id::FactionId::new("sol");
        let system = SystemId::new("19");
        put(&mut state, &system, "sol_flagship", &sol, 1);
        let placed = genesis(&mut state, ContentStore::embedded(), POK);
        assert_eq!(placed, vec![(sol.clone(), system.clone())]);
        let infantry: Vec<String> = state
            .system_state(&system)
            .units
            .iter()
            .filter(|unit| unit.type_id.as_str() != "sol_flagship")
            .map(|unit| unit.type_id.to_string())
            .collect();
        assert_eq!(
            infantry,
            vec!["sol_infantry".to_owned()],
            "Sol builds Spec Ops"
        );
    }
}
