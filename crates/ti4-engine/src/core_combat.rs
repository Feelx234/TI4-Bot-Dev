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
use ti4_engine_core::rules::combat as cc;
use ti4_engine_core::rules::combat::Rd;
use ti4_engine_core::kernel::hooks::BorrowedStats;
use ti4_engine_core::kernel::seam::{adapters, providers, Delta, Layer, Pass, Replacement, RollKind, RollSite, RuleRef, SeatSystemSite, Seams, SetWhy, Source, Step, Verdict};
use ti4_engine_core::kernel::Next;
use ti4_engine_core::state::{Piece, Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;
use ti4_model::units::Unit as StrUnit;

use crate::core_sync::{CoreView, Ctx, Sources};
use crate::generated::IdStr;

/// The slices the combat queries read.
/// (The map slice is left out until the retreat rules need adjacency and a galaxy.)
pub const READS: SliceSet = SliceSet::of(&[
    Slice::Meta,
    Slice::Seats,
    Slice::Players,
    Slice::Board,
    Slice::Transient,
    Slice::Marks,
    Slice::Agenda,
    Slice::Cards,
]);

fn typed<T: IdStr>(id: &str) -> Option<T> {
    T::from_id(id)
}

// -- the string bridge --------------------------------------------------------------------------
//
// The faction modules are not ported to the core yet, so their answers come from the string engine.
// Providers on the seams are plain functions, so the string answers reach them through a thread-local
// [`Oracle`] installed for the duration of one view call. It dies with the last faction module that moves
// to `ti4-engine-core-factions`.

/// What the string bridge answers from: a copy of the position and the owner table.
struct Oracle {
    gs: GameState,
    sources: SourceSet,
    owners: Vec<Option<String>>,
}

thread_local! {
    static ORACLE: std::cell::RefCell<Option<std::rc::Rc<Oracle>>> = const { std::cell::RefCell::new(None) };
}

/// Restores the previous oracle when a view call ends.
struct Installed(Option<std::rc::Rc<Oracle>>);

impl Installed {
    fn new(o: std::rc::Rc<Oracle>) -> Self {
        Self(ORACLE.with(|c| c.borrow_mut().replace(o)))
    }
}

impl Drop for Installed {
    fn drop(&mut self) {
        ORACLE.with(|c| *c.borrow_mut() = self.0.take());
    }
}

fn with_oracle<R>(f: impl FnOnce(&Oracle) -> R) -> Option<R> {
    let o = ORACLE.with(|c| c.borrow().clone())?;
    Some(f(&o))
}

impl Oracle {
    fn player(&self, seat: Seat) -> Option<PlayerId> {
        self.owners.get(usize::from(seat.0))?.as_deref().map(PlayerId::new)
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

/// `Source` of the string bridge's providers.
const STRINGS: Source = Source::Rule(RuleRef(9200));

fn add(n: i64) -> Delta {
    if n == 0 { Delta::None } else { Delta::Add(n) }
}

/// Register the bridge: the string modules' share of the roll shift (faction modules, Mahact, Nekro, the
/// plots; the law, Morale Boost, Fighter Prototype, the nebula and the two abilities are data rows), the
/// dice, `may_sustain`, borrowed stats and the Alastor's ground forces.
fn register_string_bridge(seams: &mut Seams) {
    seams.hit_shift.push_add(STRINGS, Layer::Faction, |_, site: &RollSite, _| {
        if site.kind != RollKind::Combat {
            return Delta::None;
        }
        add(with_oracle(|o| {
            let u = site.unit;
            let Some(player) = o.player(u.owner) else { return 0 };
            let context = if u.ctx == CombatCtx::Space { "space" } else { "ground" };
            let system = u.system.map(|s| SystemId::new(s.as_str()));
            let module = crate::factions::combat_modifier(&o.gs, ContentStore::embedded(), &player, context);
            let unit_mod = o
                .with_unit(u, |cu| crate::factions::unit_roll_modifier(&o.gs, ContentStore::embedded(), o.sources, cu))
                .unwrap_or(0);
            let mahact = crate::factions::mahact_units::flagship_roll_bonus(
                &o.gs,
                ContentStore::embedded(),
                o.sources,
                &player,
                u.unit.as_str(),
            );
            let nekro = crate::factions::nekro_units::mech_roll_bonus(
                &o.gs,
                ContentStore::embedded(),
                o.sources,
                &player,
                u.unit.as_str(),
                system.as_ref(),
                None,
            );
            let assail =
                crate::factions::firmament_plots::assail_space(&o.gs, ContentStore::embedded(), o.sources, &player, system.as_ref());
            module + unit_mod + mahact + nekro + assail
        })
        .unwrap_or(0))
    });
    seams.dice.push_set(STRINGS, Layer::Faction, |_, site: &RollSite, acc| {
        let to = with_oracle(|o| {
            o.with_unit(site.unit, |cu| {
                crate::factions::unit_dice(&o.gs, ContentStore::embedded(), o.sources, cu, acc)
            })
        })
        .flatten()
        .unwrap_or(acc);
        if to == acc { Delta::None } else { Delta::Set { to, why: SetWhy::LegacyFold } }
    });
    seams.dice.push_add(STRINGS, Layer::Faction, |_, site: &RollSite, _| {
        if site.kind != RollKind::Combat {
            return Delta::None;
        }
        add(with_oracle(|o| {
            let (Some(player), Some(system)) = (o.player(site.unit.owner), site.unit.system) else { return 0 };
            let sid = SystemId::new(system.as_str());
            let ships = crate::combat::ships_of(&o.gs, ContentStore::embedded(), o.sources, &player, &sid);
            let index = usize::from(site.unit_index);
            let bastion = ships
                .get(index)
                .map_or(0, |unit| i64::try_from(crate::factions::bastion::extra_die(unit)).unwrap_or(0));
            let agent = crate::factions::borrowed_round_agents::extra_die_for(
                &o.gs,
                ContentStore::embedded(),
                o.sources,
                &sid,
                None,
                &player,
                site.unit.unit.as_str(),
                index,
                o.gs.combat_round_seq,
            );
            bastion + agent
        })
        .unwrap_or(0))
    });
    seams.may_sustain.push(STRINGS, |_, unit: &CombatUnit| {
        let allowed = with_oracle(|o| {
            o.with_unit(*unit, |cu| {
                crate::factions::hooks_combat::may_sustain(&o.gs, ContentStore::embedded(), o.sources, cu)
            })
        })
        .flatten()
        .unwrap_or(true);
        if allowed { Verdict::Abstain } else { Verdict::Forbid }
    });
    seams.borrowed_stats.push(Replacement {
        source: STRINGS,
        step: Step::BorrowedStats,
        offer: |_, site: &SeatSystemSite| {
            with_oracle(|o| {
                let player = o.player(site.who)?;
                let sid = SystemId::new(site.system.as_str());
                let b = crate::factions::hooks_combat::borrowed_stats(&o.gs, ContentStore::embedded(), o.sources, &player, &sid)?;
                Some(BorrowedStats {
                    unit: typed::<Unit>(b.unit.type_id.as_str())?,
                    sustained: b.unit.sustained_damage,
                    galvanized: b.unit.galvanized,
                    hits_on: b.hits_on.and_then(|v| i8::try_from(v).ok()),
                    barrage: b.barrage.and_then(|(v, d)| Some((i8::try_from(v).ok()?, i8::try_from(d).ok()?))),
                })
            })
            .flatten()
        },
        perform: |_, _, _| Next::Done,
    });
    seams.alastor_participants.push(STRINGS, Pass::Add, |_, site: &SeatSystemSite, out| {
        with_oracle(|o| {
            let Some(player) = o.player(site.who) else { return };
            let found = crate::factions::nekro_units::alastor_participants(&o.gs, &player, &SystemId::new(site.system.as_str()));
            for (planet, index) in found {
                if let (Some(p), Ok(i)) = (typed::<Planet>(planet.as_str()), u16::try_from(index)) {
                    out.push((p, i));
                }
            }
        });
    });
}

/// The typed view of one position, for the combat queries.
pub struct CombatView<'g> {
    _position: std::marker::PhantomData<&'g GameState>,
    view: CoreView,
    cx: Ctx,
    kernel: Kernel,
    blanked: bool,
    oracle: std::rc::Rc<Oracle>,
}

impl<'g> CombatView<'g> {
    /// The view of a position; `None` when something is not mapped.
    #[must_use]
    pub fn of(gs: &'g GameState, _content: &'g ContentStore, sources: SourceSet) -> Option<Self> {
        let mut cx = Ctx::of(gs).ok()?;
        let view = CoreView::of(gs, &mut cx, Sources { galaxy: None, rng: None }, READS).ok()?;
        let scope = crate::core_adapter::scope(sources);
        let mut kernel = Kernel::plain();
        providers::register_core(&mut kernel.hooks);
        adapters::bridge_legacy(&mut kernel.hooks.seam);
        cc::register_naaz_nomad(&mut kernel.hooks);
        register_string_bridge(&mut kernel.hooks.seam);
        kernel.sources = CoreSources(u8::try_from(scope.0).ok()?);
        let owners = (0..16_u8).map(|i| cx.owners.id(Seat(i)).map(str::to_owned)).collect();
        let oracle = std::rc::Rc::new(Oracle { gs: gs.clone(), sources, owners });
        Some(Self { _position: std::marker::PhantomData, view, cx, kernel, blanked: crate::laws::blanked(gs), oracle })
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
        let _installed = Installed::new(self.oracle.clone());
        f(Rd::new(&self.view, &self.kernel).with_blanked(self.blanked))
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

    /// `combat::roll_fleet` from the dice stream standing at `positions`: the entries it records (faces,
    /// munitions rerolls) and where the streams stand afterwards.
    #[must_use]
    pub fn roll_fleet_raw(
        &mut self,
        seed: u64,
        positions: &crate::rng::RngPositions,
        player: &PlayerId,
        system: &SystemId,
    ) -> Option<(Vec<cc::FleetRoll>, crate::rng::RngPositions)> {
        use ti4_engine_core::state::{Rng as CoreRng, RngDomain, SeedSource};
        let (seat, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        let mut typed_pos: Vec<(RngDomain, u128)> = Vec::new();
        for (t, p) in positions {
            typed_pos.push((crate::core_sync::codec::rng::domain(&mut self.cx, t).ok()?, *p));
        }
        let seeds = crate::core_sync::codec::rng::TextSeeds(&self.cx);
        let mut rng = CoreRng::new(seed);
        rng.set_positions(&typed_pos, &seeds as &dyn SeedSource);
        let rolls = self.with(|rd| cc::roll_fleet(rd, &mut rng, &seeds, seat, sys));
        let after = crate::core_sync::codec::rng::positions(&self.cx, &rng).ok()?;
        Some((rolls, after))
    }

    /// [`Self::roll_fleet_raw`] as the dice history it leaves: each entry as the dual compares it
    /// (`scene`: the system the host stamps on every roll).
    #[must_use]
    pub fn roll_fleet(
        &mut self,
        seed: u64,
        positions: &crate::rng::RngPositions,
        player: &PlayerId,
        system: &SystemId,
        scene: Option<&str>,
    ) -> Option<(Vec<DiceRow>, crate::rng::RngPositions)> {
        let seat = self.seat(player)?;
        let scene = match scene {
            Some(s) => Some(typed::<System>(s)?),
            None => None,
        };
        let (rolls, after) = self.roll_fleet_raw(seed, positions, player, system)?;
        let mut ledger = ti4_engine_core::state::Ledger::default();
        cc::record_fleet_rolls(&mut ledger, seat, scene, &rolls);
        let rows = ledger
            .dice
            .iter()
            .map(|r| {
                (
                    purpose_code(r.purpose),
                    r.owner.and_then(|o| self.cx.owners.id(o)).map(str::to_owned),
                    r.scene.map(|s| s.as_str().to_owned()),
                    r.hits_on.map(u32::from),
                    r.faces.iter().map(|f| u32::from(*f)).collect(),
                    (0..64_usize).filter(|i| r.rerolled & (1 << i) != 0).collect(),
                )
            })
            .collect();
        Some((rows, after))
    }

    /// `combat_round_dice` of the staged sets of the two sides (the attacker's first).
    #[must_use]
    pub fn round_dice(
        &mut self,
        sides: [(&PlayerId, &Option<ti4_model::state::RerollSet>); 2],
    ) -> Option<Vec<ti4_model::state::CombatRollRecord>> {
        use crate::core_sync::codec::transient as tc;
        let mut sets = Vec::with_capacity(2);
        for (player, set) in sides {
            let seat = self.seat(player)?;
            let set = match set {
                Some(s) => Some(tc::reroll_set(&mut self.cx, s).ok()?),
                None => None,
            };
            sets.push((seat, set));
        }
        let args: Vec<_> = sets.iter().map(|(seat, set)| (*seat, set.as_ref())).collect();
        let dice = cc::round_dice(&args);
        dice.iter().map(|d| tc::die_back(&self.cx, d).ok()).collect()
    }

    /// `combat::sustains_in_space`.
    #[must_use]
    pub fn sustains_in_space(&self, system: &SystemId, player: &PlayerId, unit: &StrUnit, in_combat: bool) -> Option<bool> {
        let (sys, seat, piece) = (typed::<System>(system.as_str())?, self.seat(player)?, self.piece(unit)?);
        Some(self.with(|rd| cc::sustains_in_space(rd, sys, seat, piece, in_combat)))
    }

    /// `combat::assignable_to_hit` (`ability`: the hit is a unit ability's).
    #[must_use]
    pub fn assignable_to_hit(
        &self,
        player: &PlayerId,
        system: &SystemId,
        unit_type: &str,
        planet: Option<&PlanetId>,
        ability: bool,
    ) -> Option<bool> {
        let (sys, seat, unit) = (typed::<System>(system.as_str())?, self.seat(player)?, typed::<Unit>(unit_type)?);
        let planet = match planet {
            Some(p) => Some(typed::<Planet>(p.as_str())?),
            None => None,
        };
        Some(self.with(|rd| cc::assignable_to_hit(rd, seat, sys, unit, planet, ability)))
    }

    /// `combat::sustainer_counts`.
    #[must_use]
    pub fn sustainer_counts(
        &self,
        system: &SystemId,
        player: &PlayerId,
        in_combat: bool,
        available: &[(String, StrUnit)],
    ) -> Option<std::collections::BTreeMap<String, u64>> {
        let (sys, seat) = (typed::<System>(system.as_str())?, self.seat(player)?);
        let mut pieces = Vec::with_capacity(available.len());
        for (location, unit) in available {
            pieces.push((location.starts_with("planet:"), self.piece(unit)?));
        }
        let counts = self.with(|rd| cc::sustainer_counts(rd, sys, seat, in_combat, &pieces));
        Some(counts.into_iter().map(|(u, n)| (u.as_str().to_owned(), n)).collect())
    }

    /// `combat::direct_hittable_at`.
    #[must_use]
    pub fn direct_hittable_at(&self, player: &PlayerId, system: &SystemId, unit: &str) -> Option<bool> {
        let (sys, seat, unit) = (typed::<System>(system.as_str())?, self.seat(player)?, typed::<Unit>(unit)?);
        Some(self.with(|rd| cc::direct_hittable_at(rd, seat, sys, unit)))
    }

    /// The options of the sustain offer, rendered: (id, kind, label, payload) per option, and the prompt.
    #[must_use]
    pub fn sustain_options(
        &self,
        player: &PlayerId,
        ask: ti4_engine_core::choice::combat::Ask,
    ) -> Option<(String, Vec<OptionRow>)> {
        use ti4_engine_core::choice::Ask as AnyAsk;
        let seat = self.seat(player)?;
        let opts = self.with(|rd| cc::sustain_opts(rd, seat, ask));
        let r = crate::render::Render::new(Vec::new());
        let any = AnyAsk::Combat(ask);
        let rows = opts
            .iter()
            .map(|o| {
                let o = crate::render::option(&r, &any, o);
                (o.id, o.kind, o.label, o.payload)
            })
            .collect();
        Some((crate::render::prompt(&r, &any), rows))
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

/// An option as the dual compares it: id, kind, label, payload.
pub type OptionRow = (String, String, String, std::collections::BTreeMap<String, serde_json::Value>);

/// One pooled roll group as the dual compares it: hit number, roll tag, dice, unit types with counts.
pub type FleetGroupRow = (i64, u8, i64, Vec<(String, u32)>);

/// One dice-history entry as the dual compares it: purpose, the seat whose dice, the scene system, the
/// hit number, the faces and the dice a reroll replaced.
pub type DiceRow = (u8, Option<String>, Option<String>, Option<u32>, Vec<u32>, std::collections::BTreeSet<usize>);

fn purpose_code(p: ti4_engine_core::state::RollPurpose) -> u8 {
    use ti4_engine_core::state::RollPurpose as P;
    match p {
        P::SpaceCombat => 0,
        P::Reroll => 5,
        _ => 255,
    }
}

/// A string dice-history entry as the dual compares it.
#[must_use]
pub fn dice_row(r: &crate::dice::Roll) -> DiceRow {
    let purpose = if r.reason == "space combat" {
        0
    } else if r.reason.starts_with("munitions:") {
        5
    } else {
        255
    };
    (purpose, r.by.clone(), r.system.clone(), r.hits_on, r.faces.clone(), r.rerolled.clone())
}

/// `combat::direct_hittable`: the printed rule over the game's source scope.
#[must_use]
pub fn direct_hittable(sources: SourceSet, unit: &str) -> Option<bool> {
    let scope = crate::core_adapter::scope(sources);
    Some(cc::direct_hittable(scope, typed::<Unit>(unit)?))
}
