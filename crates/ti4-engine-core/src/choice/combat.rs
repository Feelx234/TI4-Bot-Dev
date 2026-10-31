//! Decision types of the `combat` area: space and ground combat rounds, hits, retreats, bombardment.
//!
//! Owner: package B1 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far: the offer to cancel a hit with
//! SUSTAIN DAMAGE (LRR 87). The adapter side is `ti4-engine/src/render/combat.rs`.

use crate::generated::{Planet, System, Unit};

/// Where a unit that may sustain stands: in the space area (its index in the system's unit list) or on
/// a planet (the Eidolon Maximum, or a ground force an Alastor brought into the battle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SustainSite {
    Space(u16),
    /// `variant`: the unit is named by its index on the planet (`planet:{p}:variant:{i}`) rather than
    /// by the planet alone.
    Planet { planet: Planet, index: u16, variant: bool },
}

/// What the area asks. Parameters only; the option list is derived by `opts(state, ask)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// "cancel a hit at {system}": `hits` are still owed.
    CancelHit {
        system: System,
        hits: u16,
        /// The loss is bound to non-fighter ships and one exists (Graviton Laser System): display flag
        /// of the decline option.
        non_fighters_first: bool,
        /// `supply::staging_enabled`: planet-standing sustainers are named by planet and each is offered.
        staging: bool,
        /// The hit is a unit ability's (Naaz Maximum cannot be assigned it).
        ability: bool,
        /// The hit is part of a space combat (not SPACE CANNON OFFENSE).
        in_combat: bool,
        /// The combat window's own pending question (not the absorb path's): the owner's ships in the system.
        /// Its space sustainers are named by their index (`sustain|3`) and every option carries the
        /// `ShipsInSystem` preview.
        window: Option<u16>,
    },
    /// "announce a retreat from {system}" (78.9): `ships` of the asked player are in the system; `forced`: a Rout
    /// leaves no "stay".
    AnnounceRetreat { system: System, forced: bool, ships: u16 },
    /// "retreat to which system" (78.7): the whole fleet of `fleet` ships moves to the chosen neighbour.
    RetreatTo { system: System, fleet: u16 },
    /// Skilled Retreat: "withdraw to which system" (the card moves the whole fleet of `fleet` ships).
    Withdraw { system: System, fleet: u16 },
    /// "assign a hit": the owner picks which of the listed units is lost. The list is the caller's
    /// (ships, a group, a planet's units); the options are derived from it by `casualty_opts`.
    AssignHit,
    /// Raid Formation (Argent): the barrage's producer picks which of `target`'s ships with SUSTAIN DAMAGE
    /// becomes damaged, for a hit beyond the fighters.
    RaidDamage { target: crate::ids::Seat },
    /// "reroll die {n} of {unit}": one optional question per staged die (the Crowns of Thalnos, Munitions
    /// Reserves' "choose which dice").
    RerollDie { source: RerollSource, unit: crate::state::Code, die: u8 },
    /// Heart of Ixth: add or subtract 1 from a die just rolled (one question per holder).
    HeartIxth,
    /// War Funding: reroll all of the opponent's dice and the holder's misses.
    WarFunding,
    /// Wrath of Kenara: spend trade goods to add 1 to near misses.
    Wrath,
    /// Munitions Reserves after the roll: pay 2 trade goods to reroll missed dice.
    MunitionsRoll,
    /// Munitions Reserves at the start of a round (below `RULES_MUNITIONS_AFTER_ROLL`): pay 2 trade goods to reroll this round's misses;
    /// `have` is the trade goods the payer holds (the question's display facts).
    MunitionsStart { have: u16 },
    /// Supercharge (Naaz): "at the start of a combat round, you may exhaust this card to apply +1 to the results of all of your
    /// units' combat rolls during this combat round".
    Supercharge,
    /// Ambush (Mentak): which (up to 2) cruisers or destroyers roll at the start of the combat.
    Ambush,
    /// Impulse Core (`devotion: false`, at the start of a combat) and Devotion (Yin, after a round): which cruiser or destroyer is
    /// destroyed to produce 1 hit.
    Sacrifice { devotion: bool },
    /// Dimensional Splicer (Ghost): produce 1 hit at the start of a combat in a system with a wormhole.
    Splicer,
    /// The Dunlain Reaper (Letnev): "at the start of a round of ground combat, you may spend 2 resources to replace 1 of your infantry
    /// in that combat with 1 mech from your reinforcements".
    DeployMech { planet: Planet },
}

/// Which card asked the per-die reroll question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RerollSource {
    /// The Crown of Thalnos relic.
    Relic,
    /// The Crown of Thalnos law.
    Law,
    /// Munitions Reserves ("choose which dice to reroll").
    Munitions,
    /// Fire Team ("reroll any number of your dice" after your ground forces roll).
    FireTeam,
    /// The Jol-Nar commander (Ta Zern's "after you roll dice for a unit ability: you may reroll any of those dice").
    Commander,
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Sustain damage on this unit of the given type; the decline is the generic one ("take the hit").
    Sustain { site: SustainSite, unit: Unit, galvanized: bool },
    /// Stay and fight (the announcement of a retreat).
    Stay,
    /// Announce a retreat.
    Announce,
    /// Retreat to `system`, where `there` of the player's ships stand already.
    RetreatTo { system: System, there: u16 },
    /// Skilled Retreat: withdraw to `system`, where `there` of the player's ships stand already.
    Withdraw { system: System, there: u16 },
    /// Destroy the unit at `index` of the caller's list (the first of its type and damage).
    Destroy { index: u16, unit: Unit, damaged: bool },
    /// Damage a ship of this type (the first undamaged one).
    RaidDamage { unit: Unit },
    /// Reroll the die `die` of staged entry `entry` (`unit` is its rendered text, `face` what it shows).
    RerollDie { unit: crate::state::Code, entry: u8, die: u8, face: u8, hits_on: Option<u8> },
    /// Heart of Ixth: add (or subtract) 1 on the die `die` of entry `entry`; `single`: the only die rolled.
    HeartIxth { unit: crate::state::Code, entry: u8, die: u8, add: bool, single: bool },
    UseWarFunding,
    /// Wrath of Kenara: spend `count` trade goods.
    Kenara { count: u8 },
    /// Munitions Reserves: `choose` the dice one by one, or reroll all `misses` missed dice.
    Munitions { choose: bool, misses: u8 },
    /// Pay for Munitions Reserves at the start of the round.
    MunitionsStart,
    /// Exhaust Supercharge.
    UseSupercharge,
    /// Ambush: roll for one ship of type `a`, and for one of type `b` (the same type twice is two ships).
    Ambush { a: Unit, b: Option<Unit> },
    /// Destroy one of the listed type to produce a hit.
    Sacrifice { unit: Unit },
    /// Produce the hit.
    ProduceHit,
    /// Deploy the mech.
    DeployMech,
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    SustainDamage,
    AnnounceRetreat,
    RetreatTo,
    /// `skilled_retreat_choose_system`.
    Withdraw,
    /// The caller's subtype text (interned) of a casualty question.
    Casualty(crate::state::Code),
    /// The per-die reroll question of a card.
    Reroll(RerollSource),
    HeartIxth,
    WarFunding,
    WrathBump,
    MunitionsRoll,
    Supercharge,
    /// `ambush_roll`.
    Ambush,
    /// `impulse_core_sacrifice` / `devotion_sacrifice`.
    Sacrifice { devotion: bool },
    /// `dimensional_splicer`.
    Splicer,
    /// `deploy_mech`.
    DeployMech,
}
