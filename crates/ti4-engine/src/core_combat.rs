//! `Area::Combat` (package B1): the typed space-combat rules of `ti4_engine_core::rules::combat` behind a
//! string interface, and the string engine's side of the faction seam.
//!
//! One [`CombatView`] is built per call site (the codecs of the slices the rules read run once). Every
//! method answers a question of `combat.rs` with the same value in the same order, or `None` when
//! something is not mapped (an id the tables do not know, more than 16 owners), in which case the
//! caller keeps the string answer. Nothing here decides a rule.
//!
//! [`Strs`] is the list of string-only dependencies of the area: each method of the core's
//! [`Faction`] seam is answered by the string engine's faction code on the string state the view was
//! built from.

use ti4_content::ContentStore;
use ti4_engine_core::generated::{Planet, System, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::hooks::{CombatCtx, CombatUnit};
use ti4_engine_core::kernel::{Kernel, Sources as CoreSources};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::rules::combat as cc;
use ti4_engine_core::rules::combat::{Borrowed, FlagshipText, Faction, Rd};
use ti4_engine_core::state::{CoreState, Piece, Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;
use ti4_model::units::Unit as StrUnit;

use crate::core_sync::{CoreView, Ctx, Sources};
use crate::generated::IdStr;

/// The slices the combat queries read.
pub const READS: SliceSet = SliceSet::of(&[
    Slice::Meta,
    Slice::Seats,
    Slice::Players,
    Slice::Board,
    Slice::Map,
    Slice::Transient,
    Slice::Marks,
    Slice::Agenda,
]);

fn typed<T: IdStr>(id: &str) -> Option<T> {
    T::from_id(id)
}

/// The string engine's answers to the core's faction seam.
pub struct Strs<'a> {
    pub gs: &'a GameState,
    pub content: &'a ContentStore,
    pub sources: SourceSet,
    pub cx: &'a Ctx,
}

impl Strs<'_> {
    fn player(&self, seat: Seat) -> Option<PlayerId> {
        self.cx.owners.id(seat).map(PlayerId::new)
    }

    fn with_unit<R>(&self, unit: CombatUnit, f: impl FnOnce(&crate::factions::CombatUnit<'_>) -> R) -> Option<R> {
        let player = self.player(unit.owner)?;
        let system = unit.system.map(|s| SystemId::new(s.as_str()));
        let planet = unit.planet.map(|p| PlanetId::new(p.as_str()));
        let context = match unit.ctx {
            CombatCtx::Space => "space",
            CombatCtx::Ground => "ground",
        };
        let cu = crate::factions::CombatUnit {
            player: &player,
            system: system.as_ref(),
            planet: planet.as_ref(),
            unit_type: unit.unit.as_str(),
            context,
        };
        Some(f(&cu))
    }
}

impl Faction for Strs<'_> {
    fn alastor_participants(&self, _: &CoreState, who: Seat, system: System) -> SmallList<(Planet, u16), 4> {
        let Some(player) = self.player(who) else { return SmallList::new() };
        let found = crate::factions::nekro_units::alastor_participants(self.gs, &player, &SystemId::new(system.as_str()));
        found
            .into_iter()
            .filter_map(|(planet, index)| Some((typed::<Planet>(planet.as_str())?, u16::try_from(index).ok()?)))
            .collect()
    }

    fn flagship_has_text(&self, _: &CoreState, who: Seat, unit: Unit, text: FlagshipText) -> bool {
        let Some(player) = self.player(who) else { return false };
        let id = match text {
            FlagshipText::Jolnar => "jolnar_flagship",
            FlagshipText::Letnev => "letnev_flagship",
            FlagshipText::L1z1x => "l1z1x_flagship",
            FlagshipText::Hacan => "hacan_flagship",
        };
        crate::factions::flagship_has_text(self.gs, &player, unit.as_str(), id)
    }

    fn law_fighter_bonus(&self, _: &CoreState, who: Seat) -> i64 {
        self.player(who).map_or(0, |p| crate::laws::fighter_combat_bonus(self.gs, &p))
    }

    fn combat_modifier(&self, _: &CoreState, who: Seat, ctx: CombatCtx) -> i64 {
        let Some(player) = self.player(who) else { return 0 };
        let context = match ctx {
            CombatCtx::Space => "space",
            CombatCtx::Ground => "ground",
        };
        crate::faction_abilities::combat_modifier(self.gs, self.content, &player, context)
    }

    fn unit_roll_modifier(&self, _: &CoreState, unit: CombatUnit) -> i64 {
        self.with_unit(unit, |cu| crate::factions::unit_roll_modifier(self.gs, self.content, self.sources, cu))
            .unwrap_or(0)
    }

    fn mahact_flagship_bonus(&self, _: &CoreState, who: Seat, unit: Unit) -> i64 {
        let Some(player) = self.player(who) else { return 0 };
        crate::factions::mahact_units::flagship_roll_bonus(self.gs, self.content, self.sources, &player, unit.as_str())
    }

    fn nekro_mech_bonus(&self, _: &CoreState, who: Seat, unit: Unit, system: Option<System>) -> i64 {
        let Some(player) = self.player(who) else { return 0 };
        let system = system.map(|s| SystemId::new(s.as_str()));
        crate::factions::nekro_units::mech_roll_bonus(
            self.gs,
            self.content,
            self.sources,
            &player,
            unit.as_str(),
            system.as_ref(),
            None,
        )
    }

    fn assail_space(&self, _: &CoreState, who: Seat, system: Option<System>) -> i64 {
        let Some(player) = self.player(who) else { return 0 };
        let system = system.map(|s| SystemId::new(s.as_str()));
        crate::factions::firmament_plots::assail_space(self.gs, self.content, self.sources, &player, system.as_ref())
    }

    fn unit_dice(&self, _: &CoreState, unit: CombatUnit, dice: i64) -> i64 {
        self.with_unit(unit, |cu| crate::factions::unit_dice(self.gs, self.content, self.sources, cu, dice))
            .unwrap_or(dice)
    }

    fn borrowed_stats(&self, s: &CoreState, who: Seat, system: System) -> Option<Borrowed> {
        let player = self.player(who)?;
        let sid = SystemId::new(system.as_str());
        let b = crate::factions::hooks_combat::borrowed_stats(self.gs, self.content, self.sources, &player, &sid)?;
        // The string names the ship by value: the first equal one in board order.
        let wanted = self.cx_piece(&b.unit)?;
        let piece = s.board().sys(system).units.iter().find(|p| **p == wanted).copied()?;
        Some(Borrowed { unit: piece, hits_on: b.hits_on, barrage: b.barrage })
    }

    fn bastion_extra_die(&self, _: &CoreState, piece: Piece) -> i64 {
        i64::try_from(crate::factions::bastion::extra_die(&self.str_unit(piece))).unwrap_or(0)
    }

    fn round_agent_extra_die(&self, _: &CoreState, system: System, who: Seat, unit: Unit, index: usize) -> i64 {
        let Some(player) = self.player(who) else { return 0 };
        crate::factions::borrowed_round_agents::extra_die_for(
            self.gs,
            self.content,
            self.sources,
            &SystemId::new(system.as_str()),
            None,
            &player,
            unit.as_str(),
            index,
            self.gs.combat_round_seq,
        )
    }
}

impl Strs<'_> {
    fn str_unit(&self, p: Piece) -> StrUnit {
        StrUnit {
            type_id: ti4_model::id::UnitTypeId::new(p.kind().as_str()),
            owner: self.player(p.owner()).unwrap_or_else(|| PlayerId::new("")),
            sustained_damage: p.sustained(),
            galvanized: p.galvanized(),
        }
    }

    fn cx_piece(&self, u: &StrUnit) -> Option<Piece> {
        let seat = self.cx.owners.seat(u.owner.as_str())?;
        Some(Piece::new(typed::<Unit>(u.type_id.as_str())?, seat).with_sustained(u.sustained_damage).with_galvanized(u.galvanized))
    }
}

/// The typed view of one position, for the combat queries.
pub struct CombatView<'g> {
    view: CoreView,
    cx: Ctx,
    kernel: Kernel,
    gs: &'g GameState,
    content: &'g ContentStore,
    sources: SourceSet,
}

impl<'g> CombatView<'g> {
    /// The view of a position; `None` when something is not mapped.
    #[must_use]
    pub fn of(gs: &'g GameState, content: &'g ContentStore, sources: SourceSet) -> Option<Self> {
        let mut cx = Ctx::of(gs).ok()?;
        let view = CoreView::of(gs, &mut cx, Sources { galaxy: None, rng: None }, READS).ok()?;
        let scope = crate::core_adapter::scope(sources);
        let mut kernel = Kernel::plain();
        kernel.sources = CoreSources(u8::try_from(scope.0).ok()?);
        Some(Self { view, cx, kernel, gs, content, sources })
    }

    fn strs(&self) -> Strs<'_> {
        Strs { gs: self.gs, content: self.content, sources: self.sources, cx: &self.cx }
    }

    fn seat(&self, p: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(p.as_str())
    }

    fn player(&self, s: Seat) -> Option<PlayerId> {
        self.cx.owners.id(s).map(PlayerId::new)
    }

    fn str_unit(&self, p: Piece) -> Option<StrUnit> {
        Some(StrUnit {
            type_id: ti4_model::id::UnitTypeId::new(p.kind().as_str()),
            owner: self.player(p.owner())?,
            sustained_damage: p.sustained(),
            galvanized: p.galvanized(),
        })
    }

    fn piece(&self, u: &StrUnit) -> Option<Piece> {
        Some(
            Piece::new(typed::<Unit>(u.type_id.as_str())?, self.seat(&u.owner)?)
                .with_sustained(u.sustained_damage)
                .with_galvanized(u.galvanized),
        )
    }

    /// Run `f` with the read bundle.
    fn with<R>(&self, f: impl FnOnce(Rd<'_>) -> R) -> R {
        let strs = self.strs();
        f(Rd::new(&self.view, &self.kernel, &strs))
    }

    /// `combat::combatants`.
    #[must_use]
    pub fn combatants(&self, system: &SystemId) -> Option<Vec<PlayerId>> {
        let sys: System = typed(system.as_str())?;
        self.with(|rd| cc::combatants(rd, sys).iter().map(|s| self.player(*s)).collect())
    }

    /// `combat::ships_of`.
    #[must_use]
    pub fn ships_of(&self, player: &PlayerId, system: &SystemId) -> Option<Vec<StrUnit>> {
        let (seat, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        self.with(|rd| cc::ships_of(rd, seat, sys).into_iter().map(|p| self.str_unit(p)).collect())
    }

    /// `combat::effective_hits_on`.
    #[must_use]
    pub fn effective_hits_on(&self, player: &PlayerId, unit: &StrUnit) -> Option<Option<i64>> {
        let (seat, piece) = (self.seat(player)?, self.piece(unit)?);
        Some(self.with(|rd| cc::effective_hits_on(rd, seat, piece)))
    }

    /// `combat::fleet_groups`, flattened: (hit number, tag, dice, [(unit type, count)]).
    #[must_use]
    pub fn fleet_groups(&self, player: &PlayerId, system: &SystemId) -> Option<Vec<FleetGroupRow>> {
        let (seat, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        Some(self.with(|rd| {
            cc::fleet_groups(rd, seat, sys)
                .into_iter()
                .map(|((value, tag), g)| {
                    (value, tag, g.dice, g.types.iter().map(|(u, n)| (u.as_str().to_owned(), *n)).collect())
                })
                .collect()
        }))
    }
}

/// One pooled roll group as the dual compares it: hit number, roll tag, dice, unit types with counts.
pub type FleetGroupRow = (i64, u8, i64, Vec<(String, u32)>);
