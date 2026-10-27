//! `faction_marks` values: the schema of each family and the parse / render of a value tree.

use ti4_engine_core::keys::{MarkKey, MarkVal, Word};

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
        MarkKey::CombatSustainTarget()
        | MarkKey::PrivateStagedEvent(..)
        | MarkKey::YssarilSsruuRoundAgent(..) => BLOB,
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
    pub fn parse_shape(&mut self, shape: Sh, text: &str) -> Option<MarkVal> {
        match shape {
            Sh::A(kind) => self.parse_atom(kind, text).map(MarkVal::Atom),
            Sh::Rec(sep, fields) => {
                let parts: Vec<&str> = text.split(char::from(sep)).collect();
                if parts.len() != fields.len() {
                    return None;
                }
                let mut out = Vec::with_capacity(parts.len());
                for (part, field) in parts.into_iter().zip(fields) {
                    out.push(self.parse_shape(*field, part)?);
                }
                Some(MarkVal::Rec(out))
            }
            Sh::List(sep, item) => {
                if text.is_empty() {
                    return Some(MarkVal::List(Vec::new()));
                }
                let mut out = Vec::new();
                for part in text.split(char::from(sep)) {
                    out.push(self.parse_shape(*item, part)?);
                }
                Some(MarkVal::List(out))
            }
            Sh::Alt(shapes) => shapes.iter().find_map(|s| self.parse_shape(*s, text)),
        }
    }

    /// The text of a tree of this shape.
    #[must_use]
    pub fn render_shape(&self, shape: Sh, value: &MarkVal) -> Option<String> {
        match (shape, value) {
            (Sh::A(kind), MarkVal::Atom(atom)) => {
                let mut out = String::new();
                self.render_atom(kind, *atom, &mut out)?;
                Some(out)
            }
            (Sh::Rec(sep, fields), MarkVal::Rec(values)) if fields.len() == values.len() => {
                let mut out = String::new();
                for (i, (field, value)) in fields.iter().zip(values).enumerate() {
                    if i > 0 {
                        out.push(char::from(sep));
                    }
                    out.push_str(&self.render_shape(*field, value)?);
                }
                Some(out)
            }
            (Sh::List(sep, item), MarkVal::List(values)) => {
                let mut out = String::new();
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        out.push(char::from(sep));
                    }
                    out.push_str(&self.render_shape(*item, value)?);
                }
                Some(out)
            }
            (Sh::Alt(shapes), value) => shapes.iter().find_map(|s| self.render_shape(*s, value)),
            _ => None,
        }
    }

    /// The typed value of a mark with this key.
    pub fn parse_mark_value(&mut self, key: &MarkKey, text: &str) -> Option<MarkVal> {
        self.parse_shape(schema(key), text)
    }

    /// The text of a typed value of a mark with this key.
    #[must_use]
    pub fn render_mark_value(&self, key: &MarkKey, value: &MarkVal) -> Option<String> {
        self.render_shape(schema(key), value)
    }

    /// A whole mark, key and value.
    pub fn parse_mark(&mut self, key: &str, value: &str) -> Option<(MarkKey, MarkVal)> {
        let key = self.parse_mark_key(key)?;
        let value = self.parse_mark_value(&key, value)?;
        Some((key, value))
    }

    /// The texts of a whole mark.
    #[must_use]
    pub fn render_mark(&self, key: &MarkKey, value: &MarkVal) -> Option<(String, String)> {
        Some((
            self.render_mark_key(key)?,
            self.render_mark_value(key, value)?,
        ))
    }
}
