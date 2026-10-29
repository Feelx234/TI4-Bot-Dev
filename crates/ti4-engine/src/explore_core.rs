//! `Area::ExplorationRelics` (package B5): the typed relic, promissory-note, exploration, legendary
//! and Fracture reads of `ti4_engine_core::query::{relic, promissory, explore}` behind the string
//! engine's interface.
//!
//! **One view per position.** A [`CardsView`] is the typed state of one position (the `Meta`, `Seats`,
//! `Players`, `Board` and `Cards` slices, loaded once). `Observed` builds it on first use and lends it
//! through a thread-local scope ([`scoped`]); every string function of the area asks [`with`], which
//! answers from the lent view when it was built from the very state being asked about. Outside a
//! scope the string engine answers (a view per call would cost more than the string read), except
//! on the `Dual` backend, where a view is built for the call so the golden games compare every call
//! site. An id the tables do not know makes a method answer `None`: the caller keeps the string answer.
//!
//! The static reads (Fracture systems, exploration traits, `is_play_area`) need no view, only the
//! compiled-in corpus; they are skipped for a custom content store.

/// `cards_typed!(state, v => expr)`: ask the typed core (an `Option`); `None` without `typed-core`.
#[macro_export]
macro_rules! cards_typed {
    ($state:expr, $v:ident => $e:expr) => {{
        #[cfg(feature = "typed-core")]
        {
            $crate::explore_core::with($state, |$v| $e)
        }
        #[cfg(not(feature = "typed-core"))]
        {
            let _ = $state;
            None
        }
    }};
}

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(not(feature = "typed-core"))]
mod stub {
    use std::rc::Rc;

    /// Without `typed-core` there is no view.
    #[derive(Debug)]
    pub struct CardsView;

    impl CardsView {
        #[must_use]
        pub fn build(_: &ti4_model::state::GameState) -> Option<Rc<Self>> {
            None
        }
    }

    pub fn scoped<T>(_: Option<&Rc<CardsView>>, f: impl FnOnce() -> T) -> T {
        f()
    }

    // The static reads: the string engine always answers.
    use ti4_content::ContentStore;
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::{PlanetId, SystemId};

    #[must_use]
    pub fn traits_of(_: &ContentStore, _: SourceSet, _: &PlanetId) -> Option<Vec<String>> {
        None
    }
    #[must_use]
    pub fn is_fracture_system(_: &ContentStore, _: SourceSet, _: &SystemId) -> Option<bool> {
        None
    }
    #[must_use]
    pub fn fracture_systems(_: &ContentStore, _: SourceSet) -> Option<Vec<SystemId>> {
        None
    }
    #[must_use]
    pub fn egress_systems(_: &ContentStore, _: SourceSet) -> Option<Vec<SystemId>> {
        None
    }
    #[must_use]
    pub fn is_play_area(_: &ContentStore, _: &str) -> Option<bool> {
        None
    }
    #[must_use]
    pub fn resolution(_: &ContentStore, _: &str) -> Option<Option<String>> {
        None
    }
    #[must_use]
    pub fn attachment_for(_: &ContentStore, _: &str, _: &PlanetId) -> Option<String> {
        None
    }
}

#[cfg(feature = "typed-core")]
mod real {
    use std::cell::RefCell;
    use std::rc::Rc;

    use ti4_content::ContentStore;
    use ti4_engine_core::generated::{Attachment, Explore, ExploreType, Faction, Planet, PromissoryNote, Relic, System};
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::query::{explore, promissory, relic};
    use ti4_engine_core::state::{NoteKey, Slice, SliceSet};
    use ti4_engine_core::tables::Scope;
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::{ActionCardId, PlanetId, PlayerId, RelicId, SystemId};
    use ti4_model::state::GameState;

    use crate::core_sync::keys::text::{note_text, parse_note};
    use crate::core_sync::{CoreView, Ctx, Sources};
    use crate::core_adapter::typed;
use crate::generated::IdStr;

    /// What the reads need.
    pub const READS: SliceSet = SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Cards]);

    /// The typed view of one position (see the module docs).
    #[derive(Debug)]
    pub struct CardsView {
        state_addr: usize,
        view: CoreView,
        cx: Ctx,
    }

    thread_local! {
        static AMBIENT: RefCell<Option<Rc<CardsView>>> = const { RefCell::new(None) };
    }

    /// Run `f` with `view` lent to every call of the area on this thread (restored afterwards).
    pub fn scoped<T>(view: Option<&Rc<CardsView>>, f: impl FnOnce() -> T) -> T {
        struct Restore(Option<Rc<CardsView>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                AMBIENT.with(|a| *a.borrow_mut() = self.0.take());
            }
        }
        let Some(view) = view else { return f() };
        let _restore = Restore(AMBIENT.with(|a| a.borrow_mut().replace(Rc::clone(view))));
        f()
    }

    /// Ask the typed core about `state`: through the lent view when it belongs to `state`, through a
    /// view built for the call on the `Dual` backend, else `None` (the string engine answers).
    pub fn with<T>(state: &GameState, f: impl FnOnce(&CardsView) -> Option<T>) -> Option<T> {
        let addr = std::ptr::from_ref(state) as usize;
        if let Some(view) = AMBIENT.with(|a| a.borrow().clone()) {
            if view.state_addr == addr {
                return f(&view);
            }
        }
        if crate::dual::backend(crate::dual::Area::ExplorationRelics) != crate::dual::Backend::Dual {
            return None;
        }
        f(&CardsView::of(state)?)
    }


    /// The string key of a fragment trait (`relic_fragments`).
    fn trait_key(kind: ExploreType) -> &'static str {
        match kind {
            ExploreType::Cultural => "CULTURAL",
            ExploreType::Frontier => "FRONTIER",
            ExploreType::Hazardous => "HAZARDOUS",
            ExploreType::Industrial => "INDUSTRIAL",
        }
    }

    impl CardsView {
        /// The view of a position; `None` when it cannot be expressed (an unknown id, more than 16
        /// owners).
        #[must_use]
        pub fn of(state: &GameState) -> Option<Self> {
            Self::try_of(state).ok()
        }

        /// [`Self::of`] with the reason a position cannot be expressed.
        ///
        /// # Errors
        /// A value the typed state cannot hold.
        pub fn try_of(state: &GameState) -> Result<Self, crate::core_sync::Unmapped> {
            let mut cx = Ctx::of(state)?;
            let view = CoreView::of(state, &mut cx, Sources { galaxy: None, rng: None }, READS)?;
            Ok(Self { state_addr: std::ptr::from_ref(state) as usize, view, cx })
        }

        /// [`Self::of`], shared (what `Observed` keeps).
        #[must_use]
        pub fn build(state: &GameState) -> Option<Rc<Self>> {
            Self::of(state).map(Rc::new)
        }

        /// The core state, for the bridge harness.
        #[must_use]
        pub fn core(&self) -> &ti4_engine_core::state::CoreState {
            &self.view
        }

        fn seat(&self, p: &PlayerId) -> Option<Seat> {
            self.cx.owners.seat(p.as_str())
        }

        fn player(&self, s: Seat) -> Option<PlayerId> {
            self.cx.owners.id(s).map(PlayerId::new)
        }

        fn text(key: NoteKey) -> String {
            format!("{}:{}", note_text(key.note), key.faction.as_str())
        }

        fn texts(keys: Vec<NoteKey>) -> Vec<String> {
            let mut out: Vec<String> = keys.into_iter().map(Self::text).collect();
            out.sort();
            out
        }

        /// `exploration::Holdings::of`.
        #[must_use]
        pub fn holdings(&self, player: &PlayerId, planet: Option<&PlanetId>) -> Option<crate::exploration::Holdings> {
            let planet = match planet {
                Some(p) => Some(typed::<Planet>(p.as_str())?),
                None => None,
            };
            Some(crate::exploration::Holdings::from_core(explore::holdings(&self.view, self.seat(player)?, planet)))
        }

        // -- relics ----------------------------------------------------------------------------

        /// `relics::holds`.
        #[must_use]
        pub fn holds(&self, player: &PlayerId, r: &RelicId) -> Option<bool> {
            Some(relic::holds(&self.view, self.seat(player)?, typed::<Relic>(r.as_str())?))
        }

        /// `relics::ready`.
        #[must_use]
        pub fn ready(&self, player: &PlayerId, r: &str) -> Option<bool> {
            Some(relic::ready(&self.view, self.seat(player)?, typed::<Relic>(r)?))
        }

        /// `relics::triad_value`.
        #[must_use]
        pub fn triad_value(&self, player: &PlayerId) -> Option<Option<i64>> {
            Some(relic::triad_value(&self.view, self.seat(player)?))
        }

        /// `relics::substitutes_strategy_token`.
        #[must_use]
        pub fn substitutes_strategy_token(&self, player: &PlayerId) -> Option<bool> {
            Some(relic::substitutes_strategy_token(&self.view, self.seat(player)?))
        }

        /// `relics::grants_sustain`.
        #[must_use]
        pub fn grants_sustain(&self, player: &PlayerId) -> Option<bool> {
            Some(relic::grants_sustain(&self.view, self.seat(player)?))
        }

        /// `relics::extra_barrage`.
        #[must_use]
        pub fn extra_barrage(&self, player: &PlayerId) -> Option<Option<(u32, usize)>> {
            Some(relic::extra_barrage(&self.view, self.seat(player)?))
        }

        /// `relics::space_dock_cannon`.
        #[must_use]
        pub fn space_dock_cannon(&self, player: &PlayerId) -> Option<Option<(i64, i64)>> {
            Some(relic::space_dock_cannon(&self.view, self.seat(player)?))
        }

        /// `relics::prerequisite_waivers`.
        #[must_use]
        pub fn prerequisite_waivers(&self, player: &PlayerId) -> Option<usize> {
            Some(relic::prerequisite_waivers(&self.view, self.seat(player)?))
        }

        /// `relics::secret_objective_bonus`.
        #[must_use]
        pub fn secret_objective_bonus(&self, player: &PlayerId) -> Option<usize> {
            Some(relic::secret_objective_bonus(&self.view, self.seat(player)?))
        }

        /// `relics::ignores_gravity_rifts`.
        #[must_use]
        pub fn ignores_gravity_rifts(&self, player: &PlayerId) -> Option<bool> {
            Some(relic::ignores_gravity_rifts(&self.view, self.seat(player)?))
        }

        // -- exploration and legendary planets ---------------------------------------------------

        /// `exploration::purgeable`.
        #[must_use]
        pub fn purgeable(&self, player: &PlayerId) -> Option<Vec<String>> {
            Some(explore::purgeable(&self.view, self.seat(player)?).as_slice().iter().map(|k| trait_key(*k).to_owned()).collect())
        }

        /// `legendary::available`.
        #[must_use]
        pub fn legendary_available(&self, player: &PlayerId, planet: &PlanetId) -> Option<bool> {
            Some(explore::legendary_available(&self.view, self.seat(player)?, typed::<Planet>(planet.as_str())?))
        }

        /// `legendary::ionian_available`.
        #[must_use]
        pub fn ionian_available(&self, player: &PlayerId) -> Option<bool> {
            Some(explore::ionian_available(&self.view, self.seat(player)?))
        }

        /// `legendary::salvaged`.
        #[must_use]
        pub fn salvaged(&self, player: &PlayerId) -> Option<Vec<ActionCardId>> {
            Some(explore::salvaged(&self.view, self.seat(player)?).iter().map(|c| ActionCardId::new(c.as_str())).collect())
        }

        // -- promissory notes --------------------------------------------------------------------

        /// `promissory::held_by`, in text order.
        #[must_use]
        pub fn held_by(&self, player: &PlayerId) -> Option<Vec<String>> {
            Some(Self::texts(promissory::held_by(&self.view, self.seat(player)?)))
        }

        /// `promissory::holder_of`.
        #[must_use]
        pub fn holder_of(&self, alias: &str, owner: &PlayerId) -> Option<Option<PlayerId>> {
            let holder = promissory::holder_of(&self.view, parse_note(alias)?, self.seat(owner)?);
            match holder {
                None => Some(None),
                Some(s) => Some(Some(self.player(s)?)),
            }
        }

        /// `promissory::held_foreign`.
        #[must_use]
        pub fn held_foreign(&self, player: &PlayerId, alias: &str) -> Option<usize> {
            Some(promissory::held_foreign(&self.view, self.seat(player)?, parse_note(alias)?))
        }

        /// `promissory::action_notes_in_hand`, in text order.
        #[must_use]
        pub fn action_notes_in_hand(&self, player: &PlayerId) -> Option<Vec<String>> {
            Some(Self::texts(promissory::action_notes_in_hand(&self.view, self.seat(player)?)))
        }

        /// `promissory::convoys_in_hand`: the first Trade Convoys in text order.
        #[must_use]
        pub fn convoys_in_hand(&self, player: &PlayerId) -> Option<Option<String>> {
            let notes = Self::texts(
                promissory::action_notes_in_hand(&self.view, self.seat(player)?)
                    .into_iter()
                    .filter(|k| k.note == PromissoryNote::TradeConvoys)
                    .collect(),
            );
            Some(notes.into_iter().next())
        }

        /// `promissory::reaches_anyone`.
        #[must_use]
        pub fn reaches_anyone(&self, player: &PlayerId) -> Option<bool> {
            Some(promissory::reaches_anyone(&self.view, self.seat(player)?))
        }

        /// `promissory::may_receive`.
        #[must_use]
        pub fn may_receive(&self, receiver: &PlayerId, note: &str) -> Option<bool> {
            let alias = note.split_once(':').map_or(note, |(a, _)| a);
            Some(promissory::may_receive(&self.view, self.seat(receiver)?, parse_note(alias)?))
        }

        /// `promissory::denies_movement_into`.
        #[must_use]
        pub fn denies_movement_into(&self, mover: &PlayerId, system: &SystemId) -> Option<bool> {
            Some(promissory::denies_movement_into(&self.view, self.seat(mover)?, typed::<System>(system.as_str())?))
        }
    }

    // -- static reads (printed data) ---------------------------------------------------------------

    fn scope(content: &ContentStore, sources: SourceSet) -> Option<Scope> {
        content.is_embedded().then(|| crate::core_adapter::scope(sources))
    }

    /// `exploration::traits_of`.
    #[must_use]
    pub fn traits_of(content: &ContentStore, sources: SourceSet, planet: &PlanetId) -> Option<Vec<String>> {
        let scope = scope(content, sources)?;
        let planet = typed::<Planet>(planet.as_str())?;
        Some(explore::traits_of(scope, planet).as_slice().iter().map(|k| trait_key(*k).to_owned()).collect())
    }

    /// `fracture::is_fracture_system`.
    #[must_use]
    pub fn is_fracture_system(content: &ContentStore, sources: SourceSet, system: &SystemId) -> Option<bool> {
        Some(explore::is_fracture_system(scope(content, sources)?, typed::<System>(system.as_str())?))
    }

    /// `fracture::systems`.
    #[must_use]
    pub fn fracture_systems(content: &ContentStore, sources: SourceSet) -> Option<Vec<SystemId>> {
        let scope = scope(content, sources)?;
        Some(explore::fracture_systems(scope).as_slice().iter().map(|s| SystemId::new(s.as_str())).collect())
    }

    /// `fracture::egress_systems`.
    #[must_use]
    pub fn egress_systems(content: &ContentStore, sources: SourceSet) -> Option<Vec<SystemId>> {
        let scope = scope(content, sources)?;
        Some(explore::egress_systems(scope).as_slice().iter().map(|s| SystemId::new(s.as_str())).collect())
    }

    /// `exploration::resolution`.
    #[must_use]
    pub fn resolution(content: &ContentStore, card: &str) -> Option<Option<String>> {
        if !content.is_embedded() {
            return None;
        }
        Some(Some(explore::resolution(typed::<Explore>(card)?).as_str().to_owned()))
    }

    /// `exploration::attachment_for`.
    #[must_use]
    pub fn attachment_for(content: &ContentStore, card: &str, planet: &PlanetId) -> Option<String> {
        if !content.is_embedded() {
            return None;
        }
        let explore_card = typed::<Explore>(card)?;
        let id = match explore::placed(explore_card) {
            Some(explore::Placed::Attachment(a)) => a.as_str().to_owned(),
            Some(explore::Placed::Token(t)) => t.as_str().to_owned(),
            None => card.to_owned(),
        };
        if typed::<Attachment>(&format!("{id}stat")).is_none() {
            return Some(id);
        }
        let has = typed::<Planet>(planet.as_str()).is_some_and(explore::has_specialty);
        Some(if has { format!("{id}stat") } else { id })
    }

    /// `promissory::is_play_area` (a note id `alias:owner`).
    #[must_use]
    pub fn is_play_area(content: &ContentStore, note: &str) -> Option<bool> {
        if !content.is_embedded() {
            return None;
        }
        let (alias, owner) = note.split_once(':')?;
        Some(promissory::is_play_area(parse_note(alias)?, typed::<Faction>(owner)?))
    }
}
