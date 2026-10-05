//! Acquired commander effects, second batch: Empyrean and Firmament.
//!
//! Every effect is gated on `promissory::has_commander_ability` for the benefiting seat, never on
//! faction id, so an own unlocked commander, a faceup Alliance and a direct grant (Yin's
//! breakthrough) all work, and a seat without the ability sees no change.
use crate::timing::{Ability, Relation};
use std::sync::Arc;
use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;

const EMPYREAN: &str = "empyreancommander";
const FIRMAMENT: &str = "firmamentcommander";

/// Whether `owner` has a command token in `system` that a mover other than `owner` just entered.
fn empyrean_ready(state: &GameState, owner: &PlayerId, mover: Option<&str>, system: &str) -> bool {
    mover.is_some_and(|mover| mover != owner.as_str())
        && crate::promissory::has_commander_ability(state, owner, EMPYREAN)
        && state
            .board
            .get(&SystemId::new(system))
            .is_some_and(|board| board.command_tokens.contains(owner))
}

/// Timing abilities for the acquired commanders in this file.
///
/// > Empyrean commander (Suffi An): "After another player moves ships into a system that contains
/// > 1 of your command tokens: You may return that token to your reinforcements."
///
/// The window is `SHIP_MOVED` (payload `player`, `system`); the token is removed from the board
/// only if the holder accepts. Several ships entering the same system open several windows, but
/// the token is gone after the first acceptance, so it can be returned only once.
pub(crate) fn timing_abilities(owner_name: &str, seat: &PlayerId) -> Vec<Ability> {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    vec![
        Ability::stateful(
            format!("leader:{owner_name}:{EMPYREAN}:SHIP_MOVED:after"),
            seat.clone(),
            "SHIP_MOVED",
            Relation::After,
            Arc::new(move |event, _, context| {
                let Some(system) = event.text("system") else {
                    return Ok(());
                };
                if empyrean_ready(context.state, &owner, event.text("player"), system) {
                    context
                        .state
                        .system_mut(&SystemId::new(system))
                        .command_tokens
                        .remove(&owner);
                }
                Ok(())
            }),
        )
        .with_optional(true)
        .with_stateful_condition(Arc::new(move |event, _, context| {
            event.text("system").is_some_and(|system| {
                empyrean_ready(
                    context.state,
                    &condition_owner,
                    event.text("player"),
                    system,
                )
            })
        })),
    ]
}

/// Planets the Firmament commander lets `player` treat as controlled for secret objectives.
///
/// > Firmament commander: "At any time: You can treat planets in systems that contain your ships
/// > as if they were controlled by you for the purpose of scoring secret objectives."
///
/// Empty unless the seat has the ability. The list may include planets the seat really controls;
/// callers must de-duplicate. It is for secret objectives only: public objectives and every other
/// control question must not consult it.
pub(crate) fn firmament_planets(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Vec<PlanetId> {
    if !crate::promissory::has_commander_ability(state, player, FIRMAMENT) {
        return Vec::new();
    }
    let types = ti4_content::units::catalogue(content, sources);
    let mut planets: Vec<PlanetId> = Vec::new();
    for (system, board) in &state.board {
        let has_ship = board.units_of(player).into_iter().any(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(ti4_content::units::UnitType::is_ship)
        });
        if !has_ship {
            continue;
        }
        for planet in ti4_content::galaxy::planets_in(content, system.as_str(), sources) {
            let id = PlanetId::new(planet.id());
            if !planets.contains(&id) {
                planets.push(id);
            }
        }
    }
    planets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::{Scripted, Table};
    use ti4_model::content_types::DEFAULT;

    fn arena() -> GameState {
        crate::fixtures::seated_game(&[("a", "yin"), ("b", "hacan")], DEFAULT)
    }
    fn a() -> PlayerId {
        PlayerId::new("a")
    }
    fn grant(state: &mut GameState, who: &str, commander: &str) {
        assert!(crate::promissory::grant_commander_ability(
            state,
            ContentStore::embedded(),
            &PlayerId::new(who),
            commander
        ));
    }
    fn target() -> (SystemId, PlanetId) {
        crate::fixtures::a_placed_planet()
    }
    fn moved(state: &mut GameState, mover: &str, system: &SystemId, answer: &str) {
        let mut resolver = crate::fixtures::armed_resolver(state);
        let mut table = Table::with_default(Box::new(Scripted::new([answer])));
        crate::fixtures::with_context(state, DEFAULT, None, &mut table, |ctx| {
            let event = ctx
                .event_sequence
                .next(
                    "SHIP_MOVED",
                    [
                        ("player".to_owned(), mover.into()),
                        ("system".to_owned(), system.as_str().into()),
                    ]
                    .into(),
                )
                .unwrap();
            resolver.emit_with_context(ctx, event, |_, _| {}).unwrap();
        });
    }
    const OFFER: &str = "leader:yin:empyreancommander:SHIP_MOVED:after";

    #[test]
    fn empyrean_returns_the_token_when_another_player_moves_in() {
        let mut state = arena();
        let (system, _) = target();
        grant(&mut state, "a", EMPYREAN);
        state.system_mut(&system).command_tokens.insert(a());
        moved(&mut state, "b", &system, OFFER);
        assert!(!state.system_state(&system).command_tokens.contains(&a()));
    }

    #[test]
    fn empyrean_is_optional() {
        let mut state = arena();
        let (system, _) = target();
        grant(&mut state, "a", EMPYREAN);
        state.system_mut(&system).command_tokens.insert(a());
        let before = state.clone();
        moved(&mut state, "b", &system, "decline");
        assert_eq!(state, before);
    }

    #[test]
    fn empyrean_ignores_own_moves_missing_tokens_and_the_unentitled() {
        let (system, _) = target();
        // Own ships entering: not "another player".
        let mut state = arena();
        grant(&mut state, "a", EMPYREAN);
        state.system_mut(&system).command_tokens.insert(a());
        let before = state.clone();
        moved(&mut state, "a", &system, OFFER);
        assert_eq!(state, before);
        // No token of the holder in the system.
        let mut state = arena();
        grant(&mut state, "a", EMPYREAN);
        let before = state.clone();
        moved(&mut state, "b", &system, OFFER);
        assert_eq!(state, before);
        // No ability: nothing is offered even with a token present.
        let mut state = arena();
        state.system_mut(&system).command_tokens.insert(a());
        let before = state.clone();
        moved(&mut state, "b", &system, OFFER);
        assert_eq!(state, before);
    }

    #[test]
    fn empyrean_leaves_other_players_tokens_alone() {
        let mut state = arena();
        let (system, _) = target();
        grant(&mut state, "a", EMPYREAN);
        state.system_mut(&system).command_tokens.insert(a());
        state
            .system_mut(&system)
            .command_tokens
            .insert(PlayerId::new("b"));
        moved(&mut state, "b", &system, OFFER);
        assert!(
            state
                .system_state(&system)
                .command_tokens
                .contains(&PlayerId::new("b"))
        );
    }

    #[test]
    fn firmament_counts_every_planet_where_the_holder_has_ships() {
        let mut state = arena();
        let (system, planet) = target();
        grant(&mut state, "a", FIRMAMENT);
        crate::fixtures::put(&mut state, &system, "carrier", &a(), 1);
        let planets = firmament_planets(&state, ContentStore::embedded(), DEFAULT, &a());
        assert!(planets.contains(&planet));
        for found in
            ti4_content::galaxy::planets_in(ContentStore::embedded(), system.as_str(), DEFAULT)
        {
            assert!(planets.contains(&PlanetId::new(found.id())));
        }
    }

    #[test]
    fn firmament_needs_the_ability_and_a_ship_of_the_holder() {
        let mut state = arena();
        let (system, planet) = target();
        state.system_mut(&system).units.clear();
        crate::fixtures::put(&mut state, &system, "carrier", &a(), 1);
        // Ships but no ability.
        let none = firmament_planets(&state, ContentStore::embedded(), DEFAULT, &a());
        assert!(!none.contains(&planet));
        // Ability but the ship belongs to someone else.
        grant(&mut state, "a", FIRMAMENT);
        let bystander = PlayerId::new("b");
        assert!(
            !firmament_planets(&state, ContentStore::embedded(), DEFAULT, &bystander)
                .contains(&planet)
        );
        // Ground forces alone are not ships.
        let mut ground = arena();
        grant(&mut ground, "a", FIRMAMENT);
        ground.system_mut(&system).units.clear();
        crate::fixtures::put_on_planet(&mut ground, &system, &planet, "infantry", &a(), 1);
        assert!(
            !firmament_planets(&ground, ContentStore::embedded(), DEFAULT, &a()).contains(&planet)
        );
    }
}
