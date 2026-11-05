//! The per-game tables of the codecs: the owner table and the text interner.

use std::collections::BTreeSet;

use ti4_engine_core::ids::Seat;
use ti4_engine_core::state::{Code, CoreState, OwnerTable, ResolverState, Slice, Texts as CoreTexts};
use ti4_model::state::GameState;

use super::error::{R, bad};
use super::keys::{Codec as KeysCodec, Owners as KeyOwners};
use super::texts::TextCodes;

/// The owners of one table in text order: seat `i` is the `i`-th (docs 1.3). At most 16.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Owners {
    ids: Vec<String>,
}

impl Owners {
    /// Every player and every other owner that can appear on the board or in the state: unit owners,
    /// command tokens, planet control and coexistence, the Styx holder, captured units' owners,
    /// plot tokens, the keys of the per-player maps.
    ///
    /// # Errors
    /// More than 16 owners.
    pub fn of(state: &GameState) -> R<Self> {
        Self::with_extra(state, &[])
    }

    /// [`Self::of`] with owners the board does not hold yet (the neutral owner a card is about to place units for).
    ///
    /// # Errors
    /// More than 16 owners.
    pub fn with_extra(state: &GameState, extra: &[&str]) -> R<Self> {
        let plot_tokens: Vec<ti4_model::id::PlayerId> =
            state.players.iter().flat_map(|p| ti4_model::plots::read(&p.plots)).flat_map(|pl| pl.tokens).collect();
        let mut ids: BTreeSet<&str> = BTreeSet::new();
        ids.extend(plot_tokens.iter().map(|p| p.as_str()));
        ids.extend(extra.iter().copied());
        ids.extend(state.players.iter().map(|p| p.id.as_str()));
        ids.extend(state.seating_order.iter().map(|p| p.as_str()));
        ids.insert(state.speaker.as_str());
        ids.extend(state.active.iter().map(|p| p.as_str()));
        ids.extend(state.styx_holder.iter().map(|p| p.as_str()));
        ids.extend(state.influence_pays_for_units.iter().map(|p| p.as_str()));
        ids.extend(state.agenda_elected_override.iter().map(|p| p.as_str()));
        for system in state.board.values() {
            ids.extend(system.units.iter().map(|u| u.owner.as_str()));
            ids.extend(system.planet_units.values().flatten().map(|u| u.owner.as_str()));
            ids.extend(system.command_tokens.iter().map(|p| p.as_str()));
            ids.extend(system.planet_control.values().map(|p| p.as_str()));
            ids.extend(system.coexisting.values().flatten().map(|p| p.as_str()));
        }
        for p in &state.players {
            ids.extend(p.duress_by.iter().map(|x| x.as_str()));
            ids.extend(p.captured_units.iter().map(|(o, _)| o.as_str()));
        }
        transient_owners(state, &mut ids);
        ids.extend(state.scored_objectives.keys().map(|p| p.as_str()));
        ids.extend(state.promissory_notes.values().map(|p| p.as_str()));
        ids.extend(state.support_holders.iter().flat_map(|(a, b)| [a.as_str(), b.as_str()]));
        ids.extend(state.expedition_slices.values().map(|p| p.as_str()));
        if ids.len() > 16 {
            let why = bad(Slice::Seats, "more than 16 owners", ids.len());
            crate::product::refused(&why);
            return Err(why);
        }
        Ok(Self { ids: ids.into_iter().map(str::to_owned).collect() })
    }

    /// The ids in seat order.
    #[must_use]
    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    #[must_use]
    pub fn seat(&self, id: &str) -> Option<Seat> {
        self.ids.binary_search_by(|s| s.as_str().cmp(id)).ok().map(|i| Seat(i as u8))
    }

    #[must_use]
    pub fn id(&self, seat: Seat) -> Option<&str> {
        self.ids.get(usize::from(seat.0)).map(String::as_str)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// The owners the transient part of the position names: the last fight's sides and casualties, the invasion's defender, the plays of the turn. The board no longer
/// holds the neutral force once its last unit fell, but the record of that fight still names it (`last_combat_sides`, `last_ship_destroyed`, the presentation of the
/// round), and the `Transient` codec has to find every one of them in the table it is loaded with.
fn transient_owners<'a>(state: &'a GameState, ids: &mut BTreeSet<&'a str>) {
    let mut add = |p: &'a ti4_model::id::PlayerId| {
        ids.insert(p.as_str());
    };
    state.reroll_staging.keys().for_each(&mut add);
    state.last_reroll_player.iter().for_each(&mut add);
    state.last_committed_unit.iter().for_each(|(p, _, _, u)| {
        add(p);
        add(&u.owner);
    });
    state.last_sustain.iter().for_each(|(_, o, _, p, _)| {
        add(o);
        add(p);
    });
    state.pending_destructions.iter().for_each(|(_, o, _, _, _)| add(o));
    state.last_ship_destroyed.iter().for_each(|(_, o, _)| add(o));
    state.last_control_gained.iter().for_each(|(_, _, g, f)| {
        add(g);
        f.iter().for_each(&mut add);
    });
    state.last_combat_sides.iter().for_each(|(_, sides)| sides.iter().for_each(&mut add));
    state.active_space_combat.iter().for_each(|(_, a, d)| {
        add(a);
        add(d);
    });
    state.combat_round_hits.keys().for_each(&mut add);
    state.combat_round_dice.iter().for_each(|d| add(&d.player));
    let presentation = &state.combat_presentation;
    presentation.round_start.iter().chain(&presentation.barrage_start).for_each(|u| add(&u.owner));
    presentation.barrage_hits.keys().for_each(&mut add);
    presentation.barrage_dice.iter().for_each(|d| add(&d.player));
    presentation.remaining_hits.keys().for_each(&mut add);
    state.pending_reflective_hits.iter().for_each(|(_, v, _)| add(v));
    state.last_action_discarded.iter().for_each(|(p, _)| add(p));
    state.action_card_plays.iter().chain(&state.declared_card_plays).for_each(|(p, _)| add(p));
    state.last_strategy_choice.iter().for_each(|(p, _)| add(p));
    state.starting_tech_picks.iter().for_each(|pick| add(&pick.player));
    if let Some(invasion) = &state.active_invasion {
        add(&invasion.invader);
        invasion.defender.iter().for_each(&mut add);
        if let Some(step) = &invasion.last_step {
            step.before.iter().chain(&step.after).for_each(|u| add(&u.owner));
            step.dice.iter().for_each(|d| add(&d.player));
            step.hits.keys().for_each(&mut add);
        }
    }
}

/// The text codes a Ctx hands to the cores it loads: the lineage's tables **between** lends.
///
/// The tables belong to the state (`CoreState::texts`, D2-3); a core loaded through this `Ctx` starts
/// from a clone of them and the codecs intern into the core. Adapter code that needs a code *before* a
/// core exists (the facts of a window, built from the string position) mints it here, and the next load
/// carries it into the core; after a load or a lend the `Ctx` adopts the core's tables, so a code minted
/// by either side resolves on both. `Code(0)` is the diplomacy rules tag, so a default `Diplomacy` equals
/// a loaded default `DiplomacyState`. Until the flip (D2-8) a `Ctx` is rebuilt for every lend; after it the
/// state is the only owner and this field goes away.
#[derive(Debug, Clone, Default)]
pub struct Texts {
    inner: CoreTexts,
}

impl Texts {
    /// # Errors
    /// More than 65 535 distinct texts.
    pub fn intern(&mut self, slice: Slice, text: &str) -> R<Code> {
        self.inner.intern_code(slice, text)
    }

    /// # Errors
    /// A code this table never issued.
    pub fn text(&self, slice: Slice, code: Code) -> R<&str> {
        self.inner.code_text(slice, code)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.codes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.codes.is_empty()
    }

    /// The tables as the state holds them.
    #[must_use]
    pub const fn tables(&self) -> &CoreTexts {
        &self.inner
    }

    pub(crate) fn tables_mut(&mut self) -> &mut CoreTexts {
        &mut self.inner
    }
}

/// The tables of one game.
#[derive(Debug, Clone, Default)]
pub struct Ctx {
    pub owners: Owners,
    pub texts: Texts,
    /// The text codec of the typed keys (marks, outcomes, ledger reasons): the owner table in the
    /// keys' own form (`None` while an owner id is a proper prefix of another, which the key order
    /// cannot represent: the marks and outcomes of that game stay unmapped).
    pub keys: KeysCodec,
    /// The persistent part of the timing resolver the cores of this game start from (D2-4b): the `Pending` codec loads it
    /// into `pending.resolver`, every load or lend adopts what the core's `Pending` holds afterwards. `None` until the
    /// caller sets one (the string engine keeps its resolver in `Game`, not in `GameState`; a reverse lend hands it in).
    pub resolver: Option<ResolverState>,
    /// `owners` as the state holds them; every core loaded through this `Ctx` shares it.
    owner_table: OwnerTable,
    /// Owners the table holds although the position does not (yet): the neutral owner of a card that places units for it.
    extra: Vec<String>,
}

impl Ctx {
    /// # Errors
    /// More than 16 owners.
    pub fn of(state: &GameState) -> R<Self> {
        Self::with_extra(state, &[])
    }

    /// [`Self::of`] with the owners of `extra` in the table.
    ///
    /// # Errors
    /// More than 16 owners.
    pub fn with_extra(state: &GameState, extra: &[&str]) -> R<Self> {
        let owners = Owners::with_extra(state, extra)?;
        let mut keys = KeysCodec::default();
        keys.owners = key_owners(&owners);
        if keys.owners.is_none() {
            // XF-6: the key order cannot represent an owner id that is a proper prefix of another one.
            crate::product::refused(&bad(Slice::Seats, "an owner id that is a prefix of another", owners.ids().join(",")));
        }
        let owner_table = owner_table(&owners);
        Ok(Self { owners, texts: Texts::default(), keys, resolver: None, owner_table, extra: extra.iter().map(|e| (*e).to_owned()).collect() })
    }

    /// Check that `state` still has the owners of this table: players are fixed for a game, the
    /// table of a state never changes (D2-3).
    ///
    /// # Errors
    /// More than 16 owners, or an owner the table does not hold (or one it holds that the position lost).
    pub fn refresh(&mut self, state: &GameState) -> R<()> {
        let extra: Vec<&str> = self.extra.iter().map(String::as_str).collect();
        let owners = Owners::with_extra(state, &extra)?;
        if owners != self.owners {
            let id = owners.ids().iter().find(|id| self.owners.seat(id).is_none()).or_else(|| self.owners.ids().first());
            return Err(bad(Slice::Seats, "owner not in the game's table", id.map_or("", String::as_str)));
        }
        Ok(())
    }

    /// Run `f` with the staged text tables out of the `Ctx` (so a codec helper can take the owner table
    /// by `&Ctx` and the texts by `&mut`).
    pub fn minting<T>(&mut self, f: impl FnOnce(&Self, &mut CoreTexts) -> T) -> T {
        let mut tables = std::mem::take(&mut self.texts.inner);
        let out = f(self, &mut tables);
        self.texts.inner = tables;
        out
    }

    /// Give a freshly made core the tables of this lineage: the owners and the texts.
    pub fn attach(&self, core: &mut CoreState) {
        core.owners = self.owner_table.clone();
        core.texts = self.texts.inner.clone();
    }

    /// Take over the tables of a core this `Ctx` attached (and the codecs and the call grew): what a core
    /// minted resolves here from now on.
    pub fn adopt(&mut self, core: &CoreState) {
        debug_assert!(core.texts.codes.extends(&self.texts.inner.codes), "a core's code table must continue the lineage's");
        debug_assert!(core.texts.blobs.extends(&self.texts.inner.blobs), "a core's blob table must continue the lineage's");
        self.texts.inner = core.texts.clone();
        if core.loaded.has(Slice::Pending) && (self.resolver.is_some() || core.pending.resolver != ResolverState::default()) {
            self.resolver = Some(core.pending.resolver.clone());
        }
    }

    pub(crate) fn seat(&self, slice: Slice, id: &str) -> R<Seat> {
        self.owners.seat(id).ok_or_else(|| bad(slice, "owner not in the table", id))
    }

    pub(crate) fn owner(&self, slice: Slice, seat: Seat) -> R<&str> {
        self.owners.id(seat).ok_or_else(|| bad(slice, "seat not in the table", seat.0))
    }
}

fn owner_table(owners: &Owners) -> OwnerTable {
    OwnerTable::new(owners.ids().iter().map(String::as_bytes))
}

fn key_owners(owners: &Owners) -> Option<KeyOwners> {
    KeyOwners::new(owners.ids().iter().map(String::as_str))
}
