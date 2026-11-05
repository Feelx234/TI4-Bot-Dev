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
use ti4_engine_core::kernel::seam::{adapters, providers, Delta, Layer, RollKind, RollSite, RuleRef, Seams, SetWhy, SilenceSite, Source, Verdict};
use ti4_engine_core::state::{Piece, Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;
use ti4_model::units::Unit as StrUnit;

use crate::core_sync::{CoreView, Ctx, Sources};
use crate::core_adapter::typed;
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


// -- the string bridge --------------------------------------------------------------------------
//
// The faction modules are not ported to the core yet, so their answers come from the string engine.
// Providers on the seams are plain functions, so the string answers reach them through a thread-local
// [`Oracle`] installed for the duration of one view call. It dies with the last faction module that moves
// to `ti4-engine-core-factions`.

/// What the string bridge answers from: a copy of the position and the owner table.
pub(crate) struct Oracle {
    pub(crate) gs: GameState,
    pub(crate) sources: SourceSet,
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

pub(crate) fn with_oracle<R>(f: impl FnOnce(&Oracle) -> R) -> Option<R> {
    let o = ORACLE.with(|c| c.borrow().clone())?;
    Some(f(&o))
}

impl Oracle {
    pub(crate) fn player(&self, seat: Seat) -> Option<PlayerId> {
        self.owners.get(usize::from(seat.0))?.as_deref().map(PlayerId::new)
    }

    pub(crate) fn with_unit<R>(&self, unit: CombatUnit, f: impl FnOnce(&crate::factions::CombatUnit<'_>) -> R) -> Option<R> {
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
        if site.kind == RollKind::Barrage {
            // An ANTI-FIGHTER BARRAGE roll reads only the plot (Assail) of the string modules.
            return add(with_oracle(|o| {
                let player = o.player(site.unit.owner)?;
                let system = site.unit.system.map(|s| SystemId::new(s.as_str()));
                Some(crate::factions::firmament_plots::assail_space(&o.gs, ContentStore::embedded(), o.sources, &player, system.as_ref()))
            })
            .flatten()
            .unwrap_or(0));
        }
        if site.kind == RollKind::CannonAssail {
            // Assail against the players this gun fires at.
            return add(with_oracle(|o| {
                let owner = o.player(site.unit.owner)?;
                let targets: Vec<PlayerId> = site.foes.iter().filter_map(|s| o.player(s)).collect();
                Some(crate::factions::firmament_plots::assail_against(&o.gs, &owner, targets.iter()))
            })
            .flatten()
            .unwrap_or(0));
        }
        if site.kind == RollKind::Ground {
            // B2: a ground force's whole shift is the string's `ground_roll_shift` (the faction abilities, Shield
            // Paling's exemption, the unit modifiers, the Nekro mech, Assail against the planet's other
            // owners): no data row applies to a ground roll.
            return add(with_oracle(|o| {
                let player = o.player(site.unit.owner)?;
                let system = SystemId::new(site.unit.system?.as_str());
                let planet = PlanetId::new(site.unit.planet?.as_str());
                let whole = crate::combat_effects::ground_roll_shift(
                    &o.gs,
                    ContentStore::embedded(),
                    o.sources,
                    &player,
                    &system,
                    &planet,
                    site.unit.unit.as_str(),
                );
                // The Nekro mech's term is native (`ca_combat::nekro`'s Mordred row, registered in the combat
                // kernel): counted once, not again through the string total.
                let mordred = crate::factions::nekro_units::mech_roll_bonus(
                    &o.gs,
                    ContentStore::embedded(),
                    o.sources,
                    &player,
                    site.unit.unit.as_str(),
                    Some(&system),
                    Some(&planet),
                );
                let supercharge = o.with_unit(site.unit, |cu| crate::factions::naaz::supercharge_text(&o.gs, ContentStore::embedded(), o.sources, cu)).unwrap_or(0);
                Some(whole - mordred - supercharge)
            })
            .flatten()
            .unwrap_or(0));
        }
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
            let assail =
                crate::factions::firmament_plots::assail_space(&o.gs, ContentStore::embedded(), o.sources, &player, system.as_ref());
            let supercharge = o
                .with_unit(u, |cu| crate::factions::naaz::supercharge_text(&o.gs, ContentStore::embedded(), o.sources, cu))
                .unwrap_or(0);
            module + unit_mod - supercharge + mahact + assail
        })
        .unwrap_or(0))
    });
    seams.dice.push_set(STRINGS, Layer::Faction, |_, site: &RollSite, acc| {
        if !matches!(site.kind, RollKind::Combat | RollKind::Ground) {
            return Delta::None;
        }
        let to = with_oracle(|o| {
            o.with_unit(site.unit, |cu| {
                crate::factions::unit_dice(&o.gs, ContentStore::embedded(), o.sources, cu, acc)
            })
        })
        .flatten()
        .unwrap_or(acc);
        if to == acc { Delta::None } else { Delta::Set { to, why: SetWhy::LegacyFold } }
    });
    seams.dice.push_add(STRINGS, Layer::Faction, |q, site: &RollSite, _| {
        if site.kind == RollKind::Ground {
            // B2: the round agents' extra die of a ground force (the galvanized die and the Sol agent's die are
            // the invasion rule's own).
            return add(site.unit.system.map_or(0, |sys| ti4_engine_core_factions::cc_ground::borrowed::query::extra_die_for(q.s, q.k, sys, site.unit.planet, site.unit.owner, site.unit.unit, usize::from(site.unit_index), q.s.meta().combat_round_seq)));
        }
        if !matches!(site.kind, RollKind::Combat | RollKind::Barrage) {
            return Delta::None;
        }
        let combat = site.kind == RollKind::Combat;
        add(with_oracle(|o| {
            let (Some(player), Some(system)) = (o.player(site.unit.owner), site.unit.system) else { return 0 };
            let sid = SystemId::new(system.as_str());
            let ships = crate::combat::ships_of(&o.gs, ContentStore::embedded(), o.sources, &player, &sid);
            let index = usize::from(site.unit_index);
            let bastion = ships
                .get(index)
                .map_or(0, |unit| i64::try_from(crate::factions::bastion::extra_die(unit)).unwrap_or(0));
            // The round agents' extra die is read from the core state (a mark written in the same window is not in the oracle).
            let agent = if !combat { 0 } else { site.unit.system.map_or(0, |sys| ti4_engine_core_factions::cc_ground::borrowed::query::extra_die_for(q.s, q.k, sys, None, site.unit.owner, site.unit.unit, index, q.s.meta().combat_round_seq)) };
            bastion + agent
        })
        .unwrap_or(0))
    });
    seams.silenced.push(STRINGS, |_, site: &SilenceSite| {
        let lost = with_oracle(|o| {
            let player = o.player(site.owner)?;
            Some(crate::factions::crimson::abilities_lost(&o.gs, &player, &SystemId::new(site.system.as_str())))
        })
        .flatten()
        .unwrap_or(false);
        if lost { Verdict::Allow } else { Verdict::Abstain }
    });
    // Faction bars on SPACE CANNON (Argent flagship): `Allow` bars the shooter (`Combine::AllowWins`).
    seams.cannon_barred.push(STRINGS, |_, site: &ti4_engine_core::kernel::seam::CannonSite| {
        let barred = with_oracle(|o| {
            let (shooter, target) = (o.player(site.shooter)?, o.player(site.target)?);
            Some(crate::factions::hooks_combat::space_cannon_barred(
                &o.gs,
                ContentStore::embedded(),
                o.sources,
                &shooter,
                &target,
                &SystemId::new(site.system.as_str()),
            ))
        })
        .flatten()
        .unwrap_or(false);
        if barred { Verdict::Allow } else { Verdict::Abstain }
    });
    seams.may_sustain.push(STRINGS, |_, unit: &CombatUnit| {
        let allowed = with_oracle(|o| {
            o.with_unit(*unit, |cu| {
                crate::factions::hooks_combat::may_sustain_unported(&o.gs, ContentStore::embedded(), o.sources, cu)
            })
        })
        .flatten()
        .unwrap_or(true);
        if allowed { Verdict::Abstain } else { Verdict::Forbid }
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
        let kernel = combat_kernel(sources)?;
        let _ = scope;
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

    /// Run `f` against the core's copy of the dice stream standing at `positions`: what it recorded and
    /// where the streams stand afterwards.
    fn drawn(
        &mut self,
        seed: u64,
        positions: &crate::rng::RngPositions,
        f: impl FnOnce(Rd<'_>, &mut ti4_engine_core::state::Rng, &dyn ti4_engine_core::state::SeedSource) -> Vec<cc::FleetRoll>,
    ) -> Option<(Vec<cc::FleetRoll>, crate::rng::RngPositions)> {
        use ti4_engine_core::state::{Rng as CoreRng, RngDomain, SeedSource};
        let mut typed_pos: Vec<(RngDomain, u128)> = Vec::new();
        for (t, p) in positions {
            typed_pos.push((crate::core_sync::codec::rng::domain(&mut self.cx, t).ok()?, *p));
        }
        let seeds = crate::core_sync::codec::rng::TextSeeds::of(&self.cx);
        let mut rng = CoreRng::new(seed);
        rng.set_positions(&typed_pos, &seeds as &dyn SeedSource);
        let rolls = self.with(|rd| f(rd, &mut rng, &seeds));
        let after = crate::core_sync::codec::rng::positions(&self.cx, &rng).ok()?;
        Some((rolls, after))
    }

    /// `combat::roll_fleet` from the dice stream standing at `positions`.
    #[must_use]
    pub fn roll_fleet_raw(
        &mut self,
        seed: u64,
        positions: &crate::rng::RngPositions,
        player: &PlayerId,
        system: &SystemId,
    ) -> Option<(Vec<cc::FleetRoll>, crate::rng::RngPositions)> {
        let (seat, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        self.drawn(seed, positions, |rd, rng, seeds| cc::roll_fleet(rd, rng, seeds, seat, sys))
    }

    /// `combat::roll_barrage_side`'s dice from the dice stream standing at `positions`.
    #[must_use]
    pub fn roll_barrage_raw(
        &mut self,
        seed: u64,
        positions: &crate::rng::RngPositions,
        player: &PlayerId,
        system: &SystemId,
    ) -> Option<(Vec<cc::FleetRoll>, crate::rng::RngPositions)> {
        let (seat, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        self.drawn(seed, positions, |rd, rng, seeds| cc::roll_barrage(rd, rng, seeds, seat, sys))
    }

    /// [`Self::roll_fleet_raw`] (or the barrage when `barrage`) as the dice history it leaves: each entry as
    /// the dual compares it (`scene`: the system the host stamps on every roll).
    #[must_use]
    pub fn roll_fleet(
        &mut self,
        seed: u64,
        positions: &crate::rng::RngPositions,
        player: &PlayerId,
        system: &SystemId,
        scene: Option<&str>,
        barrage: bool,
    ) -> Option<(Vec<DiceRow>, crate::rng::RngPositions)> {
        use ti4_engine_core::state::RollPurpose;
        let (seat, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        let scene = match scene {
            Some(s) => Some(typed::<System>(s)?),
            None => None,
        };
        let (rolls, after) = if barrage {
            self.drawn(seed, positions, |rd, rng, seeds| cc::roll_barrage(rd, rng, seeds, seat, sys))?
        } else {
            self.drawn(seed, positions, |rd, rng, seeds| cc::roll_fleet(rd, rng, seeds, seat, sys))?
        };
        let mut ledger = ti4_engine_core::state::Ledger::default();
        let purpose = if barrage { RollPurpose::AntiFighterBarrage } else { RollPurpose::SpaceCombat };
        cc::record_rolls(&mut ledger, seat, scene, &rolls, purpose);
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
                Some(s) => Some(self.cx.minting(|cx, tx| tc::reroll_set(cx, tx, s)).ok()?),
                None => None,
            };
            sets.push((seat, set));
        }
        let args: Vec<_> = sets.iter().map(|(seat, set)| (*seat, set.as_ref())).collect();
        let dice = cc::round_dice(&args);
        dice.iter().map(|d| tc::die_back(&self.cx, self.cx.texts.tables(), d).ok()).collect()
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

    /// The options of the casualty question over `units`.
    #[must_use]
    pub fn casualty_options(&self, units: &[StrUnit]) -> Option<Vec<OptionRow>> {
        use ti4_engine_core::choice::combat::Ask as CAsk;
        use ti4_engine_core::choice::Ask as AnyAsk;
        let pieces: Vec<Piece> = units.iter().map(|u| self.piece(u)).collect::<Option<_>>()?;
        let opts = cc::casualty_opts(&pieces);
        let r = crate::render::Render::new(Vec::new());
        let any = AnyAsk::Combat(CAsk::AssignHit);
        Some(
            opts.iter()
                .map(|o| {
                    let o = crate::render::option(&r, &any, o);
                    (o.id, o.kind, o.label, o.payload)
                })
                .collect(),
        )
    }

    /// `non_euclidean_shielding`: hits one use of SUSTAIN DAMAGE cancels.
    #[must_use]
    pub fn cancel_per_sustain(&self, player: &PlayerId) -> Option<usize> {
        let seat = self.seat(player)?;
        Some(self.with(|rd| cc::cancel_per_sustain(rd, seat)))
    }

    /// The units of the system and of its planets after `player` sustains at `location` (the string
    /// id suffix: `space:3`, `planet:x`, `planet:x:variant:2`); unchanged when nothing sustains.
    #[must_use]
    pub fn sustained_rows(&self, player: &PlayerId, system: &SystemId, location: &str) -> Option<Vec<BoardRow>> {
        use ti4_engine_core::choice::combat::SustainSite;
        let (seat, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        let site = if let Some(i) = location.strip_prefix("space:") {
            SustainSite::Space(i.parse().ok()?)
        } else {
            let rest = location.strip_prefix("planet:")?;
            match rest.rsplit_once(":variant:") {
                Some((p, i)) => SustainSite::Planet { planet: typed(p)?, index: i.parse().ok()?, variant: true },
                None => SustainSite::Planet { planet: typed(rest)?, index: 0, variant: false },
            }
        };
        let after = self.with(|rd| cc::apply_sustain(rd, seat, sys, site));
        let core = after.as_ref().map_or(&*self.view, |(s, _)| s);
        let mut rows = Vec::new();
        let mut push = |planet: Option<&str>, p: Piece| {
            rows.push((
                planet.map(str::to_owned),
                self.player(p.owner()).map(|id| id.to_string()).unwrap_or_default(),
                p.kind().as_str().to_owned(),
                p.sustained(),
                p.galvanized(),
            ));
        };
        for p in core.board().sys(sys).units.iter() {
            push(None, *p);
        }
        for planet in cc::planets_of_sorted(core, sys).iter() {
            for p in core.board().planet(*planet).units.iter() {
                push(Some(planet.as_str()), *p);
            }
        }
        Some(rows)
    }

    /// `CombatWindow::end_payload`'s reason, losers, retreats and lost ships, as text:
    /// `(reason, losers, retreated, destroyed (player, unit))`.
    #[must_use]
    #[allow(clippy::type_complexity, reason = "the dual compares this tuple as it is")]
    pub fn end_report(
        &self,
        system: &SystemId,
        sides: [&PlayerId; 2],
        winner: Option<&PlayerId>,
        declared_draw: bool,
        fled: &[(PlayerId, Vec<String>)],
        at_start: &[(PlayerId, Vec<String>)],
    ) -> Option<(String, Vec<String>, Vec<String>, Vec<(String, String)>)> {
        use ti4_engine_core::rules::combat::end::{end_report, EndReason};
        let sys = typed::<System>(system.as_str())?;
        let seats = [self.seat(sides[0])?, self.seat(sides[1])?];
        let winner = match winner {
            Some(w) => Some(self.seat(w)?),
            None => None,
        };
        let fleets = |list: &[(PlayerId, Vec<String>)]| -> Option<Vec<(Seat, Vec<Unit>)>> {
            list.iter()
                .map(|(who, kinds)| Some((self.seat(who)?, kinds.iter().map(|k| typed::<Unit>(k)).collect::<Option<Vec<_>>>()?)))
                .collect()
        };
        let (fled, at_start) = (fleets(fled)?, fleets(at_start)?);
        let report = self.with(|rd| end_report(rd, sys, seats, winner, declared_draw, &fled, &at_start));
        let reason = match report.reason {
            EndReason::Retreat => "retreat",
            EndReason::Draw => "draw",
            EndReason::Destruction => "destruction",
            EndReason::RoundLimit => "round_limit",
        };
        let name = |s: Seat| self.player(s).map(|p| p.to_string());
        Some((
            reason.to_owned(),
            report.losers.iter().map(|s| name(*s)).collect::<Option<_>>()?,
            report.retreated.iter().map(|s| name(*s)).collect::<Option<_>>()?,
            report.destroyed.iter().map(|(s, u)| Some((name(*s)?, u.as_str().to_owned()))).collect::<Option<_>>()?,
        ))
    }

    /// `space_cannon_guns`' unit guns (local, reaching in from adjacent systems, Linkship) as unit type ids.
    /// `None` when a unit in `reaching` or the galaxy has no typed form.
    #[must_use]
    pub fn unit_cannon_guns(
        &self,
        system: &SystemId,
        active: &PlayerId,
        galaxy: Option<&ti4_content::galaxy::Galaxy>,
        reaching: &[String],
    ) -> Option<Vec<String>> {
        let (seat, sys) = (self.seat(active)?, typed::<System>(system.as_str())?);
        let reaching = reaching.iter().map(|id| typed::<Unit>(id)).collect::<Option<Vec<_>>>()?;
        let adjacency = match galaxy {
            Some(g) => Some(crate::core_adapter::adjacency_for(g)?),
            None => None,
        };
        let guns = self.with(|rd| cc::cannon_guns::unit_guns(rd, sys, seat, adjacency.as_deref(), &reaching));
        Some(guns.iter().map(|p| p.kind().as_str().to_owned()).collect())
    }

    /// `space_cannon_guns`' attachment cannons as `(owner, planet, hit number, dice)`.
    #[must_use]
    pub fn attachment_cannon_guns(&self, system: &SystemId, active: &PlayerId) -> Option<Vec<(String, String, u32, usize)>> {
        let (seat, sys) = (self.seat(active)?, typed::<System>(system.as_str())?);
        self.with(|rd| cc::cannon_guns::attachment_guns(rd, sys, seat))
            .into_iter()
            .map(|(who, planet, value, count)| Some((self.player(who)?.to_string(), planet.as_str().to_owned(), value, count)))
            .collect()
    }

    /// The barrage plan of a side: (ship type or none for the relic, hit number, dice).
    #[must_use]
    pub fn barrage_plan(&self, player: &PlayerId, system: &SystemId) -> Option<Vec<(Option<String>, i64, usize)>> {
        let (seat, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        Some(self.with(|rd| {
            cc::barrage_plan(rd, seat, sys)
                .into_iter()
                .map(|r| (r.unit.map(|u| u.as_str().to_owned()), r.hits_on, r.dice))
                .collect()
        }))
    }

    /// The per-gun roll plan of the space cannon offense over the string's gun list.
    #[must_use]
    pub fn cannon_plan(
        &self,
        system: &SystemId,
        active: &PlayerId,
        opponent: Option<&PlayerId>,
        guns: &[(PlayerId, String)],
    ) -> Option<Vec<(String, String, i64, usize)>> {
        let (sys, act) = (typed::<System>(system.as_str())?, self.seat(active)?);
        let opp = match opponent {
            Some(p) => Some(self.seat(p)?),
            None => None,
        };
        let mut list = Vec::with_capacity(guns.len());
        for (owner, unit) in guns {
            list.push((self.seat(owner)?, typed::<Unit>(unit)?));
        }
        Some(self.with(|rd| {
            cc::cannon_plan(rd, sys, act, opp, &list)
                .into_iter()
                .filter_map(|r| Some((self.player(r.owner)?.to_string(), r.unit.as_str().to_owned(), r.hits_on, r.dice)))
                .collect()
        }))
    }

    /// The units a further hit may be assigned to.
    #[must_use]
    pub fn hit_targets(&self, player: &PlayerId, system: &SystemId, ability: bool, non_fighters_first: bool) -> Option<Vec<StrUnit>> {
        let (seat, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        self.with(|rd| cc::hit_targets(rd, seat, sys, ability, non_fighters_first))
            .into_iter()
            .map(|p| self.str_unit(p))
            .collect()
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

/// A unit of a system as the dual compares it: planet (none: space), owner, type, damaged, galvanized.
pub type BoardRow = (Option<String>, String, String, bool, bool);

/// The string side of [`BoardRow`]s.
#[must_use]
pub fn board_rows(gs: &GameState, system: &SystemId) -> Vec<BoardRow> {
    let cell = gs.system_state(system);
    let mut rows: Vec<BoardRow> = cell
        .units
        .iter()
        .map(|u| (None, u.owner.to_string(), u.type_id.to_string(), u.sustained_damage, u.galvanized))
        .collect();
    for (planet, units) in &cell.planet_units {
        rows.extend(units.iter().map(|u| {
            (Some(planet.to_string()), u.owner.to_string(), u.type_id.to_string(), u.sustained_damage, u.galvanized)
        }));
    }
    rows
}

/// One pooled roll group as the dual compares it: hit number, roll tag, dice, unit types with counts.
pub type FleetGroupRow = (i64, u8, i64, Vec<(String, u32)>);

/// One dice-history entry as the dual compares it: purpose, the seat whose dice, the scene system, the
/// hit number, the faces and the dice a reroll replaced.
pub type DiceRow = (u8, Option<String>, Option<String>, Option<u32>, Vec<u32>, std::collections::BTreeSet<usize>);

fn purpose_code(p: ti4_engine_core::state::RollPurpose) -> u8 {
    use ti4_engine_core::state::RollPurpose as P;
    match p {
        P::SpaceCombat => 0,
        P::AntiFighterBarrage => 2,
        P::Reroll => 5,
        _ => 255,
    }
}

/// A string dice-history entry as the dual compares it.
#[must_use]
pub fn dice_row(r: &crate::dice::Roll) -> DiceRow {
    let purpose = if r.reason == "space combat" {
        0
    } else if r.reason == "anti_fighter_barrage" || r.reason == "anti-fighter barrage" {
        2
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

/// `combat::eligible_retreats` over the typed movement rules (package A2's reach view of the position).
#[must_use]
pub fn eligible_retreats(
    state: &GameState,
    sources: SourceSet,
    galaxy: &ti4_content::galaxy::Galaxy,
    player: &PlayerId,
    system: &SystemId,
) -> Option<Vec<SystemId>> {
    let pos = crate::core_adapter::Position::new(state, sources, galaxy)?;
    let reach = crate::core_adapter::SeatReach::new(&pos, player, false)?;
    let (seat, sys) = (pos.seat(player)?, typed::<System>(system.as_str())?);
    let rules = reach.rules_for(sys);
    let rd = Rd::new(pos.core(), pos.kernel());
    let found = cc::eligible_retreats(rd, seat, sys, &rules.adjacency, &|dest| (*reach.rules_for(dest)).clone());
    Some(found.into_iter().map(|s| SystemId::new(s.as_str())).collect())
}

/// The neighbours of `system` a fleet of `player` may retreat towards as far as the movement rules go
/// (hyperlanes left out), each with whether the rules let it enter: the map half of `eligible_retreats`
/// the composed combat machine filters by the board as it changes.
#[must_use]
pub fn retreat_neighbors(
    state: &GameState,
    sources: SourceSet,
    galaxy: &ti4_content::galaxy::Galaxy,
    player: &PlayerId,
    system: &SystemId,
) -> Option<Vec<(SystemId, bool)>> {
    let pos = crate::core_adapter::Position::new(state, sources, galaxy)?;
    let reach = crate::core_adapter::SeatReach::new(&pos, player, false)?;
    let sys = typed::<System>(system.as_str())?;
    let rules = reach.rules_for(sys);
    let mut found = Vec::new();
    for n in rules.adjacency.of(sys).iter().map(ti4_engine_core::ids::system_at) {
        if rules.adjacency.is_hyperlane(n) {
            continue;
        }
        let may = cc::retreat_may_enter(pos.core(), n, &|dest| (*reach.rules_for(dest)).clone());
        found.push((SystemId::new(n.as_str()), may));
    }
    Some(found)
}

/// Skilled Retreat's and Dark Energy Tap's destinations over the typed movement rules.
#[must_use]
pub fn open_retreat_destinations(
    state: &GameState,
    sources: SourceSet,
    galaxy: &ti4_content::galaxy::Galaxy,
    player: &PlayerId,
    system: &SystemId,
    foreign_units: bool,
) -> Option<Vec<SystemId>> {
    let pos = crate::core_adapter::Position::new(state, sources, galaxy)?;
    let reach = crate::core_adapter::SeatReach::new(&pos, player, false)?;
    let (seat, sys) = (pos.seat(player)?, typed::<System>(system.as_str())?);
    let base = pos.core().map().adjacency()?.clone();
    let rd = Rd::new(pos.core(), pos.kernel());
    let found = cc::open_retreat_destinations(rd, seat, sys, &base, &|dest| (*reach.rules_for(dest)).clone(), foreign_units);
    Some(found.into_iter().map(|s| SystemId::new(s.as_str())).collect())
}

/// The kernel the combat rules run against: the core's rows, the legacy slots' bridge, the Naaz and Nomad
/// hooks and the string bridge for the modules not ported yet.
#[must_use]
pub fn combat_kernel(sources: SourceSet) -> Option<Kernel> {
    let scope = crate::core_adapter::scope(sources);
    let mut kernel = Kernel::plain();
    providers::register_core(&mut kernel.hooks);
    adapters::bridge_legacy(&mut kernel.hooks.seam);
    cc::register_naaz_nomad(&mut kernel.hooks);
    // Transparasteel Plating: a passed player cannot play action cards on the holder's turn.
    kernel.hooks.economy.action_cards_forbidden.push(ti4_engine_core_factions::cd_registry::yssaril::query::action_cards_forbidden);
    // The Table's Grace: the Corsair stands in for the Cruiser II a Mentak seat places (Salvage Operations, Ipswitch).
    kernel.hooks.strategy.unit_form_override.push(ti4_engine_core_factions::cc_ground::mentak::query::unit_form_override);
    // Supercharge: the +1 of the round it was used in is read from the core's own mark (the string bridge below leaves it out; a
    // snapshot of the string state would carry it into the rounds after).
    kernel.hooks.seam.hit_shift.push_add(
        Source::Faction(ti4_engine_core::generated::Faction::TheNaazRokhaAlliance),
        Layer::Faction,
        ti4_engine_core_factions::cd_registry::naaz::query::supercharge,
    );
    // Naaz at the start of a space combat round: the Eidolon Maximum is repaired and Supercharge is offered.
    kernel.hooks.general.space_combat_round_started.push(ti4_engine_core_factions::cd_registry::naaz::space_round_started);
    // Mahact's Edict puts the opponents' command tokens into its fleet pool: the enforcement of the fleets that follows a win judges by the pool as
    // it is now, so the tokens must count (`driver::windows::tactical::fleet_limit`; the golden dual game 9, `aftermath::fight_over`).
    kernel.hooks.direct.mahact_foreign_tokens.push(ti4_engine_core_factions::cc_ground::mahact::query::foreign_tokens);
    // The agents' exhaustion is announced to a Nomad that listens (the round agents read it).
    kernel.hooks.direct.nomad_agents_watches_roll_step.push(ti4_engine_core_factions::ce_movement::nomad::query::watches_roll_step);
    kernel.hooks.direct.nomad_watches_agent_exhaustion.push(ti4_engine_core_factions::ce_movement::nomad::query::watches_agent_exhaustion);
    // Bastion's ship-lost bookkeeping and the Nekro Alastor queries and bookkeeping are native.
    ti4_engine_core_factions::ca_combat::bastion::register_observers(&mut kernel.hooks);
    ti4_engine_core_factions::ca_combat::nekro::register(&mut kernel.hooks);
    // The Cavalry is played inside a window (the opening of the fight): the lent stats are read from the core state, which holds
    // the choice, not from the snapshot of the position the window started in.
    ti4_engine_core_factions::ce_movement::nomad::query::register_borrowed(&mut kernel.hooks);
    // Fourth Moon: the ships of the other players cannot sustain while it stands in the system. The rule reads the board of the core
    // state it is asked in, not the snapshot of the string bridge (a flagship destroyed inside the window stops barring at once).
    kernel.hooks.seam.may_sustain.push(
        Source::Faction(ti4_engine_core::generated::Faction::TheMentakCoalition),
        ti4_engine_core_factions::cc_ground::mentak::query::may_sustain,
    );
    register_string_bridge(&mut kernel.hooks.seam);
    kernel.sources = CoreSources(u8::try_from(scope.0).ok()?);
    Some(kernel)
}

/// The kernel of a window run and the string bridge installed for as long as it lives.
pub struct WindowEnv {
    pub kernel: Kernel,
    _guard: Installed,
}

/// The environment for driving a combat window from the position `gs` (the state before the string ran).
#[must_use]
pub fn window_env(gs: &GameState, cx: &Ctx, sources: SourceSet) -> Option<WindowEnv> {
    let kernel = combat_kernel(sources)?;
    let owners = (0..16_u8).map(|i| cx.owners.id(Seat(i)).map(str::to_owned)).collect();
    let oracle = std::rc::Rc::new(Oracle { gs: gs.clone(), sources, owners });
    Some(WindowEnv { kernel, _guard: Installed::new(oracle) })
}

/// The `sustainers` detail of a sustain question, from the core state it is asked in.
#[must_use]
pub fn sustainers_detail(kernel: &Kernel, core: &ti4_engine_core::state::CoreState, ask: &ti4_engine_core::choice::combat::Ask, actor: Seat) -> serde_json::Value {
    let ti4_engine_core::choice::combat::Ask::CancelHit { system, in_combat, ability, staging, .. } = *ask else {
        return serde_json::Value::Null;
    };
    let rd = Rd::new(core, kernel).with_blanked(ti4_engine_core::query::reach::blanked(core));
    let available: Vec<(bool, Piece)> = cc::sustain_candidates(rd, actor, system, in_combat, ability, staging)
        .into_iter()
        .map(|(site, p)| (matches!(site, ti4_engine_core::choice::combat::SustainSite::Planet { .. }), p))
        .collect();
    let counts = cc::sustainer_counts(rd, system, actor, in_combat, &available);
    let map: std::collections::BTreeMap<String, u64> = counts.into_iter().map(|(u, n)| (u.as_str().to_owned(), n)).collect();
    serde_json::to_value(map).unwrap_or(serde_json::Value::Null)
}
