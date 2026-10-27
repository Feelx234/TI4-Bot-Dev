//! Package CD (registry line): the typed side of the hook duals. Each function answers one string faction function of
//! naaz, naalu, sardakk, titans, yin or yssaril with the same function of `ti4_engine_core_factions::cd_registry` over a
//! typed view of the position, in the string function's own types. `None` means "the typed core cannot express this
//! position" (an unknown id, more than 16 owners): the caller keeps the string answer.

use ti4_engine_core::Seat;
use ti4_engine_core::generated::{Leader, Planet, System, Unit, UnitBaseType};
use ti4_engine_core::hooks::{CombatCtx, CombatUnit as CoreCombatUnit};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::{CoreState, Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::GameState;

use crate::core_sync::codec::Sources;
use crate::core_sync::{CoreView, Ctx};
use crate::factions::CombatUnit;
use crate::generated::IdStr;

use ti4_engine_core_factions::cd_registry::{naalu, naaz, sardakk, titans, yin, yssaril};

/// The modules whose tier Q this package has ported.
pub const PORTED: &[ti4_engine_core_factions::Id] = &[
    ti4_engine_core_factions::Id::Naalu,
    ti4_engine_core_factions::Id::Naaz,
    ti4_engine_core_factions::Id::Sardakk,
    ti4_engine_core_factions::Id::Titans,
    ti4_engine_core_factions::Id::Yin,
    ti4_engine_core_factions::Id::Yssaril,
];

/// The slices the CD hooks read.
const READS: SliceSet = SliceSet::of(&[
    Slice::Meta,
    Slice::Seats,
    Slice::Players,
    Slice::Board,
    Slice::Cards,
    Slice::Strategy,
    Slice::Trade,
    Slice::Marks,
    Slice::Transient,
]);

/// One position as the CD hooks read it.
pub struct Pos {
    view: CoreView,
    cx: Ctx,
    k: &'static Kernel,
}

impl Pos {
    fn of(gs: &GameState, sources: SourceSet) -> Option<Self> {
        Self::with_map(gs, sources, None)
    }

    /// With the map slice, which the string galaxy feeds (adjacency).
    fn with_map(gs: &GameState, sources: SourceSet, galaxy: Option<&ti4_content::galaxy::Galaxy>) -> Option<Self> {
        let mut cx = Ctx::of(gs).ok()?;
        let reads = if galaxy.is_some() { READS.union(SliceSet::of(&[Slice::Map])) } else { READS };
        let view = CoreView::of(gs, &mut cx, Sources { galaxy, rng: None }, reads).ok()?;
        Some(Self { view, cx, k: crate::core_adapter::kernel_for(sources) })
    }

    fn s(&self) -> &CoreState {
        &self.view
    }

    fn seat(&self, p: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(p.as_str())
    }

    fn player(&self, seat: Seat) -> Option<PlayerId> {
        self.cx.owners.id(seat).map(PlayerId::new)
    }
}

/// Run `f` over the typed view of `gs`.
fn with<T>(gs: &GameState, sources: SourceSet, f: impl FnOnce(&Pos) -> Option<T>) -> Option<T> {
    let p = Pos::of(gs, sources);
    if p.is_none() && std::env::var_os("TI4_CD_TRACE").is_some() {
        eprintln!("CD_TRACE typed view unavailable");
    }
    f(&p?)
}

fn full() -> SourceSet {
    ti4_model::content_types::FULL
}

fn system(id: &SystemId) -> Option<System> {
    <System as IdStr>::from_id(id.as_str())
}

fn planet(id: &PlanetId) -> Option<Planet> {
    <Planet as IdStr>::from_id(id.as_str())
}

fn unit_kind(id: &str) -> Option<Unit> {
    <Unit as IdStr>::from_id(id)
}

fn leader(id: &LeaderId) -> Option<Leader> {
    <Leader as IdStr>::from_id(id.as_str())
}

fn base_type(base: &str) -> Option<UnitBaseType> {
    Some(match base {
        "carrier" => UnitBaseType::Carrier,
        "cruiser" => UnitBaseType::Cruiser,
        "destroyer" => UnitBaseType::Destroyer,
        "dreadnought" => UnitBaseType::Dreadnought,
        "fighter" => UnitBaseType::Fighter,
        "flagship" => UnitBaseType::Flagship,
        "infantry" => UnitBaseType::Infantry,
        "mech" => UnitBaseType::Mech,
        "pds" => UnitBaseType::Pds,
        "spacedock" => UnitBaseType::Spacedock,
        "warsun" => UnitBaseType::Warsun,
        _ => return None,
    })
}

/// The typed form of a string `CombatUnit`.
fn combat_unit(p: &Pos, u: &CombatUnit<'_>) -> Option<CoreCombatUnit> {
    Some(CoreCombatUnit {
        owner: p.seat(u.player)?,
        system: match u.system {
            Some(s) => Some(system(s)?),
            None => None,
        },
        planet: match u.planet {
            Some(pl) => Some(planet(pl)?),
            None => None,
        },
        unit: unit_kind(u.unit_type)?,
        ctx: match u.context {
            "space" => CombatCtx::Space,
            "ground" => CombatCtx::Ground,
            _ => return None,
        },
    })
}

/// A typed piece as the string engine's unit.
fn model_unit(p: &Pos, piece: ti4_engine_core::state::Piece) -> Option<ti4_model::units::Unit> {
    let mut unit = ti4_model::units::Unit::new(UnitTypeId::new(piece.kind().as_str()), p.player(piece.owner())?);
    unit.sustained_damage = piece.sustained();
    unit.galvanized = piece.galvanized();
    Some(unit)
}

// -- the combat hooks of every CD faction -------------------------------------------------------------------

macro_rules! unit_roll_modifier {
    ($name:ident, $module:ident) => {
        pub fn $name(gs: &GameState, sources: SourceSet, unit: &CombatUnit<'_>) -> Option<i64> {
            with(gs, sources, |p| Some($module::query::unit_roll_modifier(p.s(), p.k, combat_unit(p, unit)?)))
        }
    };
}

unit_roll_modifier!(naaz_unit_roll_modifier, naaz);
unit_roll_modifier!(naalu_unit_roll_modifier, naalu);
unit_roll_modifier!(sardakk_unit_roll_modifier, sardakk);

// -- naaz ---------------------------------------------------------------------------------------------------

pub fn naaz_unit_dice(gs: &GameState, sources: SourceSet, unit: &CombatUnit<'_>, dice: i64) -> Option<i64> {
    with(gs, sources, |p| Some(naaz::query::unit_dice(p.s(), p.k, combat_unit(p, unit)?, dice)))
}

pub fn naaz_explore_extra_draw(gs: &GameState, sources: SourceSet, who: &PlayerId, pl: &PlanetId) -> Option<bool> {
    with(gs, sources, |p| Some(naaz::query::explore_extra_draw(p.s(), p.k, p.seat(who)?, planet(pl)?)))
}

pub fn naaz_cannot_produce(gs: &GameState, who: &PlayerId, unit_base: &str, producer_base: &str) -> Option<bool> {
    with(gs, full(), |p| Some(naaz::query::cannot_produce(p.s(), p.k, p.seat(who)?, base_type(unit_base)?, base_type(producer_base)?)))
}

pub fn naaz_effect_placement_forbidden(gs: &GameState, sources: SourceSet, who: &PlayerId, unit: &UnitTypeId) -> Option<bool> {
    with(gs, sources, |p| Some(naaz::query::effect_placement_forbidden(p.s(), p.k, p.seat(who)?, unit_kind(unit.as_str())?)))
}

pub fn naaz_captured_unit_return_form(gs: &GameState, sources: SourceSet, owner: &PlayerId, unit: &UnitTypeId) -> Option<UnitTypeId> {
    with(gs, sources, |p| {
        let back = naaz::query::captured_unit_return_form(p.s(), p.k, p.seat(owner)?, unit_kind(unit.as_str())?);
        Some(UnitTypeId::new(back.as_str()))
    })
}

pub fn naaz_ability_hit_immune(gs: &GameState, sources: SourceSet, unit: &CombatUnit<'_>) -> Option<bool> {
    with(gs, sources, |p| Some(naaz::query::ability_hit_immune(p.s(), p.k, combat_unit(p, unit)?)))
}

pub fn naaz_commander_unlocked(gs: &GameState, who: &PlayerId, l: &LeaderId) -> Option<Option<bool>> {
    with(gs, full(), |p| Some(naaz::query::commander_unlocked(p.s(), p.k, p.seat(who)?, leader(l)?)))
}

// -- naalu --------------------------------------------------------------------------------------------------

pub fn naalu_fighter_fleet_weight_halves(gs: &GameState, who: &PlayerId, unit: &str) -> Option<bool> {
    with(gs, full(), |p| Some(naalu::query::fighter_fleet_weight_halves(p.s(), p.k, p.seat(who)?, unit_kind(unit)?)))
}

pub fn naalu_matriarch_fighters(gs: &GameState, sources: SourceSet, invader: &PlayerId, sys: &SystemId) -> Option<Vec<ti4_model::units::Unit>> {
    with(gs, sources, |p| {
        let found = naalu::query::matriarch_fighters(p.s(), p.k, p.seat(invader)?, system(sys)?);
        found.as_slice().iter().map(|piece| model_unit(p, *piece)).collect()
    })
}

pub fn naalu_temporary_ground_force(
    gs: &GameState,
    sources: SourceSet,
    who: &PlayerId,
    sys: &SystemId,
    pl: &PlanetId,
    unit: &ti4_model::units::Unit,
) -> Option<bool> {
    with(gs, sources, |p| {
        let mut piece = ti4_engine_core::state::Piece::new(unit_kind(unit.type_id.as_str())?, p.seat(&unit.owner)?);
        piece = piece.with_sustained(unit.sustained_damage).with_galvanized(unit.galvanized);
        Some(naalu::query::matriarch_temporary_ground_force(p.s(), p.k, p.seat(who)?, system(sys)?, planet(pl)?, piece))
    })
}

pub fn naalu_commander_has_peek(gs: &GameState, who: &PlayerId) -> Option<bool> {
    with(gs, full(), |p| Some(naalu::query::commander_has_peek(p.s(), p.seat(who)?)))
}

pub fn naalu_secondary_waivers(
    gs: &GameState,
    follower: &PlayerId,
    primary: &PlayerId,
) -> Option<Vec<crate::factions::hooks_strategy::SecondaryWaiver>> {
    use crate::factions::hooks_strategy::SecondaryWaiver;
    with(gs, full(), |p| {
        let found = naalu::query::secondary_waivers(p.s(), p.k, p.seat(follower)?, p.seat(primary)?, None);
        let mut out = Vec::new();
        for w in found.as_slice() {
            let (note, faction) = naalu::query::mindsieve_parts(w.code)?;
            // A generic note's corpus record is `<color>_{alias}`; the key carries the alias.
            let alias = <ti4_engine_core::generated::PromissoryNote as IdStr>::as_str(note);
            let name = format!("{}:{}", alias.strip_prefix("<color>_").unwrap_or(alias), faction_alias(faction));
            let id = format!("ms|{name}");
            out.push(SecondaryWaiver { id, label: format!("resolve the secondary by giving {primary} the {name} promissory note") });
        }
        // The string lists the notes in the text order of their keys.
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Some(out)
    })
}

/// The faction alias of a promissory note key (`gift:naalu`): the corpus alias of the faction.
fn faction_alias(faction: ti4_engine_core::generated::Faction) -> &'static str {
    <ti4_engine_core::generated::Faction as IdStr>::as_str(faction)
}

// -- sardakk ------------------------------------------------------------------------------------------------

pub fn sardakk_ground_rolls_extra_hits(
    gs: &GameState,
    roller: &PlayerId,
    sys: &SystemId,
    pl: &PlanetId,
    own_hits: usize,
    opponent_hits: usize,
) -> Option<usize> {
    with(gs, full(), |p| Some(sardakk::query::ground_rolls_extra_hits(p.s(), p.k, p.seat(roller)?, system(sys)?, planet(pl)?, own_hits, opponent_hits)))
}

pub fn sardakk_commander_unlocked(gs: &GameState, who: &PlayerId, l: &LeaderId) -> Option<Option<bool>> {
    with(gs, full(), |p| Some(sardakk::query::commander_unlocked(p.s(), p.k, p.seat(who)?, leader(l)?)))
}

pub fn sardakk_commit_candidates(
    gs: &GameState,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    invader: &PlayerId,
    sys: &SystemId,
    already: &[(SystemId, PlanetId)],
) -> Option<Vec<crate::factions::hooks_ground::CommitCandidate>> {
    use crate::factions::hooks_ground::CommitCandidate;
    let p = Pos::with_map(gs, sources, galaxy)?;
    (|p: &Pos| {
        let already: Option<Vec<_>> = already.iter().map(|(s, pl)| Some((system(s)?, planet(pl)?))).collect();
        let found = sardakk::query::commit_candidates(p.s(), p.k, p.seat(invader)?, system(sys)?, &already?);
        found
            .as_slice()
            .iter()
            .map(|c| {
                Some(CommitCandidate {
                    system: SystemId::new(c.system.as_str()),
                    planet: PlanetId::new(c.planet.as_str()),
                    unit: model_unit(p, c.unit)?,
                })
            })
            .collect()
    })(&p)
}

// -- titans -------------------------------------------------------------------------------------------------

pub fn titans_forced_combat_planets(gs: &GameState, sources: SourceSet, invader: &PlayerId, sys: &SystemId) -> Option<Vec<PlanetId>> {
    with(gs, sources, |p| {
        let found = titans::query::forced_combat_planets(p.s(), p.k, p.seat(invader)?, system(sys)?);
        Some(found.as_slice().iter().map(|pl| PlanetId::new(pl.as_str())).collect())
    })
}

pub fn titans_slumberstate_draw_bonus(gs: &GameState, who: &PlayerId, requested: usize) -> Option<usize> {
    with(gs, full(), |p| Some(titans::query::slumberstate_draw_bonus(p.s(), p.k, p.seat(who)?, requested)))
}

pub fn titans_commander_unlocked(gs: &GameState, who: &PlayerId, l: &LeaderId) -> Option<Option<bool>> {
    with(gs, full(), |p| Some(titans::query::commander_unlocked(p.s(), p.k, p.seat(who)?, leader(l)?)))
}

// -- yin ----------------------------------------------------------------------------------------------------

pub fn yin_extra_prerequisite_colours(gs: &GameState, who: &PlayerId) -> Option<Vec<(String, usize)>> {
    with(gs, full(), |p| {
        let found = yin::query::extra_prerequisite_colours(p.s(), p.k, p.seat(who)?);
        Some(found.as_slice().iter().map(|c| (colour_text(c.colour).to_owned(), c.count)).collect())
    })
}

fn colour_text(colour: ti4_engine_core::generated::TechType) -> &'static str {
    use ti4_engine_core::generated::TechType;
    match colour {
        TechType::Biotic => "green",
        TechType::Cybernetic => "yellow",
        TechType::Propulsion => "blue",
        TechType::Warfare => "red",
        TechType::Unitupgrade => "unitupgrade",
    }
}

pub fn yin_commander_unlocked(gs: &GameState, who: &PlayerId, l: &LeaderId) -> Option<Option<bool>> {
    with(gs, full(), |p| Some(yin::query::commander_unlocked(p.s(), p.k, p.seat(who)?, leader(l)?)))
}

/// The research waiver as the string engine builds it; the payments are sorted by id (the string lists them in board
/// order, which is the text order of the systems).
pub fn yin_research_waiver_offer(gs: &GameState, who: &PlayerId, tech: &ti4_model::id::TechnologyId) -> Option<Option<crate::factions::hooks_strategy::ResearchWaiver>> {
    use crate::factions::hooks_strategy::{ResearchWaiver, ResearchWaiverPayment};
    with(gs, full(), |p| {
        let seat = p.seat(who)?;
        let tech = <ti4_engine_core::generated::Technology as IdStr>::from_id(tech.as_str())?;
        let offered = yin::query::research_waiver_offer(p.s(), p.k, seat, tech);
        let Some(_) = offered else { return Some(None) };
        // The payments in board order: for each system, its planets then its space area.
        let mut payments = Vec::new();
        for (_, sys, pl) in yin::query::infantry_payments(p.s(), seat).iter() {
            payments.push(match pl {
                Some(pl) => ResearchWaiverPayment {
                    id: format!("ground|{}|{}", sys.as_str(), pl.as_str()),
                    label: format!("return 1 infantry from {} in {}", pl.as_str(), sys.as_str()),
                },
                None => ResearchWaiverPayment {
                    id: format!("space|{}", sys.as_str()),
                    label: format!("return 1 infantry from space in {}", sys.as_str()),
                },
            });
        }
        Some(Some(ResearchWaiver {
            id: "yin_infantry".to_owned(),
            label: "return 1 infantry to reinforcements to ignore its prerequisites".to_owned(),
            payments,
        }))
    })
}

// -- yssaril ------------------------------------------------------------------------------------------------

pub fn yssaril_deepgloom_reach(gs: &GameState, a: &PlayerId, b: &PlayerId) -> Option<bool> {
    with(gs, full(), |p| Some(yssaril::query::deepgloom_reach(p.s(), p.k, p.seat(a)?, p.seat(b)?)))
}

pub fn yssaril_transaction_limit_exempt(gs: &GameState, active: &PlayerId, other: &PlayerId) -> Option<bool> {
    with(gs, full(), |p| Some(yssaril::query::transaction_limit_exempt(p.s(), p.k, p.seat(active)?, p.seat(other)?)))
}

pub fn yssaril_action_card_draw_bonus(gs: &GameState, who: &PlayerId, requested: usize) -> Option<usize> {
    with(gs, full(), |p| Some(yssaril::query::action_card_draw_bonus(p.s(), p.k, p.seat(who)?, requested)))
}

pub fn yssaril_action_card_limit(gs: &GameState, who: &PlayerId, limit: usize) -> Option<usize> {
    with(gs, full(), |p| Some(yssaril::query::action_card_limit(p.s(), p.k, p.seat(who)?, limit)))
}

pub fn yssaril_action_cards_forbidden(gs: &GameState, who: &PlayerId) -> Option<bool> {
    with(gs, full(), |p| Some(yssaril::query::action_cards_forbidden(p.s(), p.seat(who)?)))
}

pub fn yssaril_commander_unlocked(gs: &GameState, who: &PlayerId, l: &LeaderId) -> Option<Option<bool>> {
    with(gs, full(), |p| Some(yssaril::query::commander_unlocked(p.s(), p.k, p.seat(who)?, leader(l)?)))
}

// -- the offline cross-product -----------------------------------------------------------------------------

/// What [`compare`] did.
#[derive(Debug, Default, Clone)]
pub struct CdCheck {
    pub positions: u64,
    pub unmapped: u64,
    /// Which variants (`naaz/map`) the typed view could not hold.
    pub unmapped_what: Vec<String>,
    pub compared: u64,
    /// The slots (`naaz::unit_dice`) the cross-product reached.
    pub slots: std::collections::BTreeSet<&'static str>,
    pub mismatches: Vec<String>,
}

impl CdCheck {
    pub fn add(&mut self, o: &Self) {
        self.positions += o.positions;
        self.unmapped += o.unmapped;
        self.compared += o.compared;
        self.slots.extend(o.slots.iter().copied());
        self.mismatches.extend(o.mismatches.iter().take(60usize.saturating_sub(self.mismatches.len())).cloned());
    }
}

/// The variants of a position that put each CD faction at seat 0 with the state its hooks read.
fn variants(gs: &GameState) -> Vec<(&'static str, GameState)> {
    use ti4_model::id::{BreakthroughId, FactionId, TechnologyId};
    use ti4_model::state::LeaderStatus;
    use ti4_model::units::Unit as ModelUnit;

    let mut out = Vec::new();
    if gs.players.len() < 2 {
        return out;
    }
    let (a, b) = (gs.players[0].id.clone(), gs.players[1].id.clone());
    let hot: Vec<SystemId> = gs.board.iter().filter(|(_, s)| !s.units.is_empty() || !s.planet_units.is_empty()).map(|(k, _)| k.clone()).take(3).collect();
    let Some(first) = hot.first().cloned() else { return out };
    let planet_of = |gs: &GameState, s: &SystemId| gs.board.get(s).and_then(|b| b.planet_units.keys().next().cloned());
    let unit = |t: &str, o: &PlayerId| ModelUnit::new(UnitTypeId::new(t), o.clone());
    let mut base = gs.clone();
    base.rules_version = base.rules_version.max(17);
    base.activation_seq = 7;

    let mut v = base.clone();
    v.players[0].faction = FactionId::new("naaz");
    v.players[0].technologies.insert(TechnologyId::new("pfa"));
    v.system_mut(&first).units.extend([unit("naaz_flagship", &a), unit("naaz_mech", &a), unit("carrier", &b)]);
    if let Some(pl) = planet_of(&v, &first) {
        v.system_mut(&first).planet_units.entry(pl.clone()).or_default().extend([unit("naaz_mech", &a), unit("naaz_voltron", &a)]);
        v.exhausted_planets.insert(pl);
    }
    for kind in ["space", "ground"] {
        v.faction_marks.insert(format!("naaz:supercharge:{a}"), format!("{kind}:{}", v.combat_round_seq));
    }
    out.push(("naaz", v));

    let mut v = base.clone();
    v.players[0].faction = FactionId::new("naalu");
    v.players[0].breakthrough = Some(BreakthroughId::new("naalubt"));
    v.players[0].leaders.insert(ti4_model::id::LeaderId::new("naalucommander"), LeaderStatus::Unlocked);
    v.players[1].relic_fragments.insert("CULTURAL".to_owned(), 1);
    v.system_mut(&first).units.extend([unit("naalu_flagship", &a), unit("naalu_fighter", &a), unit("naalu_fighter2", &a), unit("fighter", &b)]);
    if let Some(pl) = planet_of(&v, &first) {
        v.system_mut(&first).planet_units.entry(pl).or_default().extend([unit("naalu_mech", &a), unit("infantry", &b)]);
    }
    v.promissory_notes.insert(format!("cf:{}", v.players[1].faction), a.clone());
    v.faction_marks.insert("naalu:gift_round".to_owned(), v.round.to_string());
    out.push(("naalu", v.clone()));
    v.faction_marks.remove("naalu:gift_round");
    out.push(("naalu-telepathic", v));

    let mut v = base.clone();
    v.players[0].faction = FactionId::new("sardakk");
    v.players[0].technologies.insert(TechnologyId::new("vpw"));
    v.players[0].leaders.insert(ti4_model::id::LeaderId::new("sardakkcommander"), LeaderStatus::Unlocked);
    v.system_mut(&first).units.extend([unit("sardakk_flagship", &a), unit("cruiser", &a), unit("cruiser", &b)]);
    if let Some(pl) = planet_of(&v, &first) {
        v.system_mut(&first).planet_units.entry(pl.clone()).or_default().extend([unit("infantry", &a), unit("sardakk_mech", &a), unit("infantry", &b)]);
        v.faction_marks.insert("sardakk:tekklar:combat".to_owned(), format!("{first}|{pl}|{b}|{a}"));
    }
    out.push(("sardakk", v));

    let mut v = base.clone();
    v.players[0].faction = FactionId::new("titans");
    v.players[0].breakthrough = Some(BreakthroughId::new("titansbt"));
    v.players[0].leaders.insert(ti4_model::id::LeaderId::new("titanscommander"), LeaderStatus::Unlocked);
    v.phase = ti4_model::state::Phase::Status;
    if let Some(pl) = planet_of(&v, &first) {
        v.system_mut(&first).planet_units.entry(pl.clone()).or_default().extend([unit("infantry", &a), unit("infantry", &b), unit("pds", &a), unit("spacedock", &a), unit("pds", &a)]);
        v.faction_marks.insert(format!("titans:coalescence:{pl}"), format!("7|{first}"));
        v.system_mut(&first).coexisting.entry(pl).or_default().insert(b.clone());
    }
    out.push(("titans", v));

    let mut v = base.clone();
    v.players[0].faction = FactionId::new("yin");
    v.players[0].leaders.insert(ti4_model::id::LeaderId::new("yincommander"), LeaderStatus::Unlocked);
    v.players[1].technologies.extend(["amd", "dxa", "gd", "cm"].map(TechnologyId::new));
    v.system_mut(&first).units.push(unit("infantry", &a));
    if let Some(pl) = planet_of(&v, &first) {
        v.system_mut(&first).planet_units.entry(pl).or_default().extend([unit("infantry", &a), unit("infantry", &a)]);
    }
    v.faction_marks.insert(format!("yin:ability_used:{a}"), "1".to_owned());
    out.push(("yin", v));

    let mut v = base.clone();
    v.players[0].faction = FactionId::new("yssaril");
    v.players[0].technologies.insert(TechnologyId::new("tp"));
    v.players[1].passed = true;
    v.phase = ti4_model::state::Phase::Action;
    v.active = Some(a.clone());
    v.turn_seq = 9;
    v.faction_marks.insert(format!("private:#borrowed_scheming:{b}"), a.to_string());
    v.faction_marks.insert(format!("yssaril:bt:negotiating:{a}:{b}"), "true".to_owned());
    v.faction_marks.insert(format!("yssaril:bt:exempt:{b}:{a}"), "9".to_owned());
    out.push(("yssaril", v));
    out
}

/// Every CD hook, through the string dispatch functions (each runs the module wrappers, so the hook dual of
/// `Area::Factions` compares each answer), asked for every seat, system, planet and unit kind the position offers.
fn drive(gs: &GameState, galaxy: Option<&ti4_content::galaxy::Galaxy>, sources: SourceSet) {
    use crate::factions::{self, hooks_combat, hooks_cards, hooks_economy, hooks_ground, hooks_strategy};
    let content = ti4_content::ContentStore::embedded();
    let ids: Vec<PlayerId> = gs.players.iter().map(|p| p.id.clone()).collect();
    let systems: Vec<SystemId> = gs.board.iter().filter(|(_, s)| !s.units.is_empty() || !s.planet_units.is_empty()).map(|(k, _)| k.clone()).take(4).collect();
    let kinds = ["naaz_mech", "naaz_flagship", "naaz_voltron", "naalu_mech", "naalu_fighter2", "naalu_flagship", "sardakk_flagship", "cruiser", "infantry", "mech", "yssaril_flagship"];
    let techs = ["amd", "dxa", "gd", "cm"].map(ti4_model::id::TechnologyId::new);
    let leaders = ["naazcommander", "naalucommander", "sardakkcommander", "titanscommander", "yincommander", "yssarilcommander", "sardakkagent"].map(ti4_model::id::LeaderId::new);
    for p in &ids {
        let _ = hooks_economy::action_card_draw_bonus(gs, content, p, 1);
        let _ = hooks_economy::action_card_limit(gs, content, p, 7);
        let _ = hooks_economy::action_cards_forbidden(gs, p);
        let _ = hooks_strategy::extra_prerequisite_colours(gs, content, p);
        let _ = crate::factions::naalu::commander_has_peek(gs, p);
        for t in &techs {
            let _ = hooks_strategy::research_waiver_offers(gs, content, p, t);
        }
        for l in &leaders {
            let _ = factions::commander_unlocked(gs, content, sources, galaxy, p, l);
        }
        for q in &ids {
            let _ = hooks_cards::transaction_reach(gs, content, p, q);
            let _ = hooks_cards::transaction_exempt_from_limit(gs, content, p, q);
            let _ = hooks_strategy::secondary_waivers(gs, content, p, q, None);
        }
        for base in ["mech", "infantry"] {
            let _ = hooks_economy::cannot_produce(gs, content, p, base, "spacedock");
        }
        for kind in kinds {
            let id = UnitTypeId::new(kind);
            let _ = hooks_economy::effect_placement_forbidden(gs, content, sources, p, &id);
            let _ = hooks_economy::captured_unit_return_form(gs, content, sources, p, &id);
            let _ = hooks_combat::fighter_fleet_weight_halves(gs, content, p, kind);
        }
        for sys in &systems {
            let _ = hooks_ground::forced_combat_planets(gs, content, sources, p, sys);
            let _ = hooks_ground::temporary_space_commit_candidates(gs, content, sources, p, sys);
            let _ = hooks_ground::commit_candidates(gs, content, sources, galaxy, p, sys, &[]);
            let planets: Vec<PlanetId> = gs.board.get(sys).map(|b| b.planet_units.keys().cloned().collect()).unwrap_or_default();
            for kind in kinds {
                for context in ["space", "ground"] {
                    let planet = if context == "ground" { planets.first() } else { None };
                    if context == "ground" && planet.is_none() {
                        continue;
                    }
                    let unit = CombatUnit { player: p, system: Some(sys), planet, unit_type: kind, context };
                    let _ = factions::unit_roll_modifier(gs, content, sources, &unit);
                    let _ = factions::unit_dice(gs, content, sources, &unit, 1);
                    let _ = hooks_combat::ability_hit_immune(gs, content, sources, &unit);
                }
            }
            for pl in &planets {
                let _ = hooks_economy::explore_extra_draw(gs, content, sources, p, pl);
                let _ = hooks_ground::ground_rolls_extra_hits(gs, content, sources, p, sys, pl, 1, 1);
                if let Some(board) = gs.board.get(sys) {
                    for u in board.planet_units.get(pl).into_iter().flatten() {
                        let _ = hooks_ground::temporary_ground_force(gs, content, sources, p, sys, pl, u);
                    }
                }
            }
        }
    }
    // The bookkeeping observers act on a copy.
    let mut copy = gs.clone();
    hooks_strategy::strategy_phase_ended(&mut copy);
    for p in &ids {
        for pl in gs.board.values().flat_map(|b| b.planet_units.keys()).take(3) {
            let mut copy = gs.clone();
            hooks_economy::explored(&mut copy, content, sources, p, pl);
        }
        for sys in &systems {
            for planet in gs.board.get(sys).map(|b| b.planet_units.keys().cloned().collect::<Vec<_>>()).unwrap_or_default() {
                for payment in [format!("ground|{sys}|{planet}"), format!("space|{sys}")] {
                    for t in &techs {
                        let mut copy = gs.clone();
                        let _ = hooks_strategy::research_waiver_paid(&mut copy, content, p, t, 0, &payment);
                    }
                }
            }
        }
    }
}

/// The CD hooks over variants of one position that seat each CD faction at seat 0 and set the state the hooks read; every
/// answer is compared by the hook dual (`Area::Factions` on `Dual` for the duration), the differences become mismatches.
pub fn compare(label: &str, gs: &GameState, galaxy: &ti4_content::galaxy::Galaxy, sources: SourceSet, out: &mut CdCheck) {
    use crate::dual::{Area, AreaMask, Backend};
    out.positions += 1;
    let _ = crate::dual::take_report();
    let mask = AreaMask::all(Backend::String).with(Area::Factions, Backend::Dual);
    for (name, v) in variants(gs) {
        // A variant the typed view cannot hold would compare the string answer with itself: count it.
        for (what, mapped) in [("plain", Pos::of(&v, sources).is_some()), ("map", Pos::with_map(&v, sources, Some(galaxy)).is_some())] {
            if !mapped {
                out.unmapped += 1;
                out.unmapped_what.push(format!("{name}/{what}"));
            }
        }
        crate::dual::with_mask(mask, || {
            drive(&v, Some(galaxy), sources);
            drive(&v, None, sources);
        });
        let report = crate::dual::take_report();
        out.compared += report.hooks.values().map(|h| h.calls).sum::<u64>();
        out.slots.extend(report.hooks.iter().filter(|(_, h)| h.calls > 0).map(|(slot, _)| *slot));
        for d in &report.differences {
            if out.mismatches.len() < 60 {
                out.mismatches.push(format!("{label}: {name}: {d}"));
            }
        }
        if report.differences.is_empty() && report.dropped > 0 {
            out.mismatches.push(format!("{label}: {name}: {} dropped differences", report.dropped));
        }
    }
}

// -- the order audit ------------------------------------------------------------------------------------

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use ti4_engine_core::kernel::HookTable;
use ti4_engine_core::kernel::seam::{Clamp, Q, RollKind, RollSite};

/// Every module of every package registered in the string `MODULES` order (what the kernel will hold).
fn full_table() -> &'static HookTable {
    static TABLE: OnceLock<HookTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = HookTable::new();
        ti4_engine_core_factions::register_all(&mut t);
        t
    })
}

/// One position in this many call sites of the sampled wrapper is audited.
const AUDIT_EVERY: u64 = 1024;

/// The order audit of the commutative slots CD filled (`order::<slot>` in the report), sampled: call it from a hot wrapper
/// and it audits one position in [`AUDIT_EVERY`]. Only runs in `Dual`.
pub fn sampled_audit(gs: &GameState, sources: SourceSet) {
    static CALLS: AtomicU64 = AtomicU64::new(0);
    if !matches!(crate::dual::backend(crate::dual::Area::Factions), crate::dual::Backend::Dual)
        || CALLS.fetch_add(1, Ordering::Relaxed) % AUDIT_EVERY != 0
    {
        return;
    }
    audit(gs, sources);
}

/// [`sampled_audit`] without the sampling: every seat and every site of the position, with the providers registered in
/// reverse and in two shuffles.
pub fn audit(gs: &GameState, sources: SourceSet) {
    use crate::dual::{HookGroup, audit_hook_order};
    let Some(pos) = Pos::of(gs, sources) else { return };
    let s = pos.s();
    let sources = pos.k.sources;
    let table = full_table();
    let kernel = |t: &HookTable| Kernel { hooks: t.clone(), sources };
    let seats: Vec<Seat> = s.players().iter().map(|p| p.seat).collect();
    let systems: Vec<System> = s.board().systems().filter(|(_, c)| !c.units.is_empty()).map(|(sys, _)| sys).collect();
    let planets: Vec<(System, Planet)> = s
        .board()
        .planets()
        .filter(|(_, c)| !c.units.is_empty())
        .filter_map(|(p, _)| Some((ti4_engine_core::query::board::home_system(s, p)?, p)))
        .collect();
    let kinds = [Unit::Iconoclast, Unit::ViszElVir, Unit::Eidolon, Unit::EidolonMaximum, Unit::HybridCrystalFighterII, Unit::CmorranNorr, Unit::CarrierI];
    let mut sites = Vec::new();
    for w in &seats {
        for kind in kinds {
            for sys in &systems {
                sites.push(RollSite {
                    unit: CoreCombatUnit { owner: *w, system: Some(*sys), planet: None, unit: kind, ctx: CombatCtx::Space },
                    kind: RollKind::Combat,
                    round: 1,
                    unit_index: 0,
                    foes: ti4_engine_core::SeatSet::EMPTY,
                });
            }
            for (sys, pl) in &planets {
                sites.push(RollSite {
                    unit: CoreCombatUnit { owner: *w, system: Some(*sys), planet: Some(*pl), unit: kind, ctx: CombatCtx::Ground },
                    kind: RollKind::Ground,
                    round: 1,
                    unit_index: 0,
                    foes: ti4_engine_core::SeatSet::EMPTY,
                });
            }
        }
    }
    audit_hook_order(HookGroup::Seam, "hit_shift", table, |t| {
        let k = kernel(t);
        sites.iter().map(|site| t.seam.hit_shift.eval(Q::new(s, &k), site, 0, Clamp::None)).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Seam, "dice", table, |t| {
        let k = kernel(t);
        sites.iter().map(|site| t.seam.dice.eval(Q::new(s, &k), site, 1, Clamp::None)).collect::<Vec<_>>()
    });
    let some_system = systems.first().copied().or_else(|| planets.first().map(|(sys, _)| *sys)).unwrap_or_else(|| ti4_engine_core::ids::system_at(0));
    audit_hook_order(HookGroup::Economy, "explore_extra_draw", table, |t| {
        let k = kernel(t);
        seats.iter().flat_map(|w| planets.iter().map(move |(_, p)| (*w, *p))).map(|(w, p)| t.economy.explore_extra_draw(s, &k, w, p)).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Economy, "cannot_produce", table, |t| {
        let k = kernel(t);
        seats.iter().map(|w| t.economy.cannot_produce(s, &k, *w, UnitBaseType::Mech, UnitBaseType::Spacedock)).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Economy, "effect_placement_forbidden", table, |t| {
        let k = kernel(t);
        seats.iter().map(|w| t.economy.effect_placement_forbidden(s, &k, *w, Unit::Eidolon)).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Combat, "ability_hit_immune", table, |t| {
        let k = kernel(t);
        seats.iter().map(|w| t.combat.ability_hit_immune(s, &k, CoreCombatUnit { owner: *w, system: Some(some_system), planet: None, unit: Unit::EidolonMaximum, ctx: CombatCtx::Space })).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Combat, "fighter_fleet_weight_halves", table, |t| {
        let k = kernel(t);
        seats.iter().map(|w| t.combat.fighter_fleet_weight_halves(s, &k, *w, Unit::HybridCrystalFighterII)).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Ground, "ground_rolls_extra_hits", table, |t| {
        let k = kernel(t);
        let planet = planets.first().map_or(Planet::ALL[0], |(_, p)| *p);
        seats.iter().map(|w| t.ground.ground_rolls_extra_hits(s, &k, *w, some_system, planet, 1, 1)).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Ground, "forced_combat_planets", table, |t| {
        let k = kernel(t);
        seats.iter().flat_map(|w| systems.iter().map(move |sys| (*w, *sys))).map(|(w, sys)| t.ground.forced_combat_planets(s, &k, w, sys).as_slice().to_vec()).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Economy, "action_card_draw_bonus", table, |t| {
        let k = kernel(t);
        seats.iter().map(|w| t.economy.action_card_draw_bonus(s, &k, *w, 1)).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Economy, "action_cards_forbidden", table, |t| seats.iter().map(|w| t.economy.action_cards_forbidden(s, *w)).collect::<Vec<_>>());
    audit_hook_order(HookGroup::Cards, "transaction_reach", table, |t| {
        let k = kernel(t);
        seats.iter().flat_map(|a| seats.iter().map(move |b| (*a, *b))).map(|(a, b)| t.cards.transaction_reach(s, &k, a, b)).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Cards, "transaction_limit_exempt", table, |t| {
        let k = kernel(t);
        seats.iter().flat_map(|a| seats.iter().map(move |b| (*a, *b))).map(|(a, b)| t.cards.transaction_limit_exempt(s, &k, a, b)).collect::<Vec<_>>()
    });
    audit_hook_order(HookGroup::Strategy, "extra_prerequisite_colours", table, |t| {
        let k = kernel(t);
        seats.iter().map(|w| t.strategy.extra_prerequisite_colours(s, &k, *w).as_slice().iter().map(|c| (c.colour, c.count)).collect::<Vec<_>>()).collect::<Vec<_>>()
    });
}

// -- bookkeeping observers (tier O) -------------------------------------------------------------------------

/// A bookkeeping observer in `Dual`: `string` runs on the real state, `core` on the typed view taken before it; both then
/// answer `facts` (the marks and flags the observer may change), compared by value.
fn dual_effect<F: PartialEq + std::fmt::Debug>(
    gs: &mut GameState,
    sources: SourceSet,
    slot: &'static str,
    group: crate::dual::HookGroup,
    string: impl FnOnce(&mut GameState),
    core: impl FnOnce(&mut ti4_engine_core::kernel::Cx<'_>, &Pos),
    facts_text: impl Fn(&GameState) -> F,
    facts_core: impl Fn(&CoreState, &Pos) -> F,
) {
    use ti4_engine_core::kernel::{Cx, FirstOption};
    let Some(before) = Pos::of(gs, sources) else { return string(gs) };
    crate::dual::run_dual_hook(
        group,
        slot,
        || {
            string(gs);
            facts_text(gs)
        },
        || {
            let mut state = before.s().clone();
            let mut ask = FirstOption;
            core(&mut Cx::new(&mut state, before.k, &mut ask), &before);
            facts_core(&state, &before)
        },
    );
}

/// `naaz::explored` in `Dual`: the planet's exhausted flag afterwards.
pub fn dual_naaz_explored(gs: &mut GameState, sources: SourceSet, who: &PlayerId, pl: &PlanetId, string: impl FnOnce(&mut GameState)) {
    let (Some(planet_ix), true) = (planet(pl), true) else { return string(gs) };
    dual_effect(
        gs,
        sources,
        "naaz::explored",
        crate::dual::HookGroup::Economy,
        string,
        |cx, p| {
            if let Some(seat) = p.seat(who) {
                naaz::observe::explored(cx, seat, planet_ix);
            }
        },
        |gs| gs.exhausted_planets.contains(pl),
        |s, _| ti4_engine_core::query::board::exhausted(s, planet_ix),
    );
}

/// `naalu::strategy_phase_ended` in `Dual`: the initiative overrides afterwards, by player.
pub fn dual_naalu_strategy_phase_ended(gs: &mut GameState, string: impl FnOnce(&mut GameState)) {
    dual_effect(
        gs,
        full(),
        "naalu::strategy_phase_ended",
        crate::dual::HookGroup::Strategy,
        string,
        |cx, _| naalu::observe::strategy_phase_ended(cx),
        |gs| gs.initiative_overrides.iter().map(|(k, v)| (k.to_string(), *v)).collect::<Vec<_>>(),
        |s, p| {
            let mut out: Vec<(String, i32)> =
                s.strategy().initiative_overrides.iter().filter_map(|(seat, v)| Some((p.player(*seat)?.to_string(), *v))).collect();
            out.sort();
            out
        },
    );
}

/// `yin::research_waiver_paid` in `Dual`: `string` runs on the real state; the typed port runs on the typed view taken
/// before it. Both answer the result and how many units the payer has where the infantry was taken from.
pub fn dual_yin_research_waiver_paid(
    gs: &mut GameState,
    sources: SourceSet,
    who: &PlayerId,
    tech: &ti4_model::id::TechnologyId,
    payment: &str,
    string: impl FnOnce(&mut GameState) -> bool,
) -> bool {
    use ti4_engine_core::kernel::{Cx, FirstOption};
    let before = Pos::of(gs, sources);
    // `ground|<system>|<planet>` or `space|<system>`.
    let parsed = (|| {
        let mut parts = payment.split('|');
        match (parts.next()?, parts.next()?, parts.next()) {
            ("ground", sys, Some(pl)) => Some((system(&SystemId::new(sys))?, Some(planet(&PlanetId::new(pl))?))),
            ("space", sys, None) => Some((system(&SystemId::new(sys))?, None)),
            _ => None,
        }
    })();
    let technology = <ti4_engine_core::generated::Technology as IdStr>::from_id(tech.as_str());
    let (Some(before), Some((sys, pl)), Some(technology)) = (before, parsed, technology) else { return string(gs) };
    let Some(seat) = before.seat(who) else { return string(gs) };
    let held_text = |gs: &GameState| {
        let cell = gs.system_state(&SystemId::new(sys.as_str()));
        let units = match pl {
            Some(pl) => cell.planet_units.get(&PlanetId::new(pl.as_str())).cloned().unwrap_or_default(),
            None => cell.units.clone(),
        };
        units.iter().filter(|u| &u.owner == who).count()
    };
    let mut result = false;
    let out = crate::dual::run_dual_hook(
        crate::dual::HookGroup::Strategy,
        "yin::research_waiver_paid",
        || {
            result = string(gs);
            (result, held_text(gs))
        },
        || {
            let mut core = before.s().clone();
            let mut ask = FirstOption;
            let code = pl.map_or(yin::query::SPACE | u16::try_from(sys.index()).unwrap_or(0), |pl| u16::try_from(pl.index() + 1).unwrap_or(0));
            let waiver = ti4_engine_core::hooks::Waiver { module: ti4_engine_core::generated::Faction::TheYinBrotherhood, code };
            let done = yin::offer::research_waiver_paid(&mut Cx::new(&mut core, before.k, &mut ask), seat, technology, waiver);
            let units = match pl {
                Some(pl) => ti4_engine_core::query::board::planet_units(&core, pl),
                None => ti4_engine_core::query::board::space_units(&core, sys),
            };
            (done, units.iter().filter(|p| p.owner() == seat).count())
        },
    );
    out.0
}

#[cfg(test)]
mod tests {
    use ti4_engine_core::generated::{Breakthrough, Faction, PromissoryNote, Technology};

    use super::*;

    #[test]
    fn the_ids_the_ports_name_are_the_variants_they_expect() {
        for (id, unit) in [
            ("naalu_fighter2", Unit::HybridCrystalFighterII),
            ("naalu_flagship", Unit::Matriarch),
            ("naalu_mech", Unit::Iconoclast),
            ("naaz_flagship", Unit::ViszElVir),
            ("naaz_mech", Unit::Eidolon),
            ("naaz_mech_space", Unit::ZGravEidolonNaazMechSpace),
            ("naaz_voltron", Unit::EidolonMaximum),
            ("sardakk_flagship", Unit::CmorranNorr),
            ("yssaril_flagship", Unit::YsiaYssrila),
        ] {
            assert_eq!(unit_kind(id), Some(unit), "{id}");
        }
        for (id, l) in [
            ("naalucommander", Leader::Maban),
            ("naazcommander", Leader::DartAndTai),
            ("sardakkcommander", Leader::GhomSekkus),
            ("titanscommander", Leader::Tungstantus),
            ("yincommander", Leader::BrotherOmar),
            ("yssarilcommander", Leader::SoAta),
        ] {
            assert_eq!(leader(&LeaderId::new(id)), Some(l), "{id}");
        }
        assert_eq!(<Technology as IdStr>::from_id("vpw"), Some(Technology::ValkyrieParticleWeave));
        assert_eq!(<Technology as IdStr>::from_id("tp"), Some(Technology::TransparasteelPlating));
        assert_eq!(<Breakthrough as IdStr>::from_id("naalubt"), Some(Breakthrough::Mindsieve));
        assert_eq!(<Breakthrough as IdStr>::from_id("titansbt"), Some(Breakthrough::SlumberstateComputing));
        assert_eq!(<PromissoryNote as IdStr>::from_id("gift"), Some(PromissoryNote::GiftOfPrescience));
        assert_eq!(<Faction as IdStr>::as_str(Faction::TheNaaluCollective), "naalu");
        assert_eq!(naalu::query::RULES_FACTION_BUGFIXES, ti4_model::state::RULES_FACTION_BUGFIXES);
    }

    /// Every slot the package wraps with the hook dual.
    const SLOTS: &[&str] = &[
        "naaz::unit_roll_modifier",
        "naaz::unit_dice",
        "naaz::explore_extra_draw",
        "naaz::cannot_produce",
        "naaz::effect_placement_forbidden",
        "naaz::captured_unit_return_form",
        "naaz::ability_hit_immune",
        "naaz::commander_unlocked",
        "naaz::explored",
        "naalu::unit_roll_modifier",
        "naalu::fighter_fleet_weight_halves",
        "naalu::matriarch_fighters",
        "naalu::temporary_ground_force",
        "naalu::secondary_waivers",
        "naalu::commander_has_peek",
        "naalu::strategy_phase_ended",
        "sardakk::unit_roll_modifier",
        "sardakk::ground_rolls_extra_hits",
        "sardakk::commit_candidates",
        "sardakk::commander_unlocked",
        "titans::forced_combat_planets",
        "titans::slumberstate_draw_bonus",
        "titans::commander_unlocked",
        "yin::extra_prerequisite_colours",
        "yin::research_waiver_offer",
        "yin::commander_unlocked",
        "yin::research_waiver_paid",
        "yssaril::deepgloom_reach",
        "yssaril::transaction_limit_exempt",
        "yssaril::action_card_draw_bonus",
        "yssaril::action_card_limit",
        "yssaril::action_cards_forbidden",
        "yssaril::commander_unlocked",
    ];

    /// The cross-product over a seated table reaches every slot and finds no difference. The dual report is process-wide, so
    /// this runs alone: `cargo test -p ti4-engine --lib core_factions::cd -- --ignored --test-threads=1`.
    #[test]
    #[ignore = "reads the process-wide dual report: run alone"]
    fn every_cd_slot_is_compared_over_a_seated_table() {
        let mut gs = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "letnev")], ti4_model::content_types::DEFAULT);
        // A table that has just been dealt holds the generic notes under the faction "generic"; a game names the owner's.
        let notes: Vec<(String, PlayerId)> = std::mem::take(&mut gs.promissory_notes).into_iter().collect();
        for (note, holder) in notes {
            let (alias, _) = note.split_once(':').unwrap_or((note.as_str(), ""));
            let owner = gs.players.iter().find(|p| p.id == holder).map(|p| p.faction.to_string()).unwrap_or_default();
            gs.promissory_notes.insert(format!("{alias}:{owner}"), holder);
        }
        let hub = crate::fixtures::hub_with_centre("18");
        let mut out = CdCheck::default();
        compare("seated", &gs, &hub.galaxy, ti4_model::content_types::DEFAULT, &mut out);
        assert!(out.mismatches.is_empty(), "{:?}", out.mismatches);
        assert_eq!(out.unmapped, 0, "the typed view holds every variant: {:?}", out.unmapped_what);
        let missing: Vec<_> = SLOTS.iter().filter(|s| !out.slots.contains(**s)).collect();
        assert!(missing.is_empty(), "not reached: {missing:?} of {:?}", out.slots);
    }

    #[test]
    fn a_mindsieve_code_names_its_note_and_faction() {
        for note in PromissoryNote::ALL {
            for faction in Faction::ALL {
                assert_eq!(naalu::query::mindsieve_parts(naalu::query::mindsieve_code(note, faction)), Some((note, faction)));
            }
        }
    }
}
