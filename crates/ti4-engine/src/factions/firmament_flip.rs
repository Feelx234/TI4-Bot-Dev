//! Puppets of the Blade and the transition from The Firmament to The Obsidian (part A of the
//! two-sided faction; see `plans/evidence/BF-firmament.md`).
//!
//! > Puppets of the Blade: "If you have at least 1 plot card in your play area, gain the following
//! > ability: ACTION: Purge The Firmament's faction sheet, leaders, planet cards, and promissory
//! > note. Then, gain all of the faction components for The Obsidian."
//! >
//! > The Blade's Orchestra (Obsidian): "When this faction comes into play: flip your home system,
//! > double-sided faction components, and all of your in-play plot cards. Then, ready Cronos Hollow
//! > and Tallin Hollow if you control them."
//!
//! [`become_obsidian`] is the one typed, atomic transition. It does, in order:
//!
//! | Component | What happens |
//! |---|---|
//! | Faction | the seat's faction becomes `obsidian` |
//! | Home system | tile `96a` is replaced by `96b` on the map (`movement::apply_map_edit`, recorded in the state so the game's map follows); units and command tokens stay; each planet's control, ground forces, coexistence, attachments and exhaustion move to its flipped planet (`cronos` to `cronoshollow`, `tallin` to `tallinhollow`, by position in the faction sheets); the Hollows the seat controls are then readied |
//! | Leaders | the Firmament's three leaders are purged (removed); the Obsidian's three are dealt as a new faction deals them (agent readied, commander and hero locked) |
//! | Promissory notes | Black Ops is purged wherever it is; every other note id that carried the faction name `firmament` (the seat's generic notes, wherever they now are) carries `obsidian`; the Obsidian's own note is put in the seat's hand |
//! | Units | `firmament_flagship` and `firmament_mech` on the board become `obsidian_flagship` and `obsidian_mech`, damage kept |
//! | Technologies | each owned Firmament faction technology becomes its Obsidian side (by position in the faction sheets), exhaustion kept; unowned ones are not gained |
//! | Breakthrough | The Sowing becomes The Reaping; the trade goods on the card stay (`firmament::goods_on_card`) |
//! | Plots | every plot card is flipped faceup, so everyone reads it |
//! | Bookkeeping | the "gains seen" marks follow, so the swap is never announced as gaining a technology or a breakthrough; borrowed Firmament commander abilities are dropped with the purged card |
//!
//! It then stages `FACTION_FLIPPED` (`player`, `from`, `to`) for the game to announce after the
//! action, which is where part B hangs what the Obsidian's cards say about coming into play (the
//! Viper Hollow's "if this unit was coexisting when this card flipped"). Part B adds the Obsidian's
//! abilities, technologies, units, leaders, note and breakthrough to a module of its own; nothing
//! here changes for it.

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{
    BreakthroughId, FactionId, PlanetId, PlayerId, SystemId, TechnologyId, UnitTypeId,
};
use ti4_model::state::GameState;

use super::firmament::{self, FACTION, OBSIDIAN};
use crate::choice::ChoiceOption;
use crate::movement::{MapEdit, MapEditError};
use crate::timing::TimingContext;

/// The component action's option id.
pub const BECOME: &str = "faction|firmament|become_obsidian";

/// Why the transition was refused. Nothing has changed when it is.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FlipError {
    /// The seat is not playing the Firmament.
    #[error("{0} does not play the Firmament")]
    NotFirmament(PlayerId),
    /// Puppets of the Blade needs a plot card in play.
    #[error("{0} has no plot card in play")]
    NoPlot(PlayerId),
    /// The corpus lacks one of the two faction sheets, or they do not pair up.
    #[error("the Firmament and Obsidian sheets do not describe a flip")]
    Sheets,
    /// The seat's home system is not the Firmament's tile.
    #[error("{0}'s home system is not the Firmament's")]
    HomeSystem(PlayerId),
    /// The map refused the tile replacement.
    #[error(transparent)]
    Map(#[from] MapEditError),
}

/// The Firmament-to-Obsidian pairing read from the two faction sheets.
struct Pairing {
    old_home: SystemId,
    new_home: SystemId,
    planets: Vec<(PlanetId, PlanetId)>,
    technologies: Vec<(TechnologyId, TechnologyId)>,
    new_notes: Vec<String>,
}

fn pairing(content: &ContentStore) -> Result<Pairing, FlipError> {
    let old = ti4_content::factions::get(content, FACTION).ok_or(FlipError::Sheets)?;
    let new = ti4_content::factions::get(content, OBSIDIAN).ok_or(FlipError::Sheets)?;
    let (old_planets, new_planets) = (old.home_planets(), new.home_planets());
    let (old_tech, new_tech) = (old.faction_tech(), new.faction_tech());
    if old_planets.len() != new_planets.len() || old_tech.len() != new_tech.len() {
        return Err(FlipError::Sheets);
    }
    Ok(Pairing {
        old_home: SystemId::new(old.home_system().ok_or(FlipError::Sheets)?),
        new_home: SystemId::new(new.home_system().ok_or(FlipError::Sheets)?),
        planets: old_planets
            .into_iter()
            .zip(new_planets)
            .map(|(from, to)| (PlanetId::new(from), PlanetId::new(to)))
            .collect(),
        technologies: old_tech
            .into_iter()
            .zip(new_tech)
            .map(|(from, to)| (TechnologyId::new(from), TechnologyId::new(to)))
            .collect(),
        new_notes: new
            .promissory_notes()
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
    })
}

/// Whether `player` may take the action: the Firmament with a plot card in play.
#[must_use]
pub fn can_become_obsidian(state: &GameState, player: &PlayerId) -> bool {
    firmament::is_firmament(state, player) && firmament::has_plot(state, player)
}

/// Purge The Firmament's components and gain The Obsidian's. See the module documentation.
///
/// `galaxy` is the game's map; the replacement tile is recorded in `state` (`faction_marks`) and the
/// game's own step brings its map up to it. Atomic: on error `state` is untouched.
///
/// # Errors
/// [`FlipError`] when the seat cannot take the action or the map cannot take the new tile.
pub fn become_obsidian(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &Galaxy,
    player: &PlayerId,
) -> Result<(), FlipError> {
    if !firmament::is_firmament(state, player) {
        return Err(FlipError::NotFirmament(player.clone()));
    }
    if !firmament::has_plot(state, player) {
        return Err(FlipError::NoPlot(player.clone()));
    }
    let pair = pairing(content)?;
    if state
        .player(player)
        .and_then(|seat| seat.home_system.clone())
        != Some(pair.old_home.clone())
    {
        return Err(FlipError::HomeSystem(player.clone()));
    }
    let mut next = state.clone();

    // The home system tile. `apply_map_edit` carries the ships and command tokens; the planets'
    // contents are carried here, planet by planet.
    let old_board = next.board.get(&pair.old_home).cloned();
    let mut trial_galaxy = galaxy.clone();
    crate::movement::apply_map_edit(
        &mut next,
        &mut trial_galaxy,
        content,
        sources,
        &MapEdit::Replace {
            old: pair.old_home.to_string(),
            new: pair.new_home.to_string(),
        },
    )?;
    if let Some(old) = old_board {
        let system = next.system_mut(&pair.new_home);
        for (from, to) in &pair.planets {
            if let Some(holder) = old.planet_control.get(from) {
                system.planet_control.insert(to.clone(), holder.clone());
            }
            if let Some(units) = old.planet_units.get(from) {
                system.planet_units.insert(to.clone(), units.clone());
            }
            if let Some(sharing) = old.coexisting.get(from) {
                system.coexisting.insert(to.clone(), sharing.clone());
            }
            if old.purged_planets.contains(from) {
                system.purged_planets.insert(to.clone());
            }
        }
    }
    for (from, to) in &pair.planets {
        if next.exhausted_planets.remove(from) {
            next.exhausted_planets.insert(to.clone());
        }
        if let Some(attachments) = next.planet_attachments.remove(from) {
            next.planet_attachments.insert(to.clone(), attachments);
        }
    }
    // The Blade's Orchestra: "ready Cronos Hollow and Tallin Hollow if you control them".
    for (_, hollow) in &pair.planets {
        if next.system_state(&pair.new_home).planet_control.get(hollow) == Some(player) {
            next.exhausted_planets.remove(hollow);
        }
    }

    // The seat's own record: faction, home, leaders, technologies, breakthrough, plots.
    let old_leaders = crate::leaders::for_faction(content, sources, FACTION);
    let new_leaders = crate::leaders::starting_states(content, sources, OBSIDIAN);
    {
        let Some(seat) = next.player_mut(player) else {
            return Err(FlipError::NotFirmament(player.clone()));
        };
        seat.faction = FactionId::new(OBSIDIAN);
        seat.home_system = Some(pair.new_home.clone());
        seat.home_planets = pair.planets.iter().map(|(_, to)| to.clone()).collect();
        for leader in &old_leaders {
            seat.leaders.remove(leader);
        }
        for (leader, status) in new_leaders {
            seat.leaders.insert(leader, status);
        }
        for (from, to) in &pair.technologies {
            if seat.technologies.remove(from) {
                seat.technologies.insert(to.clone());
            }
            if seat.exhausted_technologies.remove(from) {
                seat.exhausted_technologies.insert(to.clone());
            }
        }
        if seat
            .breakthrough
            .as_ref()
            .is_some_and(|held| held.as_str() == firmament::BREAKTHROUGH)
        {
            seat.breakthrough = Some(BreakthroughId::new(OBSIDIAN_BREAKTHROUGH));
        }
    }
    firmament::flip_plots(&mut next, player);
    rename_units(&mut next, player);
    rename_notes(&mut next, content, player, &pair.new_notes);
    drop_borrowed_commander(&mut next);
    follow_seen_marks(&mut next, player);

    let payload = std::collections::BTreeMap::from([
        ("player".to_owned(), player.to_string().into()),
        ("from".to_owned(), FACTION.into()),
        ("to".to_owned(), OBSIDIAN.into()),
    ]);
    crate::supply::stage_event(&mut next, FLIPPED, &payload);

    *state = next;
    Ok(())
}

/// The Obsidian's breakthrough, The Reaping: the other side of The Sowing.
pub const OBSIDIAN_BREAKTHROUGH: &str = "obsidianbt";
/// The staged event announced after the transition.
pub const FLIPPED: &str = "FACTION_FLIPPED";

/// The Firmament's flagship and mech on the board become the Obsidian's.
fn rename_units(state: &mut GameState, player: &PlayerId) {
    let swap = |unit: &mut ti4_model::units::Unit| {
        if &unit.owner != player {
            return;
        }
        match unit.type_id.as_str() {
            firmament::FLAGSHIP => unit.type_id = UnitTypeId::new("obsidian_flagship"),
            firmament::MECH => unit.type_id = UnitTypeId::new("obsidian_mech"),
            _ => {}
        }
    };
    for board in state.board.values_mut() {
        board.units.iter_mut().for_each(swap);
        board.planet_units.values_mut().flatten().for_each(swap);
    }
}

/// Note ids carry the owner's faction name: the seat's notes move from `firmament` to `obsidian`,
/// Black Ops is purged, and the Obsidian's own note is dealt to the seat.
fn rename_notes(state: &mut GameState, content: &ContentStore, player: &PlayerId, own: &[String]) {
    let suffix = format!(":{FACTION}");
    let renamed = |note: &str| {
        note.strip_suffix(suffix.as_str())
            .map(|alias| format!("{alias}:{OBSIDIAN}"))
    };
    let purged = crate::promissory::note_id(firmament::NOTE, FACTION);
    state.promissory_notes.remove(&purged);
    state.promissory_faceup.remove(&purged);
    let moved: Vec<(String, PlayerId)> = state
        .promissory_notes
        .iter()
        .filter(|(note, _)| note.ends_with(&suffix))
        .map(|(note, holder)| (note.clone(), holder.clone()))
        .collect();
    for (note, holder) in moved {
        state.promissory_notes.remove(&note);
        if let Some(new) = renamed(&note) {
            state.promissory_notes.insert(new, holder);
        }
    }
    let faceup: Vec<String> = state
        .promissory_faceup
        .iter()
        .filter(|note| note.ends_with(&suffix))
        .cloned()
        .collect();
    for note in faceup {
        state.promissory_faceup.remove(&note);
        if let Some(new) = renamed(&note) {
            state.promissory_faceup.insert(new);
        }
    }
    for alias in own {
        crate::promissory::take(
            state,
            content,
            player,
            &crate::promissory::note_id(alias, OBSIDIAN),
        );
    }
}

/// A purged commander card cannot be borrowed any more (Alliance, Yin, Mahact Imperia grants).
fn drop_borrowed_commander(state: &mut GameState) {
    let suffix = format!(":{}", firmament::COMMANDER);
    state.faction_marks.retain(|key, _| {
        !(key.starts_with(crate::promissory::COMMANDER_ABILITY_PREFIX) && key.ends_with(&suffix))
    });
}

/// The game announces `TECHNOLOGY_GAINED` / `BREAKTHROUGH_GAINED` by comparing each seat with what
/// it held at the last step. A flip is no gain: bring those records up to the new cards.
fn follow_seen_marks(state: &mut GameState, player: &PlayerId) {
    let Some(seat) = state.player(player) else {
        return;
    };
    let technologies: Vec<String> = seat.technologies.iter().map(ToString::to_string).collect();
    let breakthrough = seat
        .breakthrough
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    state.faction_marks.insert(
        format!("private:#seen:technologies:{player}"),
        technologies.join(","),
    );
    state
        .faction_marks
        .insert(format!("private:#seen:breakthrough:{player}"), breakthrough);
}

/// Puppets of the Blade, as the component action it is. Needs the map (the home tile is replaced).
pub(crate) fn component_actions(
    state: &GameState,
    _content: &ContentStore,
    _sources: SourceSet,
    _galaxy: &Galaxy,
    player: &PlayerId,
) -> Vec<ChoiceOption> {
    if !can_become_obsidian(state, player) {
        return Vec::new();
    }
    vec![ChoiceOption::labelled(
        BECOME,
        crate::faction_abilities::ACTION_KIND,
        "Puppets of the Blade: purge the Firmament's components and become the Obsidian",
    )]
}

pub(crate) fn perform_component(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    option: &ChoiceOption,
) -> bool {
    if option.id != BECOME {
        return false;
    }
    let Some(galaxy) = context.galaxy else {
        return false;
    };
    become_obsidian(
        context.state,
        context.content,
        context.sources,
        galaxy,
        player,
    )
    .is_ok()
}

#[cfg(test)]
mod tests {
    use ti4_model::content_types::DEFAULT;
    use ti4_model::id::LeaderId;
    use ti4_model::state::LeaderStatus;

    use super::super::crimson::testkit::scripted;
    use super::super::firmament::testkit::*;
    use super::*;

    /// A map with the Firmament's home tile in its ring.
    fn map() -> Galaxy {
        let ids = plain(6);
        let mut ring_ids: Vec<&str> = ids[1..].iter().map(String::as_str).collect();
        ring_ids[0] = "96a";
        ring(&ids[0], &ring_ids)
    }

    fn cronos() -> PlanetId {
        PlanetId::new("cronos")
    }
    fn tallin() -> PlanetId {
        PlanetId::new("tallin")
    }

    /// The Firmament (`a`) with two plots, both factions' cards, units, notes and a spent planet.
    fn loaded() -> GameState {
        let mut state = game();
        let home = home(&state, &a());
        assert_eq!(home.as_str(), "96a");
        firmament::place_plot(&mut state, &a(), &b());
        firmament::place_plot(&mut state, &a(), &c());
        {
            let seat = state.player_mut(&a()).unwrap();
            for tech in [firmament::PLANESPLITTER, firmament::PARASITE] {
                seat.technologies.insert(TechnologyId::new(tech));
            }
            seat.exhausted_technologies
                .insert(TechnologyId::new(firmament::PARASITE));
            seat.breakthrough = Some(BreakthroughId::new(firmament::BREAKTHROUGH));
            seat.leaders
                .insert(LeaderId::new(firmament::AGENT), LeaderStatus::Exhausted);
            seat.leaders
                .insert(LeaderId::new(firmament::COMMANDER), LeaderStatus::Unlocked);
        }
        state
            .faction_marks
            .insert("firmament:sowing:a".to_owned(), "3".to_owned());
        crate::fixtures::put(&mut state, &home, "firmament_flagship", &a(), 1);
        crate::fixtures::put(&mut state, &home, "destroyer", &b(), 1);
        crate::fixtures::put_on_planet(&mut state, &home, &cronos(), "firmament_mech", &a(), 1);
        state.system_mut(&home).command_tokens.insert(a());
        state.exhausted_planets.insert(cronos());
        state
            .planet_attachments
            .insert(tallin(), vec!["attachment".to_owned()]);
        for note in ["ps:firmament", "an:firmament"] {
            state.promissory_notes.insert(note.to_owned(), b());
        }
        state.promissory_faceup.insert("an:firmament".to_owned());
        state
            .promissory_notes
            .insert("blackops:firmament".to_owned(), c());
        state.faction_marks.insert(
            "commander_ability:b:firmamentcommander".to_owned(),
            "granted".to_owned(),
        );
        state
            .faction_marks
            .insert("private:#seen:technologies:a".to_owned(), "x".to_owned());
        state
    }

    fn act(state: &mut GameState, galaxy: &Galaxy, who: &PlayerId) -> bool {
        let Some(option) =
            crate::factions::mapped_component_actions(state, content(), DEFAULT, galaxy, who)
                .into_iter()
                .find(|option| option.id == BECOME)
        else {
            return false;
        };
        let mut table = scripted(&[]);
        crate::fixtures::with_context(state, DEFAULT, Some(galaxy), &mut table, |context| {
            crate::factions::perform_component(context, who, &option)
        })
    }

    #[test]
    fn puppets_of_the_blade_is_offered_to_the_firmament_with_a_plot_in_play_only() {
        let galaxy = map();
        let mut state = game();
        let offered = |state: &GameState, who: &PlayerId| {
            crate::factions::mapped_component_actions(state, content(), DEFAULT, &galaxy, who)
                .iter()
                .any(|option| option.id == BECOME)
        };
        assert!(!offered(&state, &a()), "no plot, no ability");
        firmament::place_plot(&mut state, &a(), &b());
        assert!(offered(&state, &a()));
        assert!(!offered(&state, &b()), "nobody else has the ability");
    }

    #[test]
    fn becoming_the_obsidian_swaps_the_faction_and_every_two_sided_component() {
        let galaxy = map();
        let mut state = loaded();
        let old_home = home(&state, &a());
        assert!(act(&mut state, &galaxy, &a()), "the action resolves");

        // Faction and home.
        let seat = state.player(&a()).unwrap();
        assert_eq!(seat.faction.as_str(), OBSIDIAN);
        assert_eq!(seat.home_system, Some(SystemId::new("96b")));
        assert_eq!(
            seat.home_planets,
            [PlanetId::new("cronoshollow"), PlanetId::new("tallinhollow")]
        );
        assert!(!state.board.contains_key(&old_home), "the tile is flipped");
        let hollow = SystemId::new("96b");
        let board = state.system_state(&hollow);
        assert_eq!(
            board.planet_control.get(&PlanetId::new("cronoshollow")),
            Some(&a())
        );
        assert_eq!(
            board.planet_control.get(&PlanetId::new("tallinhollow")),
            Some(&a())
        );
        assert!(board.planet_control.get(&cronos()).is_none());
        assert!(board.command_tokens.contains(&a()), "command tokens stay");
        assert!(
            board
                .units
                .iter()
                .any(|u| u.type_id.as_str() == "obsidian_flagship"),
            "ships stay, flipped"
        );
        assert!(
            board
                .units
                .iter()
                .any(|u| u.owner == b() && u.type_id.as_str() == "destroyer"),
            "other players' ships stay"
        );
        assert!(
            board
                .on_planet(&PlanetId::new("cronoshollow"))
                .iter()
                .any(|u| u.type_id.as_str() == "obsidian_mech"),
            "ground forces stay on the flipped planet, flipped"
        );
        assert!(
            board
                .on_planet(&PlanetId::new("tallinhollow"))
                .iter()
                .any(|u| u.type_id.as_str().contains("infantry")),
            "the starting infantry moved with its planet"
        );
        assert!(
            !state
                .board
                .values()
                .flat_map(|s| s.units.iter().chain(s.planet_units.values().flatten()))
                .any(|u| u.type_id.as_str().starts_with("firmament")),
            "no Firmament piece is left"
        );
        // The Blade's Orchestra: the Hollows are ready, the attachment followed.
        assert!(
            !state
                .exhausted_planets
                .contains(&PlanetId::new("cronoshollow"))
        );
        assert!(!state.exhausted_planets.contains(&cronos()));
        assert_eq!(
            state.planet_attachments.get(&PlanetId::new("tallinhollow")),
            Some(&vec!["attachment".to_owned()])
        );
        // The map follows through the recorded edit.
        let mut replayed = galaxy.clone();
        crate::movement::replay_map_edits(&state, &mut replayed, content(), DEFAULT).unwrap();
        assert!(replayed.coord_of("96b").is_some() && replayed.coord_of("96a").is_none());

        // Leaders.
        let seat = state.player(&a()).unwrap();
        assert_eq!(seat.leaders.len(), 3);
        for (id, status) in [
            ("obsidianagent", LeaderStatus::Readied),
            ("obsidiancommander", LeaderStatus::Locked),
            ("obsidianhero", LeaderStatus::Locked),
        ] {
            assert_eq!(seat.leaders.get(&LeaderId::new(id)), Some(&status), "{id}");
        }
        // Technologies keep their exhaustion; the breakthrough keeps its goods.
        assert!(
            seat.technologies
                .contains(&TechnologyId::new("planesplitter-obs"))
        );
        assert!(
            seat.technologies
                .contains(&TechnologyId::new("parasite-obs"))
        );
        assert_eq!(seat.technologies.len(), 2);
        assert_eq!(
            seat.exhausted_technologies,
            std::collections::BTreeSet::from([TechnologyId::new("parasite-obs")])
        );
        assert_eq!(
            seat.breakthrough.as_ref().map(BreakthroughId::as_str),
            Some("obsidianbt")
        );
        assert_eq!(firmament::goods_on_card(&state, &a()), 3);
        // Plots flip faceup and are public.
        assert_eq!(seat.plots, ["u:b", "u:c"]);
        assert_eq!(
            ti4_model::view::view_for(&state, &b())
                .player(&a())
                .unwrap()
                .plots,
            ["u:b", "u:c"]
        );
    }

    #[test]
    fn the_notes_follow_the_factions_name_and_black_ops_is_purged() {
        let galaxy = map();
        let mut state = loaded();
        assert!(act(&mut state, &galaxy, &a()));
        assert_eq!(state.promissory_notes.get("ps:obsidian"), Some(&b()));
        assert_eq!(state.promissory_notes.get("an:obsidian"), Some(&b()));
        assert!(state.promissory_faceup.contains("an:obsidian"));
        assert!(
            state
                .promissory_notes
                .keys()
                .all(|note| !note.ends_with(":firmament")),
            "{:?}",
            state.promissory_notes
        );
        assert!(!state.promissory_notes.contains_key("blackops:obsidian"));
        assert_eq!(
            state.promissory_notes.get("malevolency:obsidian"),
            Some(&a()),
            "the Obsidian's own note is dealt"
        );
        assert!(
            !state
                .faction_marks
                .contains_key("commander_ability:b:firmamentcommander"),
            "a borrowed Firmament commander goes with the purged card"
        );
    }

    #[test]
    fn the_flip_is_not_a_gain_and_stages_the_event_part_b_listens_for() {
        let galaxy = map();
        let mut state = loaded();
        assert!(act(&mut state, &galaxy, &a()));
        let seen = state
            .faction_marks
            .get("private:#seen:technologies:a")
            .unwrap();
        assert_eq!(seen, "parasite-obs,planesplitter-obs");
        assert_eq!(
            state
                .faction_marks
                .get("private:#seen:breakthrough:a")
                .map(String::as_str),
            Some("obsidianbt")
        );
        assert_eq!(crate::supply::staged_events(&state), 1);
        assert!(
            state
                .faction_marks
                .values()
                .any(|row| row.contains(FLIPPED)),
            "FACTION_FLIPPED is staged for the game to announce"
        );
        assert!(
            crate::supply::staging_enabled(&state),
            "staging stays on for a seat that is now the Obsidian"
        );
    }

    #[test]
    fn a_planet_another_player_holds_stays_theirs_and_is_not_readied_for_the_flipper() {
        let galaxy = map();
        let mut state = loaded();
        let old_home = home(&state, &a());
        state.system_mut(&old_home).set_control(cronos(), b());
        assert!(act(&mut state, &galaxy, &a()));
        let board = state.system_state(&SystemId::new("96b"));
        assert_eq!(
            board.planet_control.get(&PlanetId::new("cronoshollow")),
            Some(&b())
        );
        assert!(
            state
                .exhausted_planets
                .contains(&PlanetId::new("cronoshollow")),
            "exhaustion follows the planet; only controlled Hollows are readied"
        );
    }

    #[test]
    fn the_transition_refuses_atomically() {
        let galaxy = map();
        // No plot.
        let mut bare = game();
        let before = serde_json::to_string(&bare).unwrap();
        assert!(!act(&mut bare, &galaxy, &a()));
        assert_eq!(
            become_obsidian(&mut bare, content(), DEFAULT, &galaxy, &a()),
            Err(FlipError::NoPlot(a()))
        );
        assert_eq!(serde_json::to_string(&bare).unwrap(), before);
        // Not the Firmament.
        let mut sol = loaded();
        let before = serde_json::to_string(&sol).unwrap();
        assert_eq!(
            become_obsidian(&mut sol, content(), DEFAULT, &galaxy, &b()),
            Err(FlipError::NotFirmament(b()))
        );
        // A map that does not carry the home tile cannot take the replacement.
        let other = ring(
            &plain(7)[0],
            &plain(7)[1..].iter().map(String::as_str).collect::<Vec<_>>(),
        );
        assert!(matches!(
            become_obsidian(&mut sol, content(), DEFAULT, &other, &a()),
            Err(FlipError::Map(_))
        ));
        assert_eq!(serde_json::to_string(&sol).unwrap(), before);
        // Without a map the action does nothing.
        let mut table = scripted(&[]);
        let option = ChoiceOption::labelled(BECOME, crate::faction_abilities::ACTION_KIND, "x");
        let done = crate::fixtures::with_context(&mut sol, DEFAULT, None, &mut table, |context| {
            crate::factions::perform_component(context, &a(), &option)
        });
        assert!(!done);
        assert_eq!(serde_json::to_string(&sol).unwrap(), before);
    }

    #[test]
    fn once_obsidian_the_firmament_abilities_are_gone_and_new_plots_are_faceup() {
        let galaxy = map();
        let mut state = loaded();
        assert!(act(&mut state, &galaxy, &a()));
        assert!(!firmament::is_firmament(&state, &a()));
        assert!(!can_become_obsidian(&state, &a()), "it happens once");
        state.record_score(&b(), ObjectiveId::new("fwm"));
        assert!(
            firmament::plot_options(
                &state,
                content(),
                DEFAULT,
                &a(),
                crate::secrets::Timing::Status,
                None,
                None
            )
            .is_empty(),
            "Plots Within Plots is purged with the sheet"
        );
        firmament::place_plot(&mut state, &a(), &b());
        assert_eq!(state.player(&a()).unwrap().plots.last().unwrap(), "u:b");
    }

    #[test]
    fn a_game_without_the_firmament_is_never_offered_the_flip() {
        let galaxy = map();
        let state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        for who in [a(), b()] {
            assert!(
                crate::factions::mapped_component_actions(
                    &state,
                    content(),
                    DEFAULT,
                    &galaxy,
                    &who
                )
                .iter()
                .all(|option| option.id != BECOME)
            );
        }
    }

    use ti4_model::id::ObjectiveId;
}
