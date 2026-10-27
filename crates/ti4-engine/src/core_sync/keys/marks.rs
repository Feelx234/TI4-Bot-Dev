//! `faction_marks`: the text of every key family, and the schema of its value.

use ti4_engine_core::keys::{Atom, Family, MarkKey, Word};

use super::text::{Codec, K};

/// A piece of a key pattern.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Seg {
    /// Literal text.
    L(&'static str),
    /// A field.
    F(K),
    /// A group that is there or not as a whole (its fields are `Absent` when it is not).
    O(&'static [Seg]),
}
use Seg::{F, L, O};

const SEAT: Seg = F(K::Seat);
const SYS: Seg = F(K::System);
const PLANET: Seg = F(K::Planet);
const UNIT: Seg = F(K::Unit);
const DEC: Seg = F(K::Dec);
const NOTE: Seg = F(K::Note);
const FACTION: Seg = F(K::Faction);
const LEADER: Seg = F(K::Leader);
const TECH: Seg = F(K::Tech);

const REVEAL_KINDS: &[Word] = &[Word::Action, Word::Choice, Word::Standing];
const REVEAL_SOURCES: &[Word] = &[
    Word::Mi,
    Word::Spynet,
    Word::YssarilCommander,
    Word::YssarilHero,
];
const REVEAL_DECKS: &[Word] = &[
    Word::ActionCards,
    Word::PromissoryNotes,
    Word::SecretObjectives,
];
const SEEN_KINDS: &[Word] = &[Word::Breakthrough, Word::Relics, Word::Technologies];
const PRIVATE_KINDS: &[Word] = &[
    Word::InsiderPeek,
    Word::WinnuGained,
    Word::WinnuImperator,
    Word::WinnuTactical,
];
const BT_KINDS: &[Word] = &[Word::Declined, Word::Exempt, Word::Negotiating];

/// The pattern of every family, in the order of `Family::ALL`.
pub(crate) static PATTERNS: [(Family, &[Seg]); Family::COUNT] = [
    (Family::AllowPass, &[L("allow_pass")]),
    (Family::ArborecLw2, &[L("arborec:lw2:"), SEAT]),
    (
        Family::ArborecPsychosporeExhausted,
        &[L("arborec:psychospore:exhausted:"), SEAT],
    ),
    (
        Family::ArgentProduction,
        &[L("argent:argentagent:production:"), SEAT, L(":"), SYS],
    ),
    (
        Family::ArgentWingTransfer,
        &[L("argent:wing_transfer:"), SEAT],
    ),
    (
        Family::BastionIconExhausted,
        &[L("bastion:icon:exhausted:"), SEAT],
    ),
    (
        Family::BastionLost,
        &[L("bastion:lost:"), SYS, L(":"), SEAT, L(":"), UNIT],
    ),
    (
        Family::BastionRetreated,
        &[L("bastion:retreated:"), SYS, L(":"), SEAT],
    ),
    (Family::BonusFrom, &[L("bonus_from")]),
    (Family::CabalCrucible, &[L("cabal:crucible:"), SEAT]),
    (
        Family::CabalDevour,
        &[L("cabal:devour:"), SYS, O(&[L("|"), PLANET])],
    ),
    (
        Family::CardsReveal,
        &[
            L("cards:reveal:"),
            F(K::Word(REVEAL_KINDS)),
            L("|"),
            F(K::Word(REVEAL_SOURCES)),
            L("|"),
            SEAT,
            L("|"),
            SEAT,
            L("|"),
            F(K::Word(REVEAL_DECKS)),
        ],
    ),
    (Family::CardsStaged, &[L("cards:staged:"), F(K::Pad(8))]),
    (Family::CombatSustainTarget, &[L("combat:sustain_target")]),
    (
        Family::CommanderAbility,
        &[L("commander_ability:"), SEAT, L(":"), LEADER],
    ),
    (
        Family::CrimsonBreachActive,
        &[L("crimson:breach:active:"), SYS],
    ),
    (
        Family::CrimsonBtExhausted,
        &[L("crimson:bt:exhausted:"), SEAT],
    ),
    (Family::CrimsonCard, &[L("crimson:card:"), SEAT]),
    (
        Family::CrimsonDeployed,
        &[L("crimson:deployed:"), SEAT, L(":"), DEC],
    ),
    (
        Family::CrimsonSever,
        &[L("crimson:sever:"), NOTE, L(":"), FACTION],
    ),
    (
        Family::DeepwroughtBtExhausted,
        &[L("deepwrought:bt:exhausted:"), SEAT],
    ),
    (
        Family::DeepwroughtCarrina,
        &[L("deepwrought:carrina:"), SEAT],
    ),
    (Family::DeepwroughtOcean, &[L("deepwrought:ocean:"), PLANET]),
    (
        Family::DeepwroughtPurgedTech,
        &[L("deepwrought:purged_tech:"), TECH],
    ),
    (
        Family::DeepwroughtShare,
        &[L("deepwrought:share:"), NOTE, L(":"), FACTION],
    ),
    (Family::EmpyreanAetherstream, &[L("empyrean|aetherstream")]),
    (Family::EmpyreanPassage, &[L("empyrean|passage")]),
    (Family::EmpyreanTetherAllow, &[L("empyrean|tether_allow")]),
    (Family::EmpyreanTether, &[L("empyrean|tether|"), SEAT]),
    (Family::FirmamentAgent, &[L("firmament:agent")]),
    (Family::FirmamentAgentMoved, &[L("firmament:agent:moved")]),
    (Family::FirmamentEnervate, &[L("firmament:enervate")]),
    (Family::FirmamentSowing, &[L("firmament:sowing:"), SEAT]),
    (Family::GhostAgentLink, &[L("ghost:agent_link")]),
    (Family::Inf2Card, &[L("inf2:card:"), SEAT]),
    (
        Family::KeleresOdlynn,
        &[L("keleres:odlynn:"), SEAT, L(":"), DEC],
    ),
    (
        Family::KeleresOmniopiares,
        &[L("keleres:omniopiares:"), DEC, L(":"), PLANET],
    ),
    (
        Family::KeleresAgentGoods,
        &[L("keleres|agent_goods|"), SEAT],
    ),
    (Family::KeleresAsnUsed, &[L("keleres|asn_used|"), SEAT]),
    (
        Family::KeleresExecutiveOrderPending,
        &[L("keleres|executive_order_pending")],
    ),
    (Family::KeleresLawsBlank, &[L("keleres|laws_blank")]),
    (Family::LinkFrom, &[L("link_from")]),
    (Family::LinkTo, &[L("link_to")]),
    (Family::LinkX, &[L("link_x")]),
    (Family::LinkY, &[L("link_y")]),
    (Family::MahactBenediction, &[L("mahact:benediction")]),
    (Family::MahactCl2, &[L("mahact:cl2:"), SEAT]),
    (Family::MahactFleet, &[L("mahact:fleet:"), SEAT]),
    (
        Family::MahactVaultsExhausted,
        &[L("mahact:vaults:exhausted:"), SEAT],
    ),
    (Family::MapEdit, &[L("map_edit|"), F(K::Pad(4))]),
    (Family::MentakHero, &[L("mentak:hero:"), SYS]),
    (Family::MuaatWarsunMoved, &[L("muaat:warsun_moved:"), SEAT]),
    (Family::NaaluGiftRound, &[L("naalu:gift_round")]),
    (Family::NaazSupercharge, &[L("naaz:supercharge:"), SEAT]),
    (
        Family::NekroAlastor,
        &[
            L("nekro:alastor|"),
            DEC,
            L("|"),
            SYS,
            L("|"),
            SEAT,
            L("|"),
            PLANET,
            L("|"),
            UNIT,
        ],
    ),
    (Family::NekroPropagation, &[L("nekro:propagation:"), SEAT]),
    (Family::NekroThreat, &[L("nekro:threat:"), SEAT]),
    (Family::NekroThreatPhase, &[L("nekro:threat_phase:"), SEAT]),
    (Family::NekroToken, &[L("nekro:token:"), SEAT, L(":Z")]),
    (
        Family::NekroTs,
        &[L("nekro:ts:"), SYS, O(&[L("|"), PLANET])],
    ),
    (Family::NomadArtuno, &[L("nomad:artuno:"), SEAT]),
    (Family::NomadCavalry, &[L("nomad:cavalry:"), SEAT]),
    (Family::NomadHero, &[L("nomad:hero:"), SEAT]),
    (
        Family::NomadTcsNegotiating,
        &[L("nomad:tcs:negotiating:"), SEAT, L(":"), SEAT],
    ),
    (Family::Paid, &[L("paid:"), SEAT, L(":"), TECH]),
    (
        Family::PrivateBorrowedScheming,
        &[L("private:#borrowed_scheming:"), SEAT],
    ),
    (
        Family::PrivateCombatReplayRoll,
        &[L("private:#combat:replay_roll")],
    ),
    (
        Family::PrivateGhostCommander,
        &[L("private:#ghost:commander:"), SEAT, L(":"), DEC],
    ),
    (Family::PrivateMoved, &[L("private:#moved:"), DEC]),
    (
        Family::PrivateSeen,
        &[L("private:#seen:"), F(K::Word(SEEN_KINDS)), L(":"), SEAT],
    ),
    (
        Family::PrivateStagedEvent,
        &[L("private:#staged:event:"), F(K::Pad(6))],
    ),
    (
        Family::PrivateStagedGround,
        &[L("private:#staged:ground:"), F(K::Pad(8))],
    ),
    (
        Family::PrivateSeat,
        &[L("private:"), SEAT, L(":"), F(K::Word(PRIVATE_KINDS))],
    ),
    (
        Family::ProductionValueSwapSeq,
        &[L("production:value_swap_seq")],
    ),
    (
        Family::RalnelNanoExhausted,
        &[L("ralnel:nano:exhausted:"), SEAT],
    ),
    (Family::RalnelSkim, &[L("ralnel:skim:"), SEAT]),
    (Family::SaarAgentBoost, &[L("saar:agent:boost:"), SEAT]),
    (
        Family::SaarDeorbitBarrageExhausted,
        &[L("saar:deorbit_barrage:exhausted:"), SEAT],
    ),
    (
        Family::SardakkAgentTactical,
        &[L("sardakk:agent:tactical:"), SEAT],
    ),
    (Family::SardakkHeroSkip, &[L("sardakk:hero:skip")]),
    (Family::SardakkTekklarCombat, &[L("sardakk:tekklar:combat")]),
    (Family::SardakkTekklarRound, &[L("sardakk:tekklar:round")]),
    (
        Family::TitansAllowanceAsked,
        &[L("titans:allowance_asked:"), SEAT, L(":"), DEC],
    ),
    (
        Family::TitansCoalescence,
        &[L("titans:coalescence:"), PLANET],
    ),
    (Family::TitansSleeper, &[L("titans:sleeper:"), PLANET]),
    (Family::WhX, &[L("wh_x")]),
    (Family::WhY, &[L("wh_y")]),
    (Family::YinAbilityUsed, &[L("yin:ability_used:"), SEAT]),
    (
        Family::YssarilBt,
        &[
            L("yssaril:bt:"),
            F(K::Word(BT_KINDS)),
            L(":"),
            SEAT,
            L(":"),
            SEAT,
        ],
    ),
    (
        Family::YssarilSsruuRoundAgent,
        &[
            L("yssaril:ssruu-round-agent:"),
            SEAT,
            L(":"),
            LEADER,
            L(":"),
            DEC,
        ],
    ),
];

/// The literal a group or field is introduced by, when it has one.
fn first_literal(segs: &[Seg]) -> Option<&'static str> {
    match segs.first()? {
        L(lit) => Some(lit),
        O(group) => first_literal(group),
        F(_) => None,
    }
}

/// How many fields a pattern has.
fn field_count(segs: &[Seg]) -> usize {
    segs.iter()
        .map(|seg| match seg {
            F(_) => 1,
            O(group) => field_count(group),
            L(_) => 0,
        })
        .sum()
}

impl Codec {
    /// The typed key of a `faction_marks` key text; `None` for text no family has (a test key,
    /// an id outside the corpus, an owner outside the table).
    pub fn parse_mark_key(&mut self, text: &str) -> Option<MarkKey> {
        for (family, segs) in &PATTERNS {
            let mut rest = text;
            let mut atoms = Vec::new();
            if self.parse_segs(segs, &mut rest, &mut atoms, None).is_some() && rest.is_empty() {
                if let Some(key) = MarkKey::from_atoms(*family, &atoms) {
                    return Some(key);
                }
            }
        }
        None
    }

    fn parse_segs(
        &mut self,
        segs: &[Seg],
        rest: &mut &str,
        atoms: &mut Vec<Atom>,
        follow: Option<&str>,
    ) -> Option<()> {
        for (i, seg) in segs.iter().enumerate() {
            match seg {
                L(lit) => *rest = rest.strip_prefix(lit)?,
                F(kind) => {
                    let delimiter = match segs.get(i + 1) {
                        Some(next) => first_literal(std::slice::from_ref(next)),
                        None => follow,
                    };
                    let end = match delimiter {
                        Some(d) => rest.find(d).unwrap_or(rest.len()),
                        None => rest.len(),
                    };
                    let (field, tail) = rest.split_at(end);
                    atoms.push(self.parse_atom(*kind, field)?);
                    *rest = tail;
                }
                O(group) => {
                    let present = first_literal(group).is_some_and(|lit| rest.starts_with(lit));
                    if present {
                        self.parse_segs(group, rest, atoms, follow)?;
                    } else {
                        atoms.extend(std::iter::repeat_n(Atom::Absent, field_count(group)));
                    }
                }
            }
        }
        Some(())
    }

    /// The text of a typed key.
    #[must_use]
    pub fn render_mark_key(&self, key: &MarkKey) -> Option<String> {
        let (_, segs) = &PATTERNS[key.family().index()];
        let (atoms, n) = key.atoms();
        let mut out = String::new();
        let mut cursor = 0;
        self.render_segs(segs, &atoms[..n], &mut cursor, &mut out)?;
        (cursor == n).then_some(out)
    }

    fn render_segs(
        &self,
        segs: &[Seg],
        atoms: &[Atom],
        cursor: &mut usize,
        out: &mut String,
    ) -> Option<()> {
        for seg in segs {
            match seg {
                L(lit) => out.push_str(lit),
                F(kind) => {
                    self.render_atom(*kind, *atoms.get(*cursor)?, out)?;
                    *cursor += 1;
                }
                O(group) => {
                    if atoms.get(*cursor) == Some(&Atom::Absent) {
                        *cursor += field_count(group);
                    } else {
                        self.render_segs(group, atoms, cursor, out)?;
                    }
                }
            }
        }
        Some(())
    }
}
