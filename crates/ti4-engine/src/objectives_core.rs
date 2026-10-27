//! The objectives area on the typed core (package A4): the adapter between the string engine's
//! `objectives` / `secrets` / `Observed` call sites and `ti4_engine_core::query::objectives`.
//!
//! A position that wants the typed answers carries an [`ObjView`]: the slices the requirements read
//! (seats, players, board, agenda, objectives, and the map when the position has a galaxy), loaded
//! once, plus a per-seat scan ([`SeatFacts`]). The view belongs to one `Observed` (one position, which
//! cannot change while it is borrowed) and is lent to the string functions through a thread-local
//! scope ([`scoped`]), so `Position` constructors and the many call sites that build one stay as they
//! are. A function called outside a scope, with a state the view was not built from, with another
//! galaxy or source scope, or with an id the core has no variant for, answers from the string engine.
//!
//! Hook terms ([`Terms`]) are computed here with the string engine, per seat, once.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(not(feature = "typed-core"))]
mod stub {
    use std::rc::Rc;

    /// Without `typed-core` there is no view.
    #[derive(Debug)]
    pub struct ObjView;
    /// Without `typed-core` nothing is converted.
    #[derive(Debug, Clone, Default)]
    pub struct TypedImagined;

    impl ObjView {
        #[must_use]
        pub fn build(
            _: &ti4_model::state::GameState,
            _: &ti4_content::ContentStore,
            _: ti4_model::content_types::SourceSet,
            _: Option<&ti4_content::galaxy::Galaxy>,
        ) -> Option<Rc<Self>> {
            None
        }
    }

    pub fn scoped<T>(_: Option<&Rc<ObjView>>, f: impl FnOnce() -> T) -> T {
        f()
    }

    /// Without `typed-core` the string engine always answers.
    #[must_use]
    pub fn secret_typed(
        _: &crate::secrets::Position<'_>,
        _: &ti4_model::id::SecretObjectiveId,
        _: bool,
    ) -> Option<Option<crate::objectives::RequirementProgress>> {
        None
    }
}

#[cfg(feature = "typed-core")]
mod real {
    use std::cell::RefCell;
    use std::rc::Rc;

    use ti4_content::ContentStore;
    use ti4_content::galaxy::Galaxy;
    use ti4_engine_core::bits::SysSet;
    use ti4_engine_core::containers::SmallList;
    use ti4_engine_core::generated::{FactionTag, Planet, PlanetTrait, PublicObjective, SecretObjective, System, Technology, UnitBaseType};
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::query::economy::Spend as CoreSpend;
    use ti4_engine_core::query::objectives as q;
    use ti4_engine_core::state::{CoreState, Slice, SliceSet};
    use ti4_engine_core::tables::Scope;
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::{ObjectiveId, PlayerId};
    use ti4_model::state::GameState;

    use crate::core_sync::{Codec, Ctx, Sources};
    use crate::generated::IdStr;
    use crate::objectives::{CostFamily, CostProgress, CountFamily, RequirementProgress};
    use crate::production::Spend;

    /// The typed view of one position (see the module docs).
    #[derive(Debug)]
    pub struct ObjView {
        state_addr: usize,
        galaxy_addr: Option<usize>,
        sources: SourceSet,
        scope: Scope,
        core: CoreState,
        ctx: Ctx,
        edge: SysSet,
        facts: RefCell<[Option<Rc<q::SeatFacts>>; 16]>,
    }

    thread_local! {
        static AMBIENT: RefCell<Option<Rc<ObjView>>> = const { RefCell::new(None) };
    }

    /// Run `f` with `view` lent to every objectives call on this thread (restored afterwards).
    pub fn scoped<T>(view: Option<&Rc<ObjView>>, f: impl FnOnce() -> T) -> T {
        struct Restore(Option<Rc<ObjView>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                AMBIENT.with(|a| *a.borrow_mut() = self.0.take());
            }
        }
        let Some(view) = view else { return f() };
        let _restore = Restore(AMBIENT.with(|a| a.borrow_mut().replace(Rc::clone(view))));
        f()
    }

    fn ambient() -> Option<Rc<ObjView>> {
        AMBIENT.with(|a| a.borrow().clone())
    }

    /// The core's scope of a source set.
    #[must_use]
    pub fn scope_of(sources: SourceSet) -> Scope {
        crate::core_adapter::scope(sources)
    }

    impl ObjView {
        /// Load the slices the requirements read. `None` when the typed state cannot hold the
        /// position (an unknown id, more than 16 owners) or the content is not the compiled-in corpus.
        #[must_use]
        pub fn build(
            state: &GameState,
            content: &ContentStore,
            sources: SourceSet,
            galaxy: Option<&Galaxy>,
        ) -> Option<Rc<Self>> {
            if !content.is_embedded() {
                return None;
            }
            let mut cx = Ctx::of(state).ok()?;
            let mut slices = SliceSet::of(&[Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Objectives]);
            if galaxy.is_some() {
                slices.insert(Slice::Map);
            }
            let core = Codec::load(&mut cx, state, Sources { galaxy, rng: None }, slices).ok()?;
            let edge = if galaxy.is_some() { q::edge_systems(&core) } else { SysSet::EMPTY };
            Some(Rc::new(Self {
                state_addr: std::ptr::from_ref(state) as usize,
                galaxy_addr: galaxy.map(|g| std::ptr::from_ref(g) as usize),
                sources,
                scope: scope_of(sources),
                core,
                ctx: cx,
                edge,
                facts: RefCell::new(std::array::from_fn(|_| None)),
            }))
        }

        /// Whether this view answers for exactly this position.
        fn matches(&self, state: &GameState, sources: SourceSet, galaxy: Option<&Galaxy>) -> bool {
            self.state_addr == std::ptr::from_ref(state) as usize
                && self.sources == sources
                // A position with a galaxy needs the view built from the same one; without, the map
                // flag turns the map-shaped requirements off.
                && galaxy.is_none_or(|g| self.galaxy_addr == Some(std::ptr::from_ref(g) as usize))
        }

        #[must_use]
        pub const fn core(&self) -> &CoreState {
            &self.core
        }

        #[must_use]
        pub fn seat_of(&self, player: &PlayerId) -> Option<Seat> {
            self.ctx.owners.seat(player.as_str())
        }

        fn facts(&self, seat: Seat, state: &GameState, content: &ContentStore, sources: SourceSet) -> Rc<q::SeatFacts> {
            let slot = usize::from(seat.0);
            if let Some(f) = &self.facts.borrow()[slot] {
                return Rc::clone(f);
            }
            let player = self.ctx.owners.id(seat).map(PlayerId::new);
            let terms = player.as_ref().map(|p| terms_of(state, content, sources, p, self)).unwrap_or_default();
            let f = Rc::new(q::SeatFacts::of(&self.core, self.scope, seat, terms));
            self.facts.borrow_mut()[slot] = Some(Rc::clone(&f));
            f
        }
    }

    /// The hook answers for one seat, from the string engine.
    fn terms_of(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, view: &ObjView) -> q::Terms {
        let mut t = q::Terms::default();
        for p in &state.players {
            if crate::factions::keleres::has_iihq(state, &p.id)
                && let Some(seat) = view.seat_of(&p.id)
            {
                t.iihq.insert(seat);
            }
        }
        t.blanked = crate::laws::blanked(state);
        t.faction_tag = state.player(player).and_then(|p| <FactionTag as IdStr>::from_id(p.faction.as_str()));
        for planet in crate::factions::borrowed_commanders_b::firmament_planets(state, content, sources, player) {
            match <Planet as IdStr>::from_id(planet.as_str()) {
                Some(p) => {
                    t.firmament.insert(p);
                }
                None => t.unmapped = true,
            }
        }
        t.per_good = crate::factions::hooks_economy::trade_good_worth(state, player, 1).max(1);
        t.commodities = crate::factions::keleres::spendable_commodities(state, player);
        t.oceans = !crate::factions::deepwrought::spendable_oceans(state, player).is_empty();
        for (_, planet) in state.controlled_planets(player) {
            let bonus = crate::factions::bastion_units::resource_bonus(state, planet);
            if bonus != 0
                && let Some(p) = <Planet as IdStr>::from_id(planet.as_str())
            {
                t.helios.push((p, bonus));
            }
        }
        t
    }

    /// The imagined sets of an option in typed form.
    #[derive(Debug, Clone, Default)]
    pub struct TypedImagined {
        planets: SmallList<Planet, 8>,
        systems: SmallList<System, 8>,
        technologies: SmallList<Technology, 4>,
        structures: SmallList<(System, Planet), 4>,
    }

    impl TypedImagined {
        /// `None` when an id has no typed variant.
        #[must_use]
        pub fn from_ids<'a>(
            planets: impl IntoIterator<Item = &'a str>,
            systems: impl IntoIterator<Item = &'a str>,
            technologies: impl IntoIterator<Item = &'a str>,
            structures: impl IntoIterator<Item = (&'a str, &'a str)>,
        ) -> Option<Self> {
            let mut out = Self::default();
            for p in planets {
                out.planets.push(<Planet as IdStr>::from_id(p)?);
            }
            for s in systems {
                out.systems.push(<System as IdStr>::from_id(s)?);
            }
            for t in technologies {
                out.technologies.push(<Technology as IdStr>::from_id(t)?);
            }
            for (s, p) in structures {
                out.structures.push((<System as IdStr>::from_id(s)?, <Planet as IdStr>::from_id(p)?));
            }
            Some(out)
        }

        #[must_use]
        pub fn get(&self) -> q::Imagined<'_> {
            q::Imagined {
                planets: self.planets.as_slice(),
                systems: self.systems.as_slice(),
                technologies: self.technologies.as_slice(),
                structures: self.structures.as_slice(),
            }
        }
    }

    /// Run `f` on the typed position of `player`, or `None` when no view answers for this one.
    pub fn with_pos<T>(
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        galaxy: Option<&Galaxy>,
        im: &TypedImagined,
        f: impl FnOnce(&q::Pos<'_>, &ObjView) -> T,
    ) -> Option<T> {
        let view = ambient()?;
        if !view.matches(state, sources, galaxy) {
            return None;
        }
        let seat = view.seat_of(player)?;
        // The map half of a requirement needs the core's galaxy when the position has one.
        if galaxy.is_some() && view.core.galaxy().is_none() {
            return None;
        }
        let facts = view.facts(seat, state, content, sources);
        let pos = q::Pos::new(&view.core, &facts, galaxy.is_some(), im.get());
        Some(f(&pos, &view))
    }

    // --- results in the string engine's types ------------------------------------------------------

    fn spend(s: CoreSpend) -> Spend {
        match s {
            CoreSpend::Resources => Spend::Resources,
            CoreSpend::Influence => Spend::Influence,
        }
    }

    fn base_name(b: UnitBaseType) -> &'static str {
        match b {
            UnitBaseType::Carrier => "carrier",
            UnitBaseType::Cruiser => "cruiser",
            UnitBaseType::Destroyer => "destroyer",
            UnitBaseType::Dreadnought => "dreadnought",
            UnitBaseType::Fighter => "fighter",
            UnitBaseType::Flagship => "flagship",
            UnitBaseType::Infantry => "infantry",
            UnitBaseType::Mech => "mech",
            UnitBaseType::Pds => "pds",
            UnitBaseType::Spacedock => "spacedock",
            UnitBaseType::Warsun => "warsun",
        }
    }

    fn trait_name(t: PlanetTrait) -> &'static str {
        match t {
            PlanetTrait::Cultural => "cultural",
            PlanetTrait::Hazardous => "hazardous",
            PlanetTrait::Industrial => "industrial",
            PlanetTrait::Faction => "faction",
            PlanetTrait::Fake => "fake",
            PlanetTrait::Lightning => "lightning",
            PlanetTrait::Mr => "mr",
            PlanetTrait::Spacestation => "spacestation",
        }
    }

    #[must_use]
    pub fn family(f: q::Family) -> CountFamily {
        use q::Family as F;
        match f {
            F::NonHome => CountFamily::NonHome,
            F::OnTheRim => CountFamily::OnTheRim,
            F::SameTrait => CountFamily::SameTrait,
            F::TechSpecialties => CountFamily::TechSpecialties,
            F::UnitUpgrades => CountFamily::UnitUpgrades,
            F::Colours { per_colour } => CountFamily::Colours { per_colour },
            F::Structures => CountFamily::Structures,
            F::StructuresAway => CountFamily::StructuresAway,
            F::FleetInOneSystem => CountFamily::FleetInOneSystem,
            F::PlanetlessSystems => CountFamily::PlanetlessSystems,
            F::AttachedPlanets => CountFamily::AttachedPlanets,
            F::NotableSystems => CountFamily::NotableSystems,
            F::GroundForcesOnOnePlanet => CountFamily::GroundForcesOnOnePlanet,
            F::MechsOnDistinctPlanets => CountFamily::MechsOnDistinctPlanets,
            F::PlanetsOfTrait(t) => CountFamily::PlanetsOfTrait { trait_name: trait_name(t) },
            F::SameColourTechnologies => CountFamily::SameColourTechnologies,
            F::ShipsInSystems => CountFamily::ShipsInSystems,
            F::Units(b) => CountFamily::Units { base_type: base_name(b) },
            F::RivalHomePlanets => CountFamily::RivalHomePlanets,
            F::CapitalShipSystems => CountFamily::CapitalShipSystems,
            F::CapitalShipsInRivalHomeOrMecatol => CountFamily::CapitalShipsInRivalHomeOrMecatol,
            F::ShipsAdjacentToMecatol => CountFamily::ShipsAdjacentToMecatol,
            F::WeakerNeighbours => CountFamily::WeakerNeighbours,
            F::DistinctRivalHomeReaches => CountFamily::DistinctRivalHomeReaches,
            F::RivalDockSystemsWithShips => CountFamily::RivalDockSystemsWithShips,
            F::RelicFragments => CountFamily::RelicFragments,
            F::ActionCards => CountFamily::ActionCards,
            F::RivalNoteIssuers => CountFamily::RivalNoteIssuers,
            F::ShipSystemsBesideAnomaly => CountFamily::ShipSystemsBesideAnomaly,
            F::ShipSystemsBesideRivalHome => CountFamily::ShipSystemsBesideRivalHome,
            F::Neighbours => CountFamily::Neighbours,
            F::UnitsInNexus => CountFamily::UnitsInNexus,
            F::WormholeKinds => CountFamily::WormholeKinds,
            F::FactionTechnologies => CountFamily::FactionTechnologies,
            F::Laws => CountFamily::Laws,
            F::SharedPlanetSystems => CountFamily::SharedPlanetSystems,
            F::ProductionInOneSystem => CountFamily::ProductionInOneSystem,
            F::LegendaryPlanets => CountFamily::LegendaryPlanets,
            F::MecatolShipsWhileControlling => CountFamily::MecatolShipsWhileControlling,
            F::CombinedValue(s) => CountFamily::CombinedValue { kind: spend(s) },
        }
    }

    #[must_use]
    pub fn progress(p: q::Progress) -> RequirementProgress {
        RequirementProgress { family: family(p.family), have: p.have, threshold: p.threshold }
    }

    #[must_use]
    pub fn cost_progress(p: q::CostProgress) -> CostProgress {
        CostProgress {
            family: match p.family {
                q::CostFamily::Spend(s) => CostFamily::Spend(spend(s)),
                q::CostFamily::TradeGoods => CostFamily::TradeGoods,
                q::CostFamily::Tokens => CostFamily::Tokens,
                q::CostFamily::AllThree => CostFamily::AllThree,
            },
            have: p.have,
            target: p.target,
        }
    }

    /// The typed variant of a public objective alias; `None` for any other id.
    #[must_use]
    pub fn public(alias: &ObjectiveId) -> Option<PublicObjective> {
        <PublicObjective as IdStr>::from_id(alias.as_str())
    }

    // --- the answers of the public objectives --------------------------------------------------------

    /// `objectives::counting_progress` (outer `None`: not answered here).
    pub fn counting(
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        galaxy: Option<&Galaxy>,
        im: &TypedImagined,
        alias: &ObjectiveId,
    ) -> Option<Option<RequirementProgress>> {
        let Some(obj) = public(alias) else {
            // Not a public objective of the corpus: the string table has no arm for it either.
            return ambient().is_some().then_some(None);
        };
        with_pos(state, content, sources, player, galaxy, im, |pos, view| {
            q::counting_progress(pos, &view.edge, obj).map(progress)
        })
    }

    /// `objectives::remaining_position_progress`.
    pub fn remaining(
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        galaxy: Option<&Galaxy>,
        im: &TypedImagined,
        alias: &ObjectiveId,
    ) -> Option<Option<RequirementProgress>> {
        let Some(obj) = public(alias) else {
            return ambient().is_some().then_some(None);
        };
        with_pos(state, content, sources, player, galaxy, im, |pos, _| q::remaining_position_progress(pos, obj).map(progress))
    }

    /// `objectives::bought_progress_at`. Outer `None` also while a hook holds cards the core cannot
    /// express (the answer is then the string engine's).
    pub fn bought(
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        galaxy: Option<&Galaxy>,
        im: &TypedImagined,
        alias: &ObjectiveId,
    ) -> Option<Option<CostProgress>> {
        let Some(obj) = public(alias) else {
            return ambient().is_some().then_some(None);
        };
        if q::cost_of(obj).is_none() {
            return ambient().is_some().then_some(None);
        }
        with_pos(state, content, sources, player, galaxy, im, |pos, _| q::bought_progress(pos, obj).map(cost_progress))?
            .map(Some)
    }

    /// `Observed::revealed_objective_progress_imagining` whole, from the core (`None`: the string
    /// engine answers).
    pub fn revealed_progress(
        view: &Rc<ObjView>,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        galaxy: Option<&Galaxy>,
        player: &PlayerId,
        imagined: &crate::objectives::Imagined<'_>,
    ) -> Option<Vec<crate::objectives::CardProgress>> {
        if !view.matches(state, sources, galaxy) || (galaxy.is_some() && view.core.galaxy().is_none()) {
            return None;
        }
        let seat = view.seat_of(player)?;
        let im = TypedImagined::from_ids(
            imagined.planets.iter().map(ti4_model::id::PlanetId::as_str),
            imagined.systems.iter().map(String::as_str),
            imagined.technologies.iter().map(String::as_str),
            imagined.structures.iter().map(|(s, p)| (s.as_str(), p.as_str())),
        )?;
        let facts = view.facts(seat, state, content, sources);
        let pos = q::Pos::new(&view.core, &facts, galaxy.is_some(), im.get());
        let mut out = Vec::new();
        for r in view.core.objectives().revealed.iter() {
            // A secret made public has no public requirement and no price: no card.
            let ti4_engine_core::state::ObjectiveRef::Public(obj) = r else { continue };
            let obj = *obj;
            let stage = q::stage_of(obj);
            if let Some(p) = q::counting_progress(&pos, &view.edge, obj).or_else(|| q::remaining_position_progress(&pos, obj)) {
                #[expect(clippy::cast_precision_loss, reason = "small integer counts")]
                let (have, threshold) = (p.have as f64, p.threshold as f64);
                out.push(crate::objectives::CardProgress {
                    alias: obj.as_str().to_owned(),
                    family_token: crate::objectives::family_token(&family(p.family)),
                    have,
                    threshold,
                    satisfied: p.satisfied(),
                    stage,
                });
                continue;
            }
            if q::cost_of(obj).is_none() {
                continue;
            }
            // A priced objective the core cannot answer (ocean cards) is the string engine's.
            let c = q::bought_progress(&pos, obj)?;
            #[expect(clippy::cast_precision_loss, reason = "small integer amounts")]
            let (have, threshold) = (c.have as f64, c.target as f64);
            let c = cost_progress(c);
            out.push(crate::objectives::CardProgress {
                alias: obj.as_str().to_owned(),
                family_token: crate::objectives::cost_family_token(&c.family),
                have,
                threshold,
                satisfied: c.satisfied(),
                stage,
            });
        }
        Some(out)
    }

    // --- the secrets --------------------------------------------------------------------------------

    fn counting_secret_family(f: q::Family) -> bool {
        use q::Family as F;
        matches!(
            f,
            F::Units(_)
                | F::ShipsInSystems
                | F::PlanetsOfTrait(_)
                | F::SameColourTechnologies
                | F::MechsOnDistinctPlanets
                | F::GroundForcesOnOnePlanet
        )
    }

    fn typed_secret_imagined(imagined: &crate::objectives::Imagined<'_>) -> Option<TypedImagined> {
        TypedImagined::from_ids(
            imagined.planets.iter().map(ti4_model::id::PlanetId::as_str),
            imagined.systems.iter().map(String::as_str),
            imagined.technologies.iter().map(String::as_str),
            imagined.structures.iter().map(|(s, p)| (s.as_str(), p.as_str())),
        )
    }

    /// `secrets::counting_progress` (`counting`) or `secrets::remaining_position_progress` (the
    /// rest). Outer `None`: the string engine answers.
    #[allow(clippy::too_many_arguments)]
    pub fn secret(
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
        galaxy: Option<&Galaxy>,
        imagined: &crate::objectives::Imagined<'_>,
        alias: &ti4_model::id::SecretObjectiveId,
        counting: bool,
    ) -> Option<Option<RequirementProgress>> {
        let Some(obj) = <SecretObjective as IdStr>::from_id(alias.as_str()) else {
            return ambient().is_some().then_some(None);
        };
        let im = typed_secret_imagined(imagined)?;
        with_pos(state, content, sources, player, galaxy, &im, |pos, _| {
            match q::secret_progress(pos, obj) {
                Ok(p) => Some(p.filter(|p| counting_secret_family(p.family) == counting).map(progress)),
                Err(q::Unsupported) => None,
            }
        })?
    }

    /// `held_secret_records` whole, from the core.
    pub fn held_secrets(
        view: &Rc<ObjView>,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        galaxy: Option<&Galaxy>,
        player: &PlayerId,
        imagined: &crate::objectives::Imagined<'_>,
    ) -> Option<Vec<crate::objectives::CardProgress>> {
        if !view.matches(state, sources, galaxy) || (galaxy.is_some() && view.core.galaxy().is_none()) {
            return None;
        }
        let seat = view.seat_of(player)?;
        let im = typed_secret_imagined(imagined)?;
        let facts = view.facts(seat, state, content, sources);
        let pos = q::Pos::new(&view.core, &facts, galaxy.is_some(), im.get());
        let hand = &view.core.players().by_seat(seat)?.cards.secrets;
        let mut out = Vec::new();
        for obj in hand.as_slice() {
            match q::secret_progress(&pos, *obj) {
                Ok(Some(p)) => {
                    #[expect(clippy::cast_precision_loss, reason = "small integer counts")]
                    let (have, threshold) = (p.have as f64, p.threshold as f64);
                    out.push(crate::objectives::CardProgress {
                        alias: obj.as_str().to_owned(),
                        family_token: crate::objectives::family_token(&family(p.family)),
                        have,
                        threshold,
                        satisfied: p.satisfied(),
                        stage: None,
                    });
                }
                Ok(None) => {}
                Err(q::Unsupported) => return None,
            }
        }
        Some(out)
    }

    /// [`secret`] for a secrets `Position`.
    #[must_use]
    pub fn secret_typed(
        p: &crate::secrets::Position<'_>,
        alias: &ti4_model::id::SecretObjectiveId,
        counting: bool,
    ) -> Option<Option<RequirementProgress>> {
        secret(p.state, p.content, p.sources, p.player, p.galaxy, &p.imagined, alias, counting)
    }
}
