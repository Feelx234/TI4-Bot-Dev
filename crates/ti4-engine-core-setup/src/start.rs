//! `start_game`: the opening position of a table, built straight into the typed core (docs/SETUP_NATIVE_DESIGN.md).
//!
//! Every step is the typed twin of a step of the string setup (`ti4_engine::setup::start_game_seeded_with_card_set`,
//! `seating::deploy`, `builders::promissory::deal`, `map_feasibility::apply`, `starting_tech::open`, the speaker of
//! `ti4_host::setup::create`, and what `Game::with_table` / `Game::with_galaxy` did to the position before the first load),
//! in the same order and with the same random draws, so the core equals `Codec::load` of the string position field by field.

use std::collections::BTreeMap;

use ti4_content::ContentStore;
use ti4_content::factions::{self, Placement};
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::containers::{Cow, Deck, SmallList};
use ti4_engine_core::generated::{
    ActionCard, Agenda, Explore, ExploreType, Faction, Leader, Planet, PromissoryNote, PublicObjective, Relic, SecretObjective, StrategyCard,
    System, Technology, Unit,
};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::Word;
use ti4_engine_core::state::{
    CODE_CAPACITY, CombatPresentation, CoreState, Diplomacy, MarkKey, MarkMap, MarkVal, NoteKey, NoteMap, ObjectiveRef, OwnerTable, Phase,
    Piece, PlanetFlags, Player, Players, Relationship, Rng, Seats, StartingTechPick, StartingTechSource, SysFlags, TechList,
};
use ti4_engine_core::state::LeaderStatus;
use ti4_model::content_types::{ContentType, POK, SourceSet};
use ti4_model::id::{FactionId, PlayerId};

use crate::deck::{Exclusions, StartingDecks, build_starting_decks, build_starting_decks_excluding};
use crate::error::{SetupError, unmapped};
use crate::factions::{KELERES_VARIANTS, MODULE_ALIASES, NEKRO, purges_alliance, validate_tribuni};
use crate::feasibility::{census, unsatisfiable_public, unsatisfiable_secret};
use crate::galaxy::typed_map;
use crate::ids::{IdStr, id};
use crate::leaders::starting_states;
use crate::mecatol::{MECATOL_TE, mecatol_in_galaxy};
use crate::rng::seeded_speaker_index;
use crate::strategy::strategy_card_setup;

/// Which seat opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeakerChoice {
    /// The seat at this index of the table (0: the first seat, the default).
    Seat(usize),
    /// Drawn from the game seed, uniformly (`ti4_engine::seeded_speaker`).
    Seeded,
}

/// A table to set up.
#[derive(Debug, Clone, Copy)]
pub struct TableSpec<'a> {
    pub content: &'a ContentStore,
    /// The players in seat order (at most 8).
    pub players: &'a [PlayerId],
    /// The faction alias of every seat, in seat order.
    pub factions: &'a [FactionId],
    /// The game seed: decks, dice, and (for [`SpeakerChoice::Seeded`]) the speaker.
    pub seed: u64,
    /// The named strategy card set (`te`, `pok`, `base_game_codex1`, ...), or `None` for the set of `deck_sources`.
    pub card_set: Option<&'a str>,
    /// The scope the decks are built under (`POK`; Thunder's Edge content is refused).
    pub deck_sources: SourceSet,
    pub speaker: SpeakerChoice,
    /// Ask the starting-technology picks (Winnu, Argent, the Keleres, ...) and settle those that need no answer.
    pub starting_tech: bool,
    /// The rules version of the game; `None`: the newest.
    pub rules_version: Option<u32>,
}

/// The opening position: the core with all 17 slices loaded, and the galaxy it was built with (the laws and the printed lanes applied,
/// as `Game::with_galaxy` does).
#[derive(Debug, Clone)]
pub struct CoreStart {
    /// The position the game starts from: the table as created, plus what `Game::with_table` and `Game::with_galaxy` add before the
    /// first load (the frontier tokens, the baselines of what each seat holds, the map).
    pub core: CoreState,
    /// The table as the host's `create` returns it: [`CoreStart::core`] without those additions and without the map. This is the
    /// opening state a server stores for replay and recovery (its string form is `materialize` of this core).
    pub opening: CoreState,
    /// The board with the laws and the printed lanes applied.
    pub galaxy: Galaxy,
}

fn typed_list<T: IdStr + Copy, const N: usize>(what: &'static str, texts: impl IntoIterator<Item = impl AsRef<str>>) -> Result<SmallList<T, N>, SetupError> {
    let mut out = SmallList::new();
    for t in texts {
        out.push(id::<T>(what, t.as_ref())?);
    }
    Ok(out)
}

fn typed_vec<T: IdStr>(what: &'static str, texts: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Vec<T>, SetupError> {
    texts.into_iter().map(|t| id::<T>(what, t.as_ref())).collect()
}

/// The Creuss Gate (tile 17) is named by the faction record; the seat's home system is tile 51.
const CREUSS_GATE: &str = "17";
const CREUSS_HOME: &str = "51";
/// The Crimson Rebellion's record names the Sorrow (tile 94); the seat's home system is tile 118.
const SORROW: &str = "94";
const CRIMSON_HOME: &str = "118";

/// The decks as typed lists, before they are wrapped in `Deck`s (the setup draws from them first).
struct Piles {
    objective: Vec<PublicObjective>,
    explore: Vec<(ExploreType, Vec<Explore>)>,
    relic: Vec<Relic>,
    agenda: Vec<Agenda>,
    action: Vec<ActionCard>,
    secret: Vec<SecretObjective>,
}

fn explore_key(t: ExploreType) -> String {
    t.as_str().to_ascii_uppercase()
}

fn piles(decks: &StartingDecks) -> Result<Piles, SetupError> {
    let mut explore = Vec::new();
    for t in ExploreType::ALL {
        if let Some(cards) = decks.exploration.get(&explore_key(t)) {
            explore.push((t, typed_vec::<Explore>("exploration card", cards)?));
        }
    }
    Ok(Piles {
        objective: typed_vec("public objective", decks.objectives.iter().map(ti4_model::id::ObjectiveId::as_str))?,
        explore,
        relic: typed_vec("relic", decks.relics.iter().map(ti4_model::id::RelicId::as_str))?,
        agenda: typed_vec("agenda", &decks.agendas)?,
        action: typed_vec("action card", decks.action_cards.iter().map(ti4_model::id::ActionCardId::as_str))?,
        secret: typed_vec("secret objective", decks.secrets.iter().map(ti4_model::id::SecretObjectiveId::as_str))?,
    })
}

/// The typed position of one seat while it is being built.
struct Seated {
    seat: Seat,
    player: Player,
    alias: String,
    home: System,
}

/// Seat one player as a faction and place the opening position (`seating::deploy`). The seat's units go onto the core's board.
fn deploy(core: &mut CoreState, content: &ContentStore, seat: Seat, alias: &str) -> Result<Seated, SetupError> {
    let faction = factions::get(content, alias).ok_or_else(|| SetupError::UnknownFaction(alias.to_owned()))?;
    let home = faction.home_system().ok_or_else(|| SetupError::NoHomeSystem(alias.to_owned()))?;
    let system_text = if home == CREUSS_GATE {
        CREUSS_HOME
    } else if home == SORROW {
        CRIMSON_HOME
    } else {
        home
    };
    let home_planets = faction.home_planets();
    let deployments = faction.deployments(content).map_err(|e| SetupError::Seating(e.to_string()))?;
    // Resolve everything against the corpus before the state is touched.
    let mut placements = Vec::new();
    for deployment in deployments {
        let unit_id = factions::resolve_unit(content, alias, &deployment.unit_id, POK);
        placements.push((deployment.count, deployment.placement, id::<Unit>("unit type", unit_id.as_str())?));
    }
    let system = id::<System>("system", system_text)?;
    let mut player = Player::new(seat, id::<Faction>("faction", alias)?);
    // LRR 20.1: the opening command tokens (`ti4_model::state::Player::new`).
    player.econ.tactic_tokens = 3;
    player.econ.fleet_tokens = 3;
    player.econ.strategic_tokens = 2;
    player.home_system = Some(system);
    player.home_planets = typed_list::<Planet, 4>("planet", &home_planets)?;

    core.board.sys_mut(system).flags.set(SysFlags::TOUCHED, true);
    for planet in &home_planets {
        core.board.planet_mut(id::<Planet>("planet", planet)?).control = Some(seat);
    }
    for (count, placement, unit) in placements {
        match placement {
            Placement::Space => {
                for _ in 0..count {
                    core.board.sys_mut(system).units.push(Piece::new(unit, seat));
                }
            }
            Placement::Planet(planet) => {
                let cell = core.board.planet_mut(id::<Planet>("planet", planet.as_str())?);
                cell.flags.set(PlanetFlags::UNITS_ENTRY, true);
                for _ in 0..count {
                    cell.units.push(Piece::new(unit, seat));
                }
            }
        }
    }

    for tech in faction.starting_tech().iter().filter_map(|t| content.resolve_id(ContentType::Technologies, t, POK)) {
        player.tech.owned.insert(id::<Technology>("technology", &tech)?);
    }
    // Nekro Virus: it cannot research, so the two Valefar Assimilator cards come with the faction.
    if alias == NEKRO {
        player.tech.owned.insert(id::<Technology>("technology", "vax")?);
        player.tech.owned.insert(id::<Technology>("technology", "vay")?);
    }
    // Sorrow: "place an inactive breach there".
    if home == SORROW {
        core.explore.breach.insert(id::<System>("system", SORROW)?);
    }
    // 51.1/51.2a: the faction's leaders, agents readied and the rest locked.
    for (leader, status) in starting_states(content, POK, alias) {
        let status = match status {
            ti4_model::state::LeaderStatus::Readied => LeaderStatus::Readied,
            ti4_model::state::LeaderStatus::Locked => LeaderStatus::Locked,
            other => return Err(unmapped("leader status", format!("{other:?}"))),
        };
        player.leaders.status.insert(id::<Leader>("leader", leader.as_str())?, status);
    }
    Ok(Seated { seat, player, alias: alias.to_owned(), home: system })
}

/// 69.1: every seat starts holding its own promissory notes (`promissory::deal`): the four generic ones (Mahact without the Alliance),
/// and the faction's own. The id carries the faction's name.
fn deal_notes(content: &ContentStore, seated: &[(Seat, String)]) -> Result<NoteMap<Seat>, SetupError> {
    // Keyed by text, as the string map is: a later seat of the same faction name takes the note.
    let mut hands: BTreeMap<String, Seat> = BTreeMap::new();
    for (seat, alias) in seated {
        let mut aliases: Vec<String> = ["cf", "ps", "ta", "an"].iter().map(|a| (*a).to_owned()).collect();
        if purges_alliance(alias) {
            aliases.retain(|a| a != "an");
        }
        aliases.extend(factions::get(content, alias).map(|f| f.promissory_notes().into_iter().map(ToOwned::to_owned).collect::<Vec<_>>()).unwrap_or_default());
        for note in aliases {
            hands.insert(format!("{note}:{alias}"), *seat);
        }
    }
    let mut notes = NoteMap::<Seat>::new();
    for (text, seat) in hands {
        let (alias, faction) = text.split_once(':').ok_or_else(|| unmapped("promissory note id", &text))?;
        let note = PromissoryNote::from_id(alias)
            .or_else(|| PromissoryNote::from_id(&format!("<color>_{alias}")))
            .ok_or_else(|| unmapped("promissory note", alias))?;
        notes.insert(NoteKey { note, faction: id::<Faction>("faction", faction)? }, seat);
    }
    Ok(notes)
}

/// The starting-technology picks a table owes (`starting_tech::open`): the Keleres last.
fn starting_picks(content: &ContentStore, seated: &[(Seat, String)]) -> Result<Vec<StartingTechPick>, SetupError> {
    let mut picks = Vec::new();
    let mut last = Vec::new();
    for (seat, alias) in seated {
        if KELERES_VARIANTS.contains(&alias.as_str()) {
            last.push(StartingTechPick { player: *seat, of: 2, remaining: 2, source: StartingTechSource::OwnedByOthers });
            continue;
        }
        let Some(faction) = factions::get(content, alias) else { continue };
        let Some(amount) = faction.starting_tech_amount().filter(|n| *n > 0) else { continue };
        let source = if faction.starting_tech_options().is_empty() {
            StartingTechSource::Researchable
        } else {
            let mut listed = Vec::new();
            for t in faction.starting_tech_options().iter().filter_map(|t| content.resolve_id(ContentType::Technologies, t, POK)) {
                listed.push(id::<Technology>("technology", &t)?);
            }
            StartingTechSource::Listed(TechList::from_slice(&listed).ok_or_else(|| unmapped("starting technology list", listed.len()))?)
        };
        picks.push(StartingTechPick { player: *seat, of: amount, remaining: amount, source });
    }
    picks.extend(last);
    Ok(picks)
}

/// Set a table up (docs/SETUP_NATIVE_DESIGN.md section 5).
///
/// `galaxy` is the board the host built for the table (template or seeded filler): tiles on hexes, no game position. It is handed
/// back in [`CoreStart::galaxy`] with the laws and the printed lanes applied, which is what the map slice of the core was made from.
///
/// # Errors
/// [`SetupError`]: no or too many players, Thunder's Edge content, an unknown faction or card set, the Tribuni, a value the typed core cannot hold.
pub fn start_game(spec: &TableSpec<'_>, galaxy: Galaxy) -> Result<CoreStart, SetupError> {
    let content = spec.content;
    let version = spec.rules_version.unwrap_or(ti4_model::state::RULES_VERSION);
    if version < ti4_model::state::MIN_SUPPORTED_RULES_VERSION || version > ti4_model::state::RULES_VERSION {
        return Err(SetupError::RulesVersion(version, ti4_model::state::MIN_SUPPORTED_RULES_VERSION, ti4_model::state::RULES_VERSION));
    }
    let n = spec.players.len();
    if n == 0 {
        return Err(SetupError::NoPlayers);
    }
    if n > ti4_engine_core::state::MAX_PLAYERS {
        return Err(SetupError::TooManyPlayers(n));
    }
    if spec.factions.len() != n {
        return Err(SetupError::FactionCount { players: n, factions: spec.factions.len() });
    }
    if spec.deck_sources.contains(ti4_model::content_types::Source::ThundersEdge) {
        return Err(SetupError::ThundersEdge);
    }
    // The owner table: the player ids in text order; seat `i` is the `i`-th.
    let mut owners: Vec<&str> = spec.players.iter().map(PlayerId::as_str).collect();
    owners.sort_unstable();
    if owners.windows(2).any(|w| w[0] == w[1]) {
        return Err(SetupError::Seating("a player id is seated twice".to_owned()));
    }
    let seat_of = |p: &PlayerId| Seat(owners.binary_search(&p.as_str()).expect("a player of the table") as u8);
    let seats: Vec<Seat> = spec.players.iter().map(seat_of).collect();

    let mut core = CoreState::all_loaded();
    core.owners = OwnerTable::new(owners.iter().map(|s| s.as_bytes()));
    core.seats = Seats::new(&seats, &seats, owners.len() as u8);
    core.rng = Rng::new(spec.seed);

    // Strategy: the eight cards of the set and their initiative; 83.2.
    let (cards, initiative) = strategy_card_setup(content, spec.deck_sources, spec.card_set).ok_or(SetupError::NoStrategyCardSet)?;
    for card in &cards {
        core.strategy.unclaimed.push(id::<StrategyCard>("strategy card", card.as_str())?);
    }
    for (card, number) in &initiative {
        core.strategy.initiative.insert(id::<StrategyCard>("strategy card", card.as_str())?, *number);
    }

    // The decks, drawn once from the seed: the two public objectives turned up, one secret objective dealt to every seat.
    let mut piles = piles(&build_starting_decks(content, spec.deck_sources, spec.seed))?;
    let mut revealed: Vec<ObjectiveRef> = Vec::new();
    for _ in 0..2 {
        if !piles.objective.is_empty() {
            revealed.push(ObjectiveRef::Public(piles.objective.remove(0)));
        }
    }
    let mut hands: Vec<Vec<SecretObjective>> = vec![Vec::new(); n];
    for hand in &mut hands {
        if piles.secret.is_empty() {
            break;
        }
        hand.push(piles.secret.remove(0));
    }

    // The seats: faction, home system, fleet, technology, leaders (`seating::deploy` for every seat in turn).
    validate_tribuni(spec.factions.iter().map(FactionId::as_str)).map_err(SetupError::Seating)?;
    let mut seated = Vec::with_capacity(n);
    for (seat, faction) in seats.iter().zip(spec.factions) {
        seated.push(deploy(&mut core, content, *seat, faction.as_str())?);
    }
    let table: Vec<(Seat, String)> = seated.iter().map(|s| (s.seat, s.alias.clone())).collect();
    core.cards.promissory = Cow::new(deal_notes(content, &table)?);

    // Setup rule: objectives this board can never satisfy are dealt out of the decks (`map_feasibility::apply`).
    let homes: Vec<ti4_model::id::SystemId> = seated.iter().map(|s| ti4_model::id::SystemId::new(s.home.as_str())).collect();
    let c = census(content, POK, &galaxy, &homes);
    let (public, secret) = (unsatisfiable_public(&c), unsatisfiable_secret(&c));
    if !(public.is_empty() && secret.is_empty()) {
        let again = piles_of_objectives(&build_starting_decks_excluding(
            content,
            POK,
            spec.seed,
            &Exclusions { public: public.iter().map(|a| (*a).to_owned()).collect(), secret: secret.iter().map(|a| (*a).to_owned()).collect() },
        ))?;
        let turned = revealed.len();
        piles.objective = again;
        revealed.clear();
        for _ in 0..turned {
            if !piles.objective.is_empty() {
                revealed.push(ObjectiveRef::Public(piles.objective.remove(0)));
            }
        }
        piles.secret.retain(|s| !secret.contains(&s.as_str()));
        for hand in &mut hands {
            for slot in 0..hand.len() {
                if secret.contains(&hand[slot].as_str()) && !piles.secret.is_empty() {
                    hand[slot] = piles.secret.remove(0);
                }
            }
        }
    }

    // The speaker (`ti4_host::setup::create`): the first seat unless the table says otherwise.
    let speaker = match spec.speaker {
        SpeakerChoice::Seat(i) => *seats.get(i).ok_or(SetupError::SpeakerSeat(i))?,
        SpeakerChoice::Seeded => seats[seeded_speaker_index(n, spec.seed).expect("a table has a seat")],
    };

    // Write the seats and the decks into the core.
    let mut list = Vec::with_capacity(n);
    for (s, hand) in seated.into_iter().zip(hands) {
        let mut player = s.player;
        player.cards.secrets = typed_list::<SecretObjective, 4>("secret objective", hand.iter().map(|h| h.as_str()))?;
        list.push(player);
    }
    let d = &mut core.decks;
    d.action = Deck::new(piles.action);
    d.secret = Deck::new(piles.secret);
    d.agenda = Deck::new(piles.agenda);
    d.relic = Deck::new(piles.relic);
    d.objective = Deck::new(piles.objective);
    for (t, cards) in piles.explore {
        d.explore[t as usize] = Deck::new(cards);
        d.explore_present |= 1 << t as u8;
    }
    for r in revealed {
        core.objectives.revealed.push(r);
    }
    let diplomacy_seats: Vec<Seat> = list.iter().map(|p| p.seat).collect();
    core.players = Players::from_vec(list);

    // Meta (`GameState::new`).
    let m = &mut core.meta;
    m.rules_version = version;
    m.round = 1;
    m.phase = Phase::Strategy;
    m.speaker = speaker;
    m.strategy_cards_per_player = if matches!(n, 3 | 4) { 2 } else { 1 };

    // Trade: the diplomacy ledger holds a neutral relationship for every ordered pair of seats.
    let mut diplomacy = Diplomacy::default();
    for a in &diplomacy_seats {
        for b in &diplomacy_seats {
            if a != b {
                diplomacy.relationships.insert((*a, *b), Relationship::default());
            }
        }
    }
    core.trade.diplomacy = Cow::new(diplomacy);

    // The text table: the empty presentation phase of the combat record is the one text a fresh position mints.
    let phase = core.texts.codes.intern(b"", CODE_CAPACITY).ok_or_else(|| unmapped("text table", "full"))?;
    core.transient.combat.presentation = Cow::new(CombatPresentation { phase: ti4_engine_core::state::Code(phase as u16), ..CombatPresentation::default() });

    // Starting technology: queue the picks, settle those that need no answer (`starting_tech::open`). The table `create` returns ends here.
    if spec.starting_tech {
        for pick in starting_picks(content, &table)? {
            core.transient.misc.starting_tech_picks.push(pick);
        }
        let kernel = crate::kernel::kernel_for(POK);
        let roster = crate::roster::roster_for(content, POK).ok_or_else(|| unmapped("unit roster", "the content store is not the embedded one"))?;
        ti4_engine_core::driver::turn::starting::settle_core(&mut core, kernel, crate::roster::scope(POK), roster);
    }
    let opening = core.clone();

    // What `Game::with_galaxy` did: Thunder's Edge's Mecatol registered on the board, a frontier token in every planetless system,
    // the laws and the printed lanes onto the map. Then the map slice.
    if mecatol_in_galaxy(&galaxy) == MECATOL_TE {
        // (A board with the Thunder's Edge Mecatol is built only under that scope, which the setup refuses.)
        core.board.sys_mut(id::<System>("system", MECATOL_TE)?).flags.set(SysFlags::TOUCHED, true); // mutants:skip
    }
    for system in frontier_systems(content, &galaxy) {
        core.explore.frontier.insert(id::<System>("system", &system)?);
    }
    let mut galaxy = galaxy;
    // (`>=` and `>` agree: the only version the core plays is 36, above the 22 that introduced the lanes.)
    galaxy.hyperlane_lanes = version >= ti4_model::state::RULES_HYPERLANE_LANES; // mutants:skip
    galaxy.wormholes_off = false;
    galaxy.wormholes_all_linked = false;
    galaxy.wormhole_star_links = false;
    galaxy.extra_links.clear();
    galaxy.token_wormholes.clear();
    galaxy.nexus_wormholes_off = false;
    let (typed, adjacency) = typed_map(&galaxy)?;
    core.map = ti4_engine_core::state::MapSlice::of(typed, adjacency);

    // `Game::with_table`: a table with a faction module baselines what each seat holds, so a gain after the load is told from a loaded card.
    if core.players.iter().any(|p| MODULE_ALIASES.contains(&p.faction.as_str())) {
        let mut marks = MarkMap::new();
        for p in core.players.iter() {
            marks.insert(MarkKey::PrivateSeen(Word::Breakthrough, p.seat), MarkVal::List(Vec::new()));
            marks.insert(MarkKey::PrivateSeen(Word::Relics, p.seat), MarkVal::List(Vec::new()));
        }
        core.marks.tail = Cow::new(marks);
    }

    Ok(CoreStart { core, opening, galaxy })
}

fn piles_of_objectives(decks: &StartingDecks) -> Result<Vec<PublicObjective>, SetupError> {
    typed_vec("public objective", decks.objectives.iter().map(ti4_model::id::ObjectiveId::as_str))
}

/// Systems with nothing to land on, which is where a frontier token goes (35.5): the systems of the map that hold things, sorted.
fn frontier_systems(content: &ContentStore, galaxy: &Galaxy) -> Vec<String> {
    let mut systems: Vec<String> = galaxy
        .system_ids_holding_things()
        .into_iter()
        .filter(|sid| {
            ti4_content::galaxy::system(content, sid, POK)
                .is_none_or(|record| record.planets().into_iter().all(|planet| ti4_content::galaxy::is_space_station(content, planet, POK)))
        })
        .map(str::to_owned)
        .collect();
    systems.sort();
    systems
}
