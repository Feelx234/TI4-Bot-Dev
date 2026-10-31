//! `faction_marks` values: the schema of each family and the parse / render of a value tree.

use ti4_engine_core::containers::Interner;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Word};

use crate::factions::borrowed_round_agents::RoundBonus;

use super::text::{Codec, K};

/// The shape of a value text.
#[derive(Debug, Clone, Copy)]
pub enum Sh {
    /// One piece.
    A(K),
    /// Fixed fields of different kinds, joined by this separator byte.
    Rec(u8, &'static [Sh]),
    /// Any number of items of one shape, joined by this separator byte; the empty text is none.
    List(u8, &'static Sh),
    /// The first of these shapes that fits.
    Alt(&'static [Sh]),
    /// The JSON of a Ssruu / round agent bonus (`borrowed_round_agents::RoundBonus`) as twelve fields.
    RoundBonus,
    /// The JSON of the unit that last sustained (`combat:sustain_target`): system, planet, owner, type, damage, galvanize token.
    SustainTarget,
}

const NUM: Sh = Sh::A(K::Num);
const SEAT: Sh = Sh::A(K::Seat);
const SYS: Sh = Sh::A(K::System);
const PLANET: Sh = Sh::A(K::Planet);
const UNIT: Sh = Sh::A(K::Unit);
const TECH: Sh = Sh::A(K::Tech);
const CARD: Sh = Sh::A(K::Card);
const NOTE: Sh = Sh::A(K::Note);
const SECRET: Sh = Sh::A(K::Secret);
const AGENDA: Sh = Sh::A(K::Agenda);
const FACTION: Sh = Sh::A(K::Faction);
const RELIC: Sh = Sh::A(K::Relic);
const BREAKTHROUGH: Sh = Sh::A(K::Breakthrough);
const OUTCOME: Sh = Sh::A(K::Outcome);
const EMPTY: Sh = Sh::A(K::Empty);
const BLOB: Sh = Sh::A(K::Blob);
const OPT_SEAT: Sh = Sh::A(K::Opt(&K::Seat));

const YES: Sh = Sh::A(K::Word(&[Word::Yes]));
const TRUE: Sh = Sh::A(K::Word(&[Word::True]));
const GRANTED: Sh = Sh::A(K::Word(&[Word::Granted]));
const BOOL: Sh = Sh::A(K::Word(&[Word::False, Word::True]));

const UNITS_WITH_COUNTS: Sh = Sh::List(b',', &Sh::Rec(b':', &[UNIT, NUM]));

const AGENT_GOODS: Sh = Sh::Alt(&[SEAT, Sh::A(K::Word(&[Word::Declined, Word::Probe]))]);

const STAGED_GROUND: Sh = Sh::Alt(&[
    Sh::Rec(
        b'|',
        &[
            Sh::A(K::Word(&[Word::Control])),
            SYS,
            PLANET,
            SEAT,
            OPT_SEAT,
        ],
    ),
    Sh::Rec(
        b'|',
        &[
            Sh::A(K::Word(&[Word::Destroyed])),
            SYS,
            PLANET,
            SEAT,
            UNIT,
            BOOL,
            Sh::A(K::Destroy),
        ],
    ),
    // ... of a galvanized unit: the string appends the token `|g` (`hooks_ground::stage_ground_force_destroyed`).
    Sh::Rec(
        b'|',
        &[
            Sh::A(K::Word(&[Word::Destroyed])),
            SYS,
            PLANET,
            SEAT,
            UNIT,
            BOOL,
            Sh::A(K::Destroy),
            Sh::A(K::Word(&[Word::Galvanized])),
        ],
    ),
    // A ground force of the space area (Nova Seed cargo): the row names the system where the planet goes
    // (`muaat::destroy_others`, "there is no planet, so the system id stands in").
    Sh::Rec(
        b'|',
        &[
            Sh::A(K::Word(&[Word::Destroyed])),
            SYS,
            SYS,
            SEAT,
            UNIT,
            BOOL,
            Sh::A(K::Destroy),
        ],
    ),
]);

const SAAR_BOOST: Sh = Sh::Alt(&[
    Sh::Rec(b'|', &[NUM, SYS, UNIT, NUM, NUM, NUM]),
    Sh::Rec(b'|', &[NUM, SYS, NUM, NUM]),
]);

/// The shape of the value of a mark with this key.
#[must_use]
pub fn schema(key: &MarkKey) -> Sh {
    match *key {
        MarkKey::AllowPass() => YES,
        MarkKey::ArgentProduction(..) => Sh::List(b';', &Sh::Rec(b'@', &[SYS, PLANET])),
        MarkKey::ArgentWingTransfer(..)
        | MarkKey::BonusFrom()
        | MarkKey::LinkFrom()
        | MarkKey::LinkTo()
        | MarkKey::LinkX()
        | MarkKey::LinkY()
        | MarkKey::WhX()
        | MarkKey::WhY()
        | MarkKey::MahactBenediction()
        | MarkKey::SardakkAgentTactical(..) => SYS,
        MarkKey::BastionRetreated(..) => Sh::Rec(b'|', &[SYS, UNITS_WITH_COUNTS]),
        MarkKey::CabalDevour(..)
        | MarkKey::DeepwroughtCarrina(..)
        | MarkKey::DeepwroughtOcean(..)
        | MarkKey::FirmamentEnervate()
        | MarkKey::MentakHero(..)
        | MarkKey::PrivateBorrowedScheming(..) => SEAT,
        MarkKey::CardsReveal(_, _, _, _, deck) => match deck {
            Word::ActionCards => Sh::List(b',', &CARD),
            Word::PromissoryNotes => Sh::List(b',', &Sh::Rec(b':', &[NOTE, FACTION])),
            _ => Sh::List(b',', &SECRET),
        },
        MarkKey::CardsStaged(..) => Sh::Rec(
            b'|',
            &[
                Sh::A(K::Word(&[Word::Discarded, Word::Taken])),
                SEAT,
                OPT_SEAT,
                CARD,
            ],
        ),
        MarkKey::YssarilSsruuRoundAgent(..) => Sh::Alt(&[Sh::RoundBonus, BLOB]),
        MarkKey::CombatSustainTarget() => Sh::Alt(&[Sh::SustainTarget, BLOB]),
        MarkKey::PrivateStagedEvent(..) => BLOB,
        MarkKey::CommanderAbility(..) => GRANTED,
        MarkKey::CrimsonCard(..) => Sh::List(b',', &UNIT),
        MarkKey::CrimsonSever(..) => Sh::Rec(b'|', &[SEAT, SYS]),
        MarkKey::DeepwroughtShare(..) => Sh::Rec(b'|', &[SEAT, TECH]),
        MarkKey::EmpyreanAetherstream() | MarkKey::FirmamentAgent() => Sh::Rec(b'|', &[NUM, SEAT]),
        MarkKey::EmpyreanPassage() | MarkKey::EmpyreanTetherAllow() => {
            Sh::Rec(b'|', &[NUM, SEAT, SEAT])
        }
        MarkKey::EmpyreanTether(..) => Sh::List(b';', &Sh::Rec(b'|', &[SYS, SYS])),
        MarkKey::FirmamentAgentMoved() => Sh::Rec(b'|', &[NUM, UNITS_WITH_COUNTS]),
        MarkKey::GhostAgentLink()
        | MarkKey::MuaatWarsunMoved(..)
        | MarkKey::TitansCoalescence(..) => Sh::Rec(b'|', &[NUM, SYS]),
        MarkKey::KeleresOdlynn(..) => OUTCOME,
        MarkKey::KeleresOmniopiares(..)
        | MarkKey::Paid(..)
        | MarkKey::PrivateCombatReplayRoll() => EMPTY,
        MarkKey::KeleresAgentGoods(..) => AGENT_GOODS,
        MarkKey::KeleresExecutiveOrderPending() => Sh::Rec(b'|', &[SEAT, AGENDA]),
        MarkKey::MahactFleet(..) => Sh::List(b',', &SEAT),
        MarkKey::MapEdit(..) => Sh::Rec(
            b'|',
            &[Sh::A(K::Word(&[Word::Replace, Word::Swap])), SYS, SYS],
        ),
        MarkKey::NaazSupercharge(..) => {
            Sh::Rec(b':', &[Sh::A(K::Word(&[Word::Ground, Word::Space])), NUM])
        }
        MarkKey::NekroThreat(..) => Sh::Rec(b'|', &[NUM, OUTCOME]),
        MarkKey::NekroToken(..) => Sh::List(b',', &FACTION),
        MarkKey::NekroTs(..) => Sh::Rec(b'|', &[SEAT, Sh::A(K::Word(&[Word::Open, Word::Used]))]),
        MarkKey::NomadCavalry(..) => Sh::Rec(b'|', &[SYS, UNIT]),
        MarkKey::NomadTcsNegotiating(..) => TRUE,
        MarkKey::PrivateSeen(kind, _) => match kind {
            Word::Relics => Sh::List(b',', &RELIC),
            Word::Breakthrough => Sh::List(b',', &BREAKTHROUGH),
            _ => Sh::List(b',', &TECH),
        },
        MarkKey::PrivateStagedGround(..) => STAGED_GROUND,
        MarkKey::PrivateSeat(_, kind) => match kind {
            Word::InsiderPeek => Sh::List(b',', &AGENDA),
            Word::WinnuGained => Sh::List(b',', &Sh::Rec(b'|', &[SYS, PLANET])),
            Word::WinnuImperator => Sh::Rec(b'|', &[SYS, UNIT, NUM, NUM]),
            _ => NUM,
        },
        MarkKey::RalnelSkim(..) => CARD,
        MarkKey::SaarAgentBoost(..) => SAAR_BOOST,
        MarkKey::SardakkHeroSkip() => Sh::Rec(b'|', &[SEAT, SYS]),
        MarkKey::SardakkTekklarCombat() => Sh::Rec(b'|', &[SYS, PLANET, SEAT, OPT_SEAT]),
        MarkKey::TitansAllowanceAsked(..) => EMPTY,
        MarkKey::TitansSleeper(..) => Sh::Rec(b'|', &[SEAT, SYS]),
        MarkKey::YssarilBt(kind, ..) => {
            if kind == Word::Negotiating {
                TRUE
            } else {
                NUM
            }
        }
        MarkKey::ArborecLw2(..)
        | MarkKey::ArborecPsychosporeExhausted(..)
        | MarkKey::BastionIconExhausted(..)
        | MarkKey::BastionLost(..)
        | MarkKey::CabalCrucible(..)
        | MarkKey::CrimsonBreachActive(..)
        | MarkKey::CrimsonBtExhausted(..)
        | MarkKey::CrimsonDeployed(..)
        | MarkKey::DeepwroughtBtExhausted(..)
        | MarkKey::DeepwroughtPurgedTech(..)
        | MarkKey::FirmamentSowing(..)
        | MarkKey::Inf2Card(..)
        | MarkKey::KeleresAsnUsed(..)
        | MarkKey::KeleresLawsBlank()
        | MarkKey::MahactCl2(..)
        | MarkKey::MahactVaultsExhausted(..)
        | MarkKey::NaaluGiftRound()
        | MarkKey::NekroAlastor(..)
        | MarkKey::NekroPropagation(..)
        | MarkKey::NekroThreatPhase(..)
        | MarkKey::NomadArtuno(..)
        | MarkKey::NomadHero(..)
        | MarkKey::PrivateGhostCommander(..)
        | MarkKey::PrivateMoved(..)
        | MarkKey::ProductionValueSwapSeq()
        | MarkKey::RalnelNanoExhausted(..)
        | MarkKey::SaarDeorbitBarrageExhausted(..)
        | MarkKey::SardakkTekklarRound()
        | MarkKey::YinAbilityUsed(..) => NUM,
    }
}

impl Codec {
    /// The tree of a value text of this shape.
    pub fn parse_shape(&self, blobs: &mut Interner, shape: Sh, text: &str) -> Option<MarkVal> {
        match shape {
            Sh::A(kind) => self.parse_atom(blobs, kind, text).map(MarkVal::Atom),
            Sh::Rec(sep, fields) => {
                let parts: Vec<&str> = text.split(char::from(sep)).collect();
                if parts.len() != fields.len() {
                    return None;
                }
                let mut out = Vec::with_capacity(parts.len());
                for (part, field) in parts.into_iter().zip(fields) {
                    out.push(self.parse_shape(blobs, *field, part)?);
                }
                Some(MarkVal::Rec(out))
            }
            Sh::List(sep, item) => {
                if text.is_empty() {
                    return Some(MarkVal::List(Vec::new()));
                }
                let mut out = Vec::new();
                for part in text.split(char::from(sep)) {
                    out.push(self.parse_shape(blobs, *item, part)?);
                }
                Some(MarkVal::List(out))
            }
            Sh::Alt(shapes) => shapes.iter().find_map(|s| self.parse_shape(blobs, *s, text)),
            Sh::RoundBonus => self.parse_round_bonus(blobs, text),
            Sh::SustainTarget => self.parse_sustain_target(blobs, text),
        }
    }

    /// The twelve fields of a round bonus; `None` unless the text is exactly what [`Self::render_round_bonus`]
    /// writes back.
    fn parse_round_bonus(&self, blobs: &mut Interner, text: &str) -> Option<MarkVal> {
        let b: RoundBonus = serde_json::from_str(text).ok()?;
        let leader = |me: &Self, blobs: &mut Interner, t: &str| me.parse_atom(blobs, K::Leader, t).map(MarkVal::Atom);
        let planet = |me: &Self, blobs: &mut Interner, p: &Option<ti4_model::id::PlanetId>| match p {
            Some(p) => me.parse_atom(blobs, K::Planet, p.as_str()).map(MarkVal::Atom),
            None => Some(MarkVal::Atom(Atom::Absent)),
        };
        let num = |n: u64| u32::try_from(n).ok().map(|n| MarkVal::Atom(Atom::Num(n)));
        let value = MarkVal::Rec(vec![
            self.parse_atom(blobs, K::Seat, b.borrower.as_str()).map(MarkVal::Atom)?,
            self.parse_atom(blobs, K::Seat, b.source_owner.as_str()).map(MarkVal::Atom)?,
            leader(self, blobs, &b.source_agent)?,
            self.parse_atom(blobs, K::System, b.system.as_str()).map(MarkVal::Atom)?,
            planet(self, blobs, &b.combat_planet)?,
            planet(self, blobs, &b.unit_planet)?,
            self.parse_atom(blobs, K::Seat, b.unit_owner.as_str()).map(MarkVal::Atom)?,
            self.parse_atom(blobs, K::Unit, b.unit_type.as_str()).map(MarkVal::Atom)?,
            num(u64::from(b.sustained_damage))?,
            num(u64::from(b.galvanized))?,
            num(u64::try_from(b.unit_index).ok()?)?,
            num(u64::from(b.round_seq))?,
        ]);
        (self.render_round_bonus(blobs, &value).as_deref() == Some(text)).then_some(value)
    }

    /// The six fields of a sustain target; `None` unless the text is exactly what [`Self::render_sustain_target`] writes back.
    fn parse_sustain_target(&self, blobs: &mut Interner, text: &str) -> Option<MarkVal> {
        let v: serde_json::Value = serde_json::from_str(text).ok()?;
        let unit = v.get("unit")?;
        let planet = match v.get("planet")? {
            serde_json::Value::Null => MarkVal::Atom(Atom::Absent),
            p => self.parse_atom(blobs, K::Planet, p.as_str()?).map(MarkVal::Atom)?,
        };
        let flag = |b: bool| MarkVal::Atom(Atom::Num(u32::from(b)));
        let value = MarkVal::Rec(vec![
            self.parse_atom(blobs, K::System, v.get("system")?.as_str()?).map(MarkVal::Atom)?,
            planet,
            self.parse_atom(blobs, K::Seat, unit.get("owner")?.as_str()?).map(MarkVal::Atom)?,
            self.parse_atom(blobs, K::Unit, unit.get("type_id")?.as_str()?).map(MarkVal::Atom)?,
            flag(unit.get("sustained_damage")?.as_bool()?),
            flag(unit.get("galvanized")?.as_bool()?),
        ]);
        (self.render_sustain_target(blobs, &value).as_deref() == Some(text)).then_some(value)
    }

    fn render_sustain_target(&self, blobs: &Interner, value: &MarkVal) -> Option<String> {
        let MarkVal::Rec(f) = value else { return None };
        let [system, planet, owner, unit, sustained, galvanized] = f.as_slice() else { return None };
        let text = |kind: K, v: &MarkVal| -> Option<String> {
            let MarkVal::Atom(a) = v else { return None };
            let mut out = String::new();
            self.render_atom(blobs, kind, *a, &mut out)?;
            Some(out)
        };
        let flag = |v: &MarkVal| match v {
            MarkVal::Atom(Atom::Num(n)) => Some(*n == 1),
            _ => None,
        };
        let planet = match planet {
            MarkVal::Atom(Atom::Absent) => None,
            p => Some(text(K::Planet, p)?),
        };
        let unit = ti4_model::units::Unit {
            type_id: ti4_model::id::UnitTypeId::new(text(K::Unit, unit)?),
            owner: ti4_model::id::PlayerId::new(text(K::Seat, owner)?),
            sustained_damage: flag(sustained)?,
            galvanized: flag(galvanized)?,
        };
        Some(serde_json::json!({"system": text(K::System, system)?, "planet": planet, "unit": unit}).to_string())
    }

    fn render_round_bonus(&self, blobs: &Interner, value: &MarkVal) -> Option<String> {
        let MarkVal::Rec(f) = value else { return None };
        let [borrower, source_owner, agent, system, combat_planet, unit_planet, unit_owner, unit, sustained, galvanized, index, round] = f.as_slice() else {
            return None;
        };
        let text = |kind: K, v: &MarkVal| -> Option<String> {
            let MarkVal::Atom(a) = v else { return None };
            let mut out = String::new();
            self.render_atom(blobs, kind, *a, &mut out)?;
            Some(out)
        };
        let planet = |v: &MarkVal| -> Option<Option<ti4_model::id::PlanetId>> {
            match v {
                MarkVal::Atom(Atom::Absent) => Some(None),
                v => text(K::Planet, v).map(|p| Some(ti4_model::id::PlanetId::new(p))),
            }
        };
        let num = |v: &MarkVal| -> Option<u32> {
            match v {
                MarkVal::Atom(Atom::Num(n)) => Some(*n),
                _ => None,
            }
        };
        let b = RoundBonus {
            borrower: ti4_model::id::PlayerId::new(text(K::Seat, borrower)?),
            source_owner: ti4_model::id::PlayerId::new(text(K::Seat, source_owner)?),
            source_agent: text(K::Leader, agent)?,
            system: ti4_model::id::SystemId::new(text(K::System, system)?),
            combat_planet: planet(combat_planet)?,
            unit_planet: planet(unit_planet)?,
            unit_owner: ti4_model::id::PlayerId::new(text(K::Seat, unit_owner)?),
            unit_type: ti4_model::id::UnitTypeId::new(text(K::Unit, unit)?),
            sustained_damage: num(sustained)? == 1,
            galvanized: num(galvanized)? == 1,
            unit_index: usize::try_from(num(index)?).ok()?,
            round_seq: num(round)?,
        };
        serde_json::to_string(&b).ok()
    }

    /// The text of a tree of this shape.
    #[must_use]
    pub fn render_shape(&self, blobs: &Interner, shape: Sh, value: &MarkVal) -> Option<String> {
        match (shape, value) {
            (Sh::A(kind), MarkVal::Atom(atom)) => {
                let mut out = String::new();
                self.render_atom(blobs, kind, *atom, &mut out)?;
                Some(out)
            }
            (Sh::Rec(sep, fields), MarkVal::Rec(values)) if fields.len() == values.len() => {
                let mut out = String::new();
                for (i, (field, value)) in fields.iter().zip(values).enumerate() {
                    if i > 0 {
                        out.push(char::from(sep));
                    }
                    out.push_str(&self.render_shape(blobs, *field, value)?);
                }
                Some(out)
            }
            (Sh::List(sep, item), MarkVal::List(values)) => {
                let mut out = String::new();
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        out.push(char::from(sep));
                    }
                    out.push_str(&self.render_shape(blobs, *item, value)?);
                }
                Some(out)
            }
            (Sh::Alt(shapes), value) => shapes.iter().find_map(|s| self.render_shape(blobs, *s, value)),
            (Sh::RoundBonus, value) => self.render_round_bonus(blobs, value),
            (Sh::SustainTarget, value) => self.render_sustain_target(blobs, value),
            _ => None,
        }
    }

    /// The typed value of a mark with this key.
    pub fn parse_mark_value(&self, blobs: &mut Interner, key: &MarkKey, text: &str) -> Option<MarkVal> {
        self.parse_shape(blobs, schema(key), text)
    }

    /// The text of a typed value of a mark with this key.
    #[must_use]
    pub fn render_mark_value(&self, blobs: &Interner, key: &MarkKey, value: &MarkVal) -> Option<String> {
        self.render_shape(blobs, schema(key), value)
    }

    /// A whole mark, key and value.
    pub fn parse_mark(&self, blobs: &mut Interner, key: &str, value: &str) -> Option<(MarkKey, MarkVal)> {
        let key = self.parse_mark_key(blobs, key)?;
        let value = self.parse_mark_value(blobs, &key, value)?;
        Some((key, value))
    }

    /// The texts of a whole mark.
    #[must_use]
    pub fn render_mark(&self, blobs: &Interner, key: &MarkKey, value: &MarkVal) -> Option<(String, String)> {
        Some((
            self.render_mark_key(blobs, key)?,
            self.render_mark_value(blobs, key, value)?,
        ))
    }
}
