//! What the strategic action reads of the position that no rule window holds, computed from the CORE alone (package KX, lane E).
//!
//! * [`follower_facts`]: what the follower window is told of one follower (`StrategySecondaryWindow::core_facts`): may the seat be asked, what
//!   the follow costs, the token-free ways (waivers) the faction hooks offer.
//! * [`perform_facts`]: what the ability of a strategy card reads besides the position (`kernel::fx_perform::Facts`): the guards the string
//!   adapters (`hero_dual::perform_facts`, `action_cards_view::carried_of`) vouch for, the structures a seat places, the home system, the
//!   public objectives Imperial can score, the systems of Thunder's Edge Warfare's free tactical action and of its Construction's uses of
//!   PRODUCTION.
//!
//! Departures from the string side, by decision (content outside the agreed scope, `docs/D2_LF_RESUME.md` section 0): no Firmament Enervate
//! (the primary instead), Mindsieve (a Thunder's Edge breakthrough of the Naalu: its waiver is refused), no Keleres agent term.

use crate::choice::strategy::Opt as SOpt;
use crate::driver::kernel_env::KernelEnv;
use crate::generated::{Faction, Leader, PromissoryNote, StrategyCard, StrategyCardKind, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::{FreeSystems, ImperialFacts, Placement, StructureFacts, TeUses};
use crate::kernel::fx_perform::{Carried, Facts, Home};
use crate::kernel::fx_strategy::placed_unit;
use crate::kernel::hooks::Waiver as HookWaiver;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::TurnCodes;
use crate::kernel::{fx, Kernel};
use crate::query::tech::UnitRoster;
use crate::query::{laws, reach};
use crate::rules::strategy;
use crate::rules::strategy_secondary::{FollowerFacts, Waiver, WaiverEffect};
use crate::state::{CoreState, NoteKey, ObjectiveRef};

// (Only the latest rules version is supported: the new engine refuses a game below it at load, so no version gate is kept here: the Technology
// ability asks the payment of the research, a space-only structure is placed in the space area, Berekar Berekon is offered to a use of PRODUCTION.)

// -- the followers -------------------------------------------------------------------------------------------------------------------

/// What taking the waiver `w` costs (`hooks_strategy::secondary_waived` of the module that offered it). A module whose payment the core does not
/// carry is [`WaiverEffect::Unported`]: the window refuses the waiver (Mindsieve is the Naalu's Thunder's Edge breakthrough, out of scope).
#[must_use]
pub fn waiver_effect(w: HookWaiver) -> WaiverEffect {
    match w.module {
        // Acquiescence is the only waiver of the Winnu (code 0): the note goes back to its owner.
        Faction::TheWinnu if w.code == 0 => WaiverEffect::GiveBack(NoteKey { note: PromissoryNote::Acquiescence, faction: Faction::TheWinnu }),
        // Jae Mir Kan: the code is the system's index.
        Faction::TheMahactGeneSorcerers => match System::from_index(usize::from(w.code)) {
            Some(system) => WaiverEffect::Agent { leader: Leader::JaeMirKan, system },
            None => WaiverEffect::Unported,
        },
        _ => WaiverEffect::Unported,
    }
}

/// The token-free ways `who` has to follow `primary`'s `card` (`hooks_strategy::secondary_waivers`), as the window offers them.
#[must_use]
pub fn waivers_of(core: &CoreState, kernel: &Kernel, primary: Seat, who: Seat, card: StrategyCard) -> Vec<Waiver> {
    kernel
        .hooks
        .strategy
        .secondary_waivers(core, kernel, who, primary, Some(card.data().kind))
        .iter()
        .map(|w| {
            // Exhausting an agent is heard by the Nomad's suite (a staged event the core does not make): the payment is not carried then.
            let heard = w.module == Faction::TheMahactGeneSorcerers && core.players().iter().any(|p| p.faction == Faction::TheNomad);
            Waiver { opt: SOpt::WaiveBy(w.module, w.code), effect: if heard { WaiverEffect::Unported } else { waiver_effect(*w) } }
        })
        .collect()
}

/// `StrategySecondaryWindow::{eligible, core_facts}` for one follower: whether the seat is asked, what the follow costs and its waivers.
#[must_use]
pub fn follower_facts(core: &CoreState, kernel: &Kernel, primary: Seat, card: StrategyCard, who: Seat) -> FollowerFacts {
    let env = KernelEnv::new(core, kernel);
    let cx = strategy::Cx { core, kernel, env: &env };
    let kind = card.data().kind;
    let costs_base = strategy::secondary_costs_token(Some(kind));
    let all = waivers_of(core, kernel, primary, who, card);
    let eligible = strategy::secondary_eligible(&cx, who, card)
        || (costs_base
            && kind != StrategyCardKind::Leadership
            && !all.is_empty()
            && core.players().by_seat(who).is_some_and(|p| strategy::can_do_something(&cx, who, p, kind)));
    let costs_token = eligible && costs_base && !strategy::secondary_is_free(&cx, who, Some(kind));
    FollowerFacts { who, eligible, costs_token, primary_instead: false, waivers: if costs_token { all } else { Vec::new() } }
}

// -- the abilities ----------------------------------------------------------------------------------------------------------------------

/// The unit `who` places for `base` as Construction does (`strategy_cards::construction_placement`): its form of the structure, placed on a
/// planet, or in the space area when it is a space-only structure; `None` where a faction effect forbids it.
fn placement(core: &CoreState, kernel: &Kernel, roster: &UnitRoster, who: Seat, base: UnitBaseType) -> Placement {
    let scope = reach::scope_of(kernel);
    let unit = placed_unit(core, kernel, scope, roster, who, base)?;
    // (A space-only structure, Saar's Floating Factory, is placed as itself in the space area; every other one on the planet.)
    let in_space = scope.unit(unit).is_some_and(|d| d.space_only);
    if kernel.hooks.economy.effect_placement_forbidden(core, kernel, who, unit) {
        return None;
    }
    Some((unit, in_space))
}

/// Titans' Hecatoncheires (`titans::hecatoncheires_offer`): "When you would place a PDS on a planet, you may place 1 mech and 1 infantry on that
/// planet instead." The two units the seat would place, `None` for any other faction and where a faction effect bans one of them.
fn hecatoncheires(core: &CoreState, kernel: &Kernel, roster: &UnitRoster, who: Seat) -> Option<(Unit, Unit)> {
    if core.players().by_seat(who)?.faction != Faction::TheTitansOfUl {
        return None;
    }
    let scope = reach::scope_of(kernel);
    let mech = placed_unit(core, kernel, scope, roster, who, UnitBaseType::Mech)?;
    let infantry = placed_unit(core, kernel, scope, roster, who, UnitBaseType::Infantry)?;
    if kernel.hooks.economy.effect_placement_forbidden(core, kernel, who, mech) || kernel.hooks.economy.effect_placement_forbidden(core, kernel, who, infantry) {
        return None;
    }
    Some((mech, infantry))
}

/// What a Construction window needs of the position (`strategy_cards_core::structure_facts`). `None` where a faction effect changes the
/// placement the core does not follow: Minister of Industry's production after a dock.
#[must_use]
pub fn structure_facts(core: &CoreState, kernel: &Kernel, roster: &UnitRoster, who: Seat, codes: &TurnCodes) -> Option<StructureFacts> {
    // Minister of Industry: a production after the dock (`laws::industry_produces_on_placement`).
    if laws::holds_ministry(core, reach::blanked(core), who, crate::generated::Agenda::MinisterOfIndustry) {
        return None;
    }
    Some(StructureFacts {
        pds: placement(core, kernel, roster, who, UnitBaseType::Pds),
        dock: placement(core, kernel, roster, who, UnitBaseType::Spacedock),
        source: codes.place_structure,
        alt: hecatoncheires(core, kernel, roster, who),
    })
}

/// The public objectives the seat could score for Imperial's primary (`strategy_cards_core::imperial_facts`): `None` where the core cannot
/// follow (more than sixteen). The payment environment is the production kernel's.
#[must_use]
pub fn imperial_facts(core: &CoreState, kernel: &Kernel, who: Seat) -> Option<ImperialFacts> {
    let seen = crate::seen::Seen::new(core, kernel, who).with_blanked(reach::blanked(core));
    let scoreable = seen.scoreable_publics(who)?;
    if scoreable.len() > 16 { // mutants:skip: a game cannot reveal more than ten objectives a seat can score
        return None;
    }
    let mut options = [None; 16];
    for (slot, objective) in options.iter_mut().zip(scoreable.iter()) {
        // (A leaked secret scored as a public objective is the string's `scoreable_on` too: not carried.)
        let ObjectiveRef::Public(objective) = objective else { return None };
        *slot = Some(*objective);
    }
    Some(ImperialFacts { scoreable: options, env: crate::kernel::fx_pay::PayEnv { scope: reach::scope_of(kernel), blanked: reach::blanked(core) } })
}

/// The systems of Thunder's Edge Warfare's free tactical action (`strategy_cards::te6_warfare`): the printed map and every system in play beside
/// it, in the string's order. `None`: more than the facts hold, or a map the core cannot list.
#[must_use]
pub fn free_systems(core: &CoreState, source: crate::state::Code) -> Option<FreeSystems> {
    let mut ids: Vec<System> = Vec::new();
    // (A position without the map slice is a game without a galaxy: the string's `galaxy: None`.)
    let galaxy = core.loaded.has(crate::state::Slice::Map).then(|| core.galaxy()).flatten();
    if let (Some(galaxy), crate::state::Phase::Action) = (galaxy, core.meta().phase) {
        ids = galaxy.placed_in_hex_order().filter(|s| !galaxy.is_hyperlane(*s)).collect();
        for (system, _) in core.board().systems() {
            if !galaxy.is_hyperlane(system) && !ids.contains(&system) {
                ids.push(system);
            }
        }
    }
    FreeSystems::new(source, &ids)
}

/// The uses of PRODUCTION the Thunder's Edge Construction primary may choose (`strategy_cards::te4_construction`): the facts of the use in each
/// system of the board that can produce, in the order of the board.
#[must_use]
pub fn te_uses(core: &CoreState, kernel: &Kernel, roster: &UnitRoster, who: Seat, source: crate::state::Code) -> Option<TeUses> {
    let cx = crate::rules::production::Cx::new(core, kernel, reach::scope_of(kernel), reach::blanked(core));
    let agent_rules = true; // mutants:skip: the latest rules (a constant)
    let mut uses = Vec::new();
    for (system, _) in core.board().systems() {
        if crate::rules::production::capacity(&cx, who, system) > 0 {
            uses.push(crate::rules::production::home::home_facts(core, kernel, roster, who, system, agent_rules)?);
        }
    }
    TeUses::new(source, &uses)
}

/// Whether the paid research of `who` asks a question the core window does not carry (`strategy_cards::research_asks_more`): Doctor Sucaban (the
/// Jol-Nar agent readied at the table, or a Yssaril agent that may copy it) with infantry of `who` on the board to trade. Specialist
/// Compounds (a Thunder's Edge breakthrough) and the Deepwrought commander (a dropped faction) are out of scope.
#[must_use]
pub fn research_asks_more(core: &CoreState, who: Seat) -> bool {
    use crate::state::LeaderStatus;
    let ready = |leader: Leader| core.players().iter().any(|p| p.leaders.status.get(&leader) == Some(&LeaderStatus::Readied));
    let sucaban = ready(Leader::DoctorSucaban) || (ready(Leader::Ssruu) && core.players().iter().any(|p| p.faction == Faction::TheUniversitiesOfJolNar));
    sucaban && infantry_on_board(core, who)
}

/// `strategy_cards::infantry_sites` is not empty: a unit of the seat whose type is infantry stands in a space area or on a planet.
fn infantry_on_board(core: &CoreState, who: Seat) -> bool {
    let infantry = |p: &crate::state::Piece| p.owner() == who && p.kind().data().base_type == UnitBaseType::Infantry;
    core.board().systems().any(|(_, cell)| cell.units.as_slice().iter().any(infantry)) || core.board().planets().any(|(_, cell)| cell.units.as_slice().iter().any(infantry))
}

/// The facts [`crate::kernel::fx_perform::begin`] reads, for `who` performing `card` (primary or secondary). Without a unit roster (the play
/// facts of the sources are missing) the abilities that need one are `Unsupported`.
#[must_use]
pub fn perform_facts(sx: &Sx<'_>, who: Seat, card: StrategyCard) -> Facts {
    let roster = sx.table.facts.plays.as_ref().and_then(|p| p.roster);
    facts_at(sx.state, sx.kernel, &sx.table.codes.turn, roster, who, card)
}

/// [`perform_facts`] on a position, with the roster and the codes in hand.
#[must_use]
pub fn facts_at(core: &CoreState, kernel: &Kernel, codes: &TurnCodes, roster: Option<&'static UnitRoster>, who: Seat, card: StrategyCard) -> Facts {
    let rules_version = core.meta().rules_version;
    let home = match core.players().by_seat(who).and_then(|p| p.home_system) {
        Some(system) => Home::At(system),
        None => Home::Nothing,
    };
    Facts {
        carried: Carried {
            // (The draw follows the hooks of the faction in the core window: Scheming, Crafty; the Deepgloom Executable is Thunder's Edge.)
            plain_draw: true,
            research: !research_asks_more(core, who),
            payments: true,
            structures: roster.and_then(|roster| structure_facts(core, kernel, roster, who, codes)),
            trade_source: codes.trade_primary,
        },
        // (Cabal's replenish note and the Trade Agreement are the core's; the siphoning plot is a dropped faction's.)
        replenish_events: false,
        rules_version,
        roster,
        scope: reach::scope_of(kernel),
        mahact_held: fx::tokens_held_by_mahact(core, who),
        home,
        imperial: (card.data().kind == StrategyCardKind::Imperial).then(|| imperial_facts(core, kernel, who)).flatten(),
        free: (card == StrategyCard::WarfareThundersEdge).then(|| free_systems(core, codes.te6warfare)).flatten(),
        te_uses: (card == StrategyCard::ConstructionThundersEdge).then(|| roster.and_then(|roster| te_uses(core, kernel, roster, who, codes.te4construction))).flatten(),
    }
}
