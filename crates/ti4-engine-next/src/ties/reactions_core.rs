//! The adapter half of the core ability table (package `core-reactions`; docs/ENGINE_REACTIONS.md): which string
//! ability ids the core table has a rule for, the registry the core builds from it.
//!
//! * [`table`]: the core `AbilityTable` with every ported rule.
//! * [`slot_of`] / [`text_of`]: the mapping between a registered string id
//!   (`technology:hacan:inf2:GROUND_FORCE_DESTROYED:after`) and the owner-independent [`Slot`].
//! (The legacy module also follows a string run with a core window, `Recording` and `follow`; that lockstep half stays in the legacy crate.)

use ti4_engine_core::choice::reaction::AbilityKind;
use ti4_engine_core::kernel::ability::{AbilityTable, Slot, Spec};

use ti4_engine_core_factions::cb_economy::cabal::timing as cabal;

use crate::kernel_sync::text::{event_text, relation_text};
use crate::render::Render;

/// One ported ability rule, named: the registered id of an owner is `{kind}:{owner}:{card}:{EVENT}:{rel}`; the
/// event and the relation are the spec's. The reaction-card slots have no card segment.
struct Named {
    slot: Slot,
    kind: &'static str,
    card: &'static str,
}

/// The texts the rules of `silent::g3` name in their questions (`silent::g3::code`, the same places); unused places hold a placeholder.
const G3_CODE_TEXTS: [&str; 16] = ["nekro_flagship", "technology:exo2", "g3_unused", "g3_unused", "g3_unused", "g3_unused", "g3_unused", "g3_unused", "g3_unused", "g3_unused", "g3_unused", "g3_unused", "g3_unused", "g3_unused", "g3_unused", "g3_unused"];

/// The texts the rules of `silent::g4` name (`silent::g4::code`, the same places); unused places hold a placeholder.
const G4_CODE_TEXTS: [&str; 16] = ["titanscommander", "md", "87", "yso", "naalu_mech_te", "naalu", "strategy", "g4_unused", "g4_unused", "g4_unused", "g4_unused", "g4_unused", "g4_unused", "g4_unused", "g4_unused", "g4_unused"];

/// The ported rules with a card key, in table order (the per-owner registration order of `reactions::arm`).
const NAMED: &[Named] = &[
    Named { slot: ti4_engine_core_factions::cb_economy::argent::timing::AGENT, kind: "leader", card: "argentagent" },
    Named { slot: ti4_engine_core::rules::reactions::instinct_training::INSTINCT_TRAINING, kind: "technology", card: "it" },
    Named { slot: ti4_engine_core::rules::reactions::l1z1x_agent::I48S, kind: "leader", card: "l1z1xagent" },
    Named { slot: ti4_engine_core::rules::reactions::tellurian::HITS, kind: "leader", card: "titansagent" },
    Named { slot: ti4_engine_core::rules::reactions::tellurian::CANNON_HITS, kind: "leader", card: "titansagent" },
    Named { slot: ti4_engine_core::rules::reactions::tellurian::GROUND_HITS, kind: "leader", card: "titansagent" },
    Named { slot: ti4_engine_core::rules::reactions::tellurian::BARRAGE_HITS, kind: "leader", card: "titansagent" },
    Named { slot: ti4_engine_core_factions::ce_movement::arborec::timing::WARRIOR_ROLLS, kind: "unit", card: "arborec_infantry2" },
    Named { slot: ti4_engine_core_factions::cd_registry::yin::timing::MILOR_SHIPS, kind: "leader", card: "yinagent" },
    Named { slot: ti4_engine_core_factions::cd_registry::yin::timing::MILOR_GROUND, kind: "leader", card: "yinagent" },
    Named { slot: ti4_engine_core_factions::cd_registry::yin::timing::INDOCTRINATION, kind: "ability", card: "indoctrination" },
    Named { slot: ti4_engine_core_factions::cd_registry::yin::timing::GREYFIRE, kind: "promissory", card: "greyfire" },
    Named { slot: ti4_engine_core_factions::cd_registry::titans::offer::TERRAGENESIS, kind: "ability", card: "terragenesis" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::timing::ANTIVIRUS_SPACE, kind: "promissory", card: "antivirus" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::timing::ANTIVIRUS_GROUND, kind: "promissory", card: "antivirus" },
    Named { slot: ti4_engine_core_factions::magen_defense::ACTIVATION, kind: "technology", card: "md" },
    Named { slot: ti4_engine_core_factions::cb_economy::winnu::timing::UNLOCK_SPACE, kind: "leader", card: "winnucommander_unlock" },
    Named { slot: ti4_engine_core_factions::cb_economy::winnu::timing::UNLOCK_GROUND, kind: "leader", card: "winnucommander_unlock" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::threat::PREDICT, kind: "ability", card: "galactic_threat" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::threat::PAYOUT, kind: "ability", card: "galactic_threat" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::singularity::SPACE, kind: "ability", card: "technological_singularity" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::singularity::GROUND, kind: "ability", card: "technological_singularity" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::singularity::OPEN_SPACE, kind: "ability", card: "technological_singularity_open" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::singularity::OPEN_GROUND, kind: "ability", card: "technological_singularity_open" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::singularity::CLOSE_SPACE, kind: "ability", card: "technological_singularity_close" },
    Named { slot: ti4_engine_core_factions::ca_combat::nekro::singularity::CLOSE_GROUND, kind: "ability", card: "technological_singularity_close" },
    Named { slot: ti4_engine_core_factions::cd_registry::sardakk::timing::HERO_FINISH, kind: "leader", card: "sardakkhero" },
    Named { slot: ti4_engine_core_factions::cd_registry::sardakk::timing::MECH_HIT, kind: "unit", card: "sardakk_mech" },
    Named { slot: ti4_engine_core_factions::cd_registry::sardakk::timing::TRO, kind: "leader", card: "sardakkagent" },
    Named { slot: ti4_engine_core_factions::ce_movement::empyrean::timing::DYNAMO_SHIP, kind: "unit", card: "empyrean_flagship" },
    Named { slot: ti4_engine_core_factions::ce_movement::empyrean::timing::DYNAMO_GROUND, kind: "unit", card: "empyrean_flagship" },
    Named { slot: ti4_engine_core_factions::ce_movement::empyrean::timing::WATCHER, kind: "unit", card: "empyrean_mech" },
    Named { slot: ti4_engine_core_factions::ce_movement::empyrean::timing::PASSAGE, kind: "ability", card: "aetherpassage" },
    Named { slot: ti4_engine_core_factions::ce_movement::empyrean::timing::STREAM, kind: "technology", card: "as" },
    Named { slot: ti4_engine_core_factions::ce_movement::nomad::timing::FUTURE_SIGHT, kind: "ability", card: "future_sight" },
    Named { slot: ti4_engine_core_factions::ce_movement::nomad::timing::CAVALRY_PLAY, kind: "promissory", card: "cavalry" },
    Named { slot: ti4_engine_core_factions::ce_movement::nomad::timing::CAVALRY_RETURN, kind: "promissory", card: "cavalry" },
    Named { slot: ti4_engine_core_factions::ce_movement::nomad::timing::ARTUNO, kind: "leader", card: "nomadagentartuno" },
    Named { slot: ti4_engine_core_factions::ce_movement::nomad::timing::TCS, kind: "technology", card: "tcs" },
    Named { slot: ti4_engine_core_factions::cc_ground::mahact::timing::LEGIONNAIRE, kind: "unit", card: "mahact_infantry" },
    Named { slot: ti4_engine_core_factions::cc_ground::mahact::timing::LEGIONNAIRE_II, kind: "unit", card: "mahact_infantry2" },
    Named { slot: ti4_engine_core_factions::cc_ground::mentak::timing::SALVAGE_TRADE_GOOD, kind: "technology", card: "so" },
    Named { slot: ti4_engine_core_factions::cc_ground::mentak::timing::SALVAGE_SHIP, kind: "technology", card: "so" },
    Named { slot: ti4_engine_core_factions::cc_ground::mentak::timing::HERO_START, kind: "leader", card: "mentakhero" },
    Named { slot: ti4_engine_core_factions::cc_ground::mentak::timing::HERO_SHIPS, kind: "leader", card: "mentakhero" },
    Named { slot: ti4_engine_core_factions::cc_ground::mentak::timing::HERO_END, kind: "leader", card: "mentakhero" },
    Named { slot: ti4_engine_core::rules::reactions::saar::SAAR_TRADE_GOOD, kind: "tech", card: "sar" },
    Named { slot: ti4_engine_core_factions::ce_movement::nomad::timing::THUNDARIAN_SPACE, kind: "leader", card: "nomadagentthundarian" },
    Named { slot: ti4_engine_core_factions::ce_movement::nomad::timing::THUNDARIAN_GROUND, kind: "leader", card: "nomadagentthundarian" },
    Named { slot: ti4_engine_core_factions::cd_registry::yin::timing::VAN_HAUGE, kind: "unit", card: "yin_flagship" },
    Named { slot: ti4_engine_core_factions::cc_ground::borrowed::timing::SOL_AGENT, kind: "leader", card: "solagent" },
    Named { slot: ti4_engine_core_factions::cc_ground::borrowed::timing::LETNEV_AGENT, kind: "leader", card: "letnevagent" },
    Named { slot: ti4_engine_core::rules::reactions::INFANTRY_II_ROLLS, kind: "technology", card: "inf2" },
    Named { slot: cabal::REANIMATOR, kind: "unit", card: "cabal_mech" },
    Named { slot: ti4_engine_core_factions::cc_ground::mentak::timing::PROMISE, kind: "promissory", card: "pop" },
    Named { slot: cabal::CRUCIBLE, kind: "promissory", card: "crucible" },
    Named { slot: ti4_engine_core_factions::cd_registry::naalu::timing::NEUROGLAIVE, kind: "technology", card: "ng" },
    Named { slot: ti4_engine_core_factions::cc_ground::ghost::timing::TAIVRA, kind: "leader", card: "ghostagent" },
    Named { slot: ti4_engine_core_factions::cc_ground::ghost::timing::ICARUS, kind: "unit", card: "ghost_mech" },
    Named { slot: ti4_engine_core_factions::ce_movement::saar::timing::MENDOSA, kind: "leader", card: "saaragent" },
    Named { slot: ti4_engine_core_factions::cc_ground::mahact::timing::EDICT_SPACE, kind: "ability", card: "edict" },
    Named { slot: ti4_engine_core_factions::cc_ground::mahact::timing::EDICT_GROUND, kind: "ability", card: "edict" },
    Named { slot: ti4_engine_core_factions::ce_movement::arborec::timing::COMMANDER, kind: "leader", card: "arboreccommander" },
    Named { slot: ti4_engine_core_factions::ce_movement::arborec::timing::FLAGSHIP, kind: "unit", card: "arborec_flagship" },
    Named { slot: ti4_engine_core_factions::cd_registry::yssaril::timing::SO_ATA, kind: "leader", card: "yssarilcommander" },
    Named { slot: ti4_engine_core_factions::cd_registry::yssaril::timing::SO_ATA_CLEAR, kind: "leader", card: "yssarilcommander_clear" },
    Named { slot: ti4_engine_core_factions::ce_movement::nomad::timing::MERCER, kind: "leader", card: "nomadagentmercer" },
    Named { slot: ti4_engine_core_factions::ce_movement::muaat::timing::NOVA_SEED, kind: "leader", card: "muaathero" },
    Named { slot: ti4_engine_core_factions::ce_movement::muaat::timing::WAR_SUN_MOVED, kind: "leader", card: "muaathero_moved" },
    Named { slot: ti4_engine_core_factions::cd_registry::titans::timing::AWAKEN, kind: "ability", card: "awaken" },
    Named { slot: ti4_engine_core_factions::cd_registry::titans::timing::OURANOS, kind: "unit", card: "titans_flagship" },
    Named { slot: ti4_engine_core_factions::silent::gift::GIFT, kind: "promissory", card: "gift" },
    Named { slot: ti4_engine_core_factions::silent::gift::GIFT_RETURN, kind: "promissory", card: "gift_return" },
    Named { slot: ti4_engine_core_factions::silent::returns::INF2_RETURN, kind: "technology", card: "inf2_return" },
    Named { slot: ti4_engine_core_factions::silent::returns::WARRIOR_RETURN, kind: "unit", card: "arborec_infantry2_return" },
    Named { slot: ti4_engine_core_factions::silent::returns::LEGIONNAIRE_RETURN, kind: "unit", card: "mahact_infantry2_return" },
    Named { slot: ti4_engine_core_factions::silent::scavenge::SCAVENGE, kind: "ability", card: "scavenge" },
    Named { slot: ti4_engine_core_factions::silent::mitosis::MITOSIS, kind: "ability", card: "mitosis" },
    Named { slot: ti4_engine_core_factions::silent::pillage::PILLAGE_GAINED, kind: "ability", card: "pillage" },
    Named { slot: ti4_engine_core_factions::silent::pillage::PILLAGE_TRANSACTION, kind: "ability", card: "pillage" },
    Named { slot: ti4_engine_core_factions::silent::winnu::RECLAMATION_MARK, kind: "ability", card: "reclamation_mark" },
    Named { slot: ti4_engine_core_factions::silent::winnu::RECLAMATION_CLEAR, kind: "ability", card: "reclamation_clear" },
    Named { slot: ti4_engine_core_factions::silent::winnu::RECLAMATION, kind: "ability", card: "reclamation" },
    Named { slot: ti4_engine_core_factions::silent::winnu::RECLAIMER, kind: "unit", card: "winnu_mech" },
    Named { slot: ti4_engine_core_factions::silent::winnu::HTP, kind: "technology", card: "htp" },
    Named { slot: ti4_engine_core_factions::silent::winnu::AGENT, kind: "leader", card: "winnuagent" },
    Named { slot: ti4_engine_core_factions::silent::winnu::RECLAMATION_TACTICAL, kind: "ability", card: "reclamation_tactical" },
    Named { slot: ti4_engine_core_factions::silent::naaz::AGENT, kind: "leader", card: "naazagent" },
    Named { slot: ti4_engine_core_factions::silent::naaz::COMMANDER, kind: "leader", card: "naazcommander" },
    Named { slot: ti4_engine_core_factions::silent::sol::CLAIRE_GIBSON, kind: "leader", card: "solcommander" },
    Named { slot: ti4_engine_core_factions::silent::strike_wing::SWA2, kind: "technology", card: "swa2" },
    Named { slot: ti4_engine_core_factions::silent::strike_wing::AMBUSCADE, kind: "promissory", card: "ambuscade" },
    Named { slot: ti4_engine_core_factions::silent::strike_wing::COMMANDER, kind: "leader", card: "argentcommander" },
    Named { slot: ti4_engine_core_factions::silent::cabal::STILLNESS, kind: "leader", card: "cabalagent" },
    Named { slot: ti4_engine_core_factions::silent::empyrean::ACAMAR, kind: "leader", card: "empyreanagent" },
    Named { slot: ti4_engine_core_factions::silent::empyrean::VOIDWATCH, kind: "technology", card: "vw" },
    Named { slot: ti4_engine_core_factions::silent::empyrean::XUANGE, kind: "leader", card: "empyreancommander" },
    Named { slot: ti4_engine_core_factions::silent::ghost::MOVED, kind: "leader", card: "ghostcommander" },
    Named { slot: ti4_engine_core_factions::silent::ghost::FINISHED, kind: "leader", card: "ghostcommander" },
    Named { slot: ti4_engine_core_factions::silent::muaat::MAGMUS, kind: "leader", card: "muaatcommander" },
    Named { slot: ti4_engine_core_factions::silent::muaat::UNLOCK, kind: "leader", card: "muaatcommander_unlock" },
    Named { slot: cabal::DEVOUR_SHIPS, kind: "ability", card: "devour" },
    Named { slot: cabal::DEVOUR_GROUND, kind: "ability", card: "devour" },
    Named { slot: cabal::TERROR_SHIPS, kind: "unit", card: "cabal_flagship" },
    Named { slot: cabal::TERROR_GROUND, kind: "unit", card: "cabal_flagship" },
    Named { slot: cabal::MARK_SPACE, kind: "ability", card: "devour_mark" },
    Named { slot: cabal::CLEAR_SPACE, kind: "ability", card: "devour_clear" },
    Named { slot: cabal::MARK_GROUND, kind: "ability", card: "devour_mark" },
    Named { slot: cabal::CLEAR_GROUND, kind: "ability", card: "devour_clear" },
    Named { slot: ti4_engine_core_factions::silent::g3::zeta::ZETA, kind: "unit", card: "saar_mech" },
    Named { slot: ti4_engine_core_factions::silent::g3::ragh::RAGH, kind: "promissory", card: "ragh" },
    Named { slot: ti4_engine_core_factions::silent::g3::alastor::ALASTOR, kind: "unit", card: "nekro_flagship" },
    Named { slot: ti4_engine_core_factions::silent::g3::exo2::EXO2, kind: "technology", card: "exo2" },
    Named { slot: ti4_engine_core_factions::silent::g3::zeu::ZEU, kind: "leader", card: "naaluagent-te" },
    Named { slot: ti4_engine_core_factions::silent::g3::nekro_commander::ACIDOS, kind: "leader", card: "nekrocommander" },
    Named { slot: ti4_engine_core_factions::silent::g3::suffi_an::SUFFI_AN, kind: "leader", card: "mentakagent" },
    Named { slot: ti4_engine_core_factions::silent::g3::mentak_commander::SULA, kind: "leader", card: "mentakcommander" },
    Named { slot: ti4_engine_core_factions::silent::g3::bio::BIO, kind: "technology", card: "bio" },
    Named { slot: ti4_engine_core_factions::silent::g3::oracle::ORACLE, kind: "leader", card: "naaluhero" },
    Named { slot: ti4_engine_core_factions::silent::g3::scepter::SCEPTER, kind: "promissory", card: "scepter" },
    Named { slot: ti4_engine_core_factions::silent::g3::iff::IFF, kind: "promissory", card: "iff" },
    Named { slot: ti4_engine_core_factions::silent::g3::tro_marks::HERO_TURN, kind: "leader", card: "sardakkhero_turn" },
    Named { slot: ti4_engine_core_factions::silent::g3::tekklar::TEKKLAR, kind: "promissory", card: "tekklar" },
    Named { slot: ti4_engine_core_factions::silent::g3::tekklar::TEKKLAR_END, kind: "promissory", card: "tekklar_end" },
    Named { slot: ti4_engine_core_factions::silent::g3::tekklar::TEKKLAR_TURN, kind: "promissory", card: "tekklar_turn" },
    Named { slot: ti4_engine_core_factions::silent::g3::tro_marks::MARK, kind: "leader", card: "sardakkagent_mark" },
    Named { slot: ti4_engine_core_factions::silent::g3::tro_marks::TURN, kind: "leader", card: "sardakkagent_turn" },
    Named { slot: ti4_engine_core_factions::silent::g3::tro_marks::STRATEGIC, kind: "leader", card: "sardakkagent_strategic" },
    Named { slot: ti4_engine_core_factions::silent::g4::propagation::PROPAGATION, kind: "ability", card: "propagation" },
    Named { slot: ti4_engine_core_factions::silent::g4::spynet::SPY_NET, kind: "promissory", card: "spynet" },
    Named { slot: ti4_engine_core_factions::silent::g4::stymie::STYMIE, kind: "promissory", card: "stymie" },
    Named { slot: ti4_engine_core_factions::silent::g4::magen_hit::MD_HIT, kind: "technology", card: "md_hit" },
    Named { slot: ti4_engine_core_factions::silent::g4::yin_spinner::YIN_SPINNER, kind: "technology", card: "yso" },
    Named { slot: ti4_engine_core_factions::silent::g4::iconoclast::ICONOCLAST, kind: "unit", card: "naalu_mech_te" },
    Named { slot: ti4_engine_core_factions::silent::g4::foresight::FORESIGHT, kind: "ability", card: "foresight" },
    Named { slot: ti4_engine_core_factions::silent::g4::titans_commander::TUNGSTANTUS, kind: "leader", card: "titanscommander" },
];

/// The last segment of a registered id: the relation, except for Salvage Operations' produce half, which the string
/// registers as `...:SPACE_COMBAT_ENDED:produce` although it is an `after` ability.
fn rel_segment(spec: &Spec) -> &'static str {
    if spec.slot == ti4_engine_core_factions::cc_ground::mentak::timing::SALVAGE_SHIP { "produce" } else { relation_text(spec.relation) }
}

/// The event segment of a registered id: the spec's event, except for T'ro, whose id says `ACTION_COMPLETED` although the string registers it on
/// `TACTICAL_ACTION_ENDED` (the id was written for the event it was first hung on).
fn event_segment(r: &Render, spec: &Spec) -> String {
    if spec.slot == ti4_engine_core_factions::cd_registry::sardakk::timing::TRO { "ACTION_COMPLETED".to_owned() } else { event_text(r, spec.event) }
}

/// How the string spells the id of a pair rule (Ssruu's copies): `A` is `leader:{lender faction}:{lender}:yssarilagent:{agent}:EVENT:rel`,
/// `B` is `leader:ssruu-copy:{borrower}:{agent}:{lender}:EVENT:after`, `C` (Captain Mendosa's) is
/// `leader:{lender faction}:{lender}:yssarilagent:{borrower}:{agent}:EVENT:rel`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PairStyle {
    A,
    B,
    C,
}

const PAIRS: &[(Slot, &str, PairStyle)] = &[
    (ti4_engine_core_factions::cd_registry::yssaril::ssruu::YIN_SHIPS, "yinagent", PairStyle::A),
    (ti4_engine_core_factions::cd_registry::yssaril::ssruu::YIN_GROUND, "yinagent", PairStyle::A),
    (ti4_engine_core_factions::cd_registry::yssaril::ssruu::SOL, "solagent", PairStyle::B),
    (ti4_engine_core_factions::cd_registry::yssaril::ssruu::LETNEV, "letnevagent", PairStyle::B),
    (ti4_engine_core_factions::cd_registry::yssaril::ssruu::ARGENT, "argentagent", PairStyle::A),
    (ti4_engine_core_factions::ce_movement::saar::timing::MENDOSA_COPY, "saaragent", PairStyle::C),
    (ti4_engine_core_factions::cc_ground::ghost::timing::TAIVRA_COPY, "ghostagent", PairStyle::A),
    (ti4_engine_core_factions::cd_registry::sardakk::timing::TRO_COPY, "sardakkagent", PairStyle::A),
    (ti4_engine_core_factions::silent::winnu::AGENT_COPY, "winnuagent", PairStyle::A),
    (ti4_engine_core_factions::silent::naaz::AGENT_COPY, "naazagent", PairStyle::A),
    (ti4_engine_core_factions::silent::g3::suffi_an::SUFFI_AN_COPY, "mentakagent", PairStyle::A),
    (ti4_engine_core_factions::cd_registry::yssaril::ssruu::L1Z1X, "l1z1xagent", PairStyle::B),
];

fn is_reaction_slot(slot: Slot) -> bool {
    let base = ti4_engine_core::rules::action_cards::reaction_slot::SLOT_BASE;
    (base..base + 100).contains(&slot.0)
}

/// Equal for the copies of one printed action card (`action_cards::name_of`).
fn card_name_key(card: ti4_engine_core::generated::ActionCard) -> u32 {
    use crate::generated::IdStr;
    static KEYS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let names = KEYS.get_or_init(|| {
        let content = ti4_content::ContentStore::embedded();
        ti4_engine_core::generated::ActionCard::ALL
            .iter()
            .map(|c| crate::ties::action_cards::name_of(content, &ti4_model::id::ActionCardId::new(c.as_str())))
            .collect()
    });
    let mine = &names[card as usize];
    u32::try_from(names.iter().position(|n| n == mine).unwrap_or(0)).unwrap_or(0)
}

thread_local! {
    /// `(player, system, neighbours)`: the systems adjacent to `system` as `player` sees them, for the window being
    /// run (`PlayerAdjacency::neighbours`; the core is lent no galaxy).
    static REACH: std::cell::RefCell<Vec<(String, String, Vec<String>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Run `body` with the adjacency facts of the window installed (read by [`follow`]).
pub fn with_reach<T>(entries: Vec<(String, String, Vec<String>)>, body: impl FnOnce() -> T) -> T {
    let before = REACH.with(|v| v.replace(entries));
    let out = body();
    REACH.with(|v| *v.borrow_mut() = before);
    out
}

thread_local! {
    /// The texts of the staged rerolls of the window being run (Fire Team, Scramble Frequency).
    static REROLL: std::cell::RefCell<Option<ti4_engine_core::kernel::ability::RerollFacts>> = const { std::cell::RefCell::new(None) };
}

/// Run `body` with the reroll texts of the window installed (read by [`follow`]).
pub fn with_reroll<T>(facts: ti4_engine_core::kernel::ability::RerollFacts, body: impl FnOnce() -> T) -> T {
    let before = REROLL.with(|v| v.replace(Some(facts)));
    let out = body();
    REROLL.with(|v| *v.borrow_mut() = before);
    out
}

/// The registered id of `spec` for an owner called `owner`.
pub(crate) fn id_text(r: &Render, spec: &Spec, owner: &str, owner_seat: &str, lender: Option<&(String, String)>) -> String {
    let (event, rel) = (event_segment(r, spec), rel_segment(spec));
    if let (Some((lender_faction, lender_seat)), Some((_, agent, style))) = (lender, PAIRS.iter().find(|p| p.0 == spec.slot)) {
        return match style {
            PairStyle::A => format!("leader:{lender_faction}:{lender_seat}:yssarilagent:{agent}:{event}:{rel}"),
            PairStyle::B => format!("leader:ssruu-copy:{owner_seat}:{agent}:{lender_seat}:{event}:{rel}"),
            PairStyle::C => format!("leader:{lender_faction}:{lender_seat}:yssarilagent:{owner_seat}:{agent}:{event}:{rel}"),
        };
    }
    if is_reaction_slot(spec.slot) {
        return format!("reaction:{owner}:{event}:{rel}");
    }
    if let Some(gap) = crate::ties::ability_gaps::index_of(spec.slot) {
        return crate::ties::ability_gaps::display_id(gap, owner);
    }
    let n = NAMED.iter().find(|n| n.slot == spec.slot).expect("every spec is named");
    format!("{}:{owner}:{}:{event}:{rel}", n.kind, n.card)
}

/// The core table: every ported ability rule.
#[must_use]
pub fn table() -> AbilityTable {
    static TABLE: std::sync::OnceLock<AbilityTable> = std::sync::OnceLock::new();
    TABLE
        .get_or_init(|| {
            let mut t = AbilityTable::default();
            t.bars = Some(ti4_engine_core::rules::agenda::political_secret_bars);
            t.card_name = Some(card_name_key);
            // The string order of `reactions::arm`, per owner: the reaction slots, then Instinct Training, I48S,
            // Infantry II, Magen Defense, the factions.
            ti4_engine_core_factions::cc_ground::register_first(&mut t);
            ti4_engine_core::rules::action_cards::reaction_slot::register(&mut t);
            ti4_engine_core::rules::reactions::register(&mut t);
            ti4_engine_core_factions::magen_defense::register_abilities(&mut t);
            ti4_engine_core_factions::silent::g4::magen_hit::register_abilities(&mut t);
            ti4_engine_core_factions::cb_economy::register_abilities(&mut t);
            ti4_engine_core_factions::silent::register_before_movement(&mut t);
            ti4_engine_core_factions::ce_movement::register_abilities(&mut t);
            // The string order of the faction modules: only abilities of one window share a bucket, and Mahact's
            // Legionnaire and Yin's agent do (GROUND_FORCE_DESTROYED), Mentak's Salvage Operations and the Nomad's
            // Cavalry (SPACE_COMBAT_ENDED).
            ti4_engine_core_factions::cc_ground::register_abilities(&mut t);
            ti4_engine_core_factions::ca_combat::register_abilities(&mut t);
            ti4_engine_core_factions::silent::g3::register_after_ca_combat(&mut t);
            ti4_engine_core_factions::silent::register_before_muaat(&mut t);
            ti4_engine_core_factions::ce_movement::register_late_abilities(&mut t);
            ti4_engine_core_factions::cb_economy::register_late_abilities(&mut t);
            ti4_engine_core_factions::cd_registry::register_early_abilities(&mut t);
            ti4_engine_core_factions::activation::register(&mut t);
            ti4_engine_core_factions::silent::register_abilities(&mut t);
            ti4_engine_core_factions::cd_registry::register_last_abilities(&mut t);
            ti4_engine_core::rules::reactions::register_after_factions(&mut t);
            ti4_engine_core_factions::cards::register(&mut t);
            // The links to the areas whose windows an action card runs as children: a play window that follows nested
            // activity runs on this table (`follow`), not on `action_cards_view::play_table`.
            ti4_engine_core::driver::children::register(&mut t);
            // The offer of Self-Assembly Routines at the end of a use of PRODUCTION a hero or a card runs (a window of the faction crate).
            ti4_engine_core_factions::self_assembly::register_links(&mut t);
            t.facts.card_groups = card_groups();
            t
        })
        .clone()
}

fn specs() -> Vec<Spec> {
    table().specs().to_vec()
}

#[allow(dead_code)]
fn kind_text(kind: AbilityKind) -> &'static str {
    match kind {
        AbilityKind::Ability => "ability",
        AbilityKind::Leader => "leader",
        AbilityKind::Breakthrough => "breakthrough",
        AbilityKind::Technology => "technology",
        AbilityKind::Unit => "unit",
        AbilityKind::Promissory => "promissory",
        AbilityKind::Reaction => "reaction",
        AbilityKind::Other => "other",
    }
}

/// The slot of a registered string id (`kind:owner:card:EVENT:rel`, or `reaction:owner:EVENT:rel`), if the table
/// has a rule for it.
#[must_use]
pub fn slot_of(id: &str) -> Option<Slot> {
    let parts: Vec<&str> = id.split(':').collect();
    let r = Render::new(Vec::new());
    if parts.len() == 4 && parts[0] == "reaction" {
        let w = ti4_engine_core::rules::action_cards::reaction_slot::windows();
        let at = w.iter().position(|(e, rel)| event_text(&r, *e) == parts[2] && relation_text(*rel) == parts[3])?;
        return Some(Slot(ti4_engine_core::rules::action_cards::reaction_slot::SLOT_BASE + u16::try_from(at).ok()?));
    }
    // Ssruu's copies: the lender and the agent are inside the id.
    if parts.len() == 8 && parts[0] == "leader" && parts[3] == "yssarilagent" {
        let all = specs();
        return PAIRS.iter().filter(|p| p.2 == PairStyle::C && p.1 == parts[5]).find(|p| all.iter().find(|s| s.slot == p.0).is_some_and(|s| event_text(&r, s.event) == parts[6] && relation_text(s.relation) == parts[7])).map(|p| p.0);
    }
    if parts.len() == 7 && parts[0] == "leader" {
        let all = specs();
        let found = if parts[1] == "ssruu-copy" {
            PAIRS.iter().filter(|p| p.2 == PairStyle::B && p.1 == parts[3]).find(|p| all.iter().find(|s| s.slot == p.0).is_some_and(|s| event_text(&r, s.event) == parts[5] && relation_text(s.relation) == parts[6]))
        } else if parts[3] == "yssarilagent" {
            PAIRS.iter().filter(|p| p.2 == PairStyle::A && p.1 == parts[4]).find(|p| all.iter().find(|s| s.slot == p.0).is_some_and(|s| event_text(&r, s.event) == parts[5] && relation_text(s.relation) == parts[6]))
        } else {
            None
        };
        if let Some(p) = found {
            return Some(p.0);
        }
    }
    if parts.len() < 5 {
        return None;
    }
    let (kind, rest) = (parts[0], &parts[2..]);
    let (event, rel) = (rest[rest.len() - 2], rest[rest.len() - 1]);
    let card = rest[..rest.len() - 2].join(":");
    let all = specs();
    NAMED.iter().filter(|n| n.kind == kind && n.card == card).find_map(|n| {
        let spec = all.iter().find(|s| s.slot == n.slot)?;
        (event_segment(&r, spec) == event && rel_segment(spec) == rel).then_some(n.slot)
    })
}

/// The printed name of each card as the index of the first card with the same name, and whether the string side has
/// an effect for each card: constant, built once.
fn card_tables() -> &'static (Vec<u8>, Vec<bool>) {
    static T: std::sync::OnceLock<(Vec<u8>, Vec<bool>)> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        use crate::generated::IdStr;
        let content = ti4_content::ContentStore::embedded();
        let names: Vec<String> = ti4_engine_core::generated::ActionCard::ALL
            .iter()
            .map(|c| crate::ties::action_cards::name_of(content, &ti4_model::id::ActionCardId::new(c.as_str())))
            .collect();
        let group = (0..names.len()).map(|i| u8::try_from(names.iter().position(|n| *n == names[i]).unwrap_or(i)).unwrap_or(u8::MAX)).collect();
        // Every action card has an effect on the string side (`tests/ties_parity.rs` checks `action_cards::effect_for` of the legacy crate).
        let effect = vec![true; ti4_engine_core::generated::ActionCard::ALL.len()];
        (group, effect)
    })
}

/// The printed-name group of each action card (the index of the first card with the same name): the windows that offer one
/// option per name outside a card play (the hand limit's discard) read it from the table's facts.
pub(crate) fn card_groups() -> Vec<u8> {
    card_tables().0.clone()
}

/// The cards the string side has an effect for (`action_cards::effect_for`), by card index: [`PlayFacts::has_effect`].
pub(crate) fn card_effects() -> Vec<bool> {
    card_tables().1.clone()
}




/// The interned texts the ability table's rules compare by (`AbilityTable::codes`) and the pay kernel: functions of the render
/// tables alone (package KF2: the live path sets them without a string position).
pub(crate) fn set_table_codes(table: &mut AbilityTable, r: &Render) {
    // The same texts always get the same codes in one `Render`, and the codes are all `set_table_codes_uncached` writes into `table.codes`: after the
    // first call the table takes them as they were (a hundred lookups in the code table per step otherwise, docs/PERF_WHY.md).
    if let Some(codes) = r.cached_table_codes() {
        table.codes = codes;
        table.facts.pay_kernel = Some(crate::ties::production_view::kernel);
        table.facts.production_kernel = Some(crate::ties::production_view::kernel_for_core);
        return;
    }
    set_table_codes_uncached(table, r);
    r.cache_table_codes(table.codes);
}

fn set_table_codes_uncached(table: &mut AbilityTable, r: &Render) {
        table.codes.ground_combat = r.code("ground_combat");
        table.codes.harrow = r.code("harrow");
        table.codes.self_assembly = r.code("self_assembly_routines");
        table.codes.van_hauge = r.code("unit_ability:yin_flagship");
        table.codes.action_card = r.code("action_card");
        table.codes.supply_base = r.code("forward_supply_base");
        table.codes.mentak = r.code("mentak");
        table.codes.future_sight = r.code("future_sight");
        table.codes.instinct_training = r.code("instinct_training");
        table.codes.direct_hit = r.code("action_card:direct_hit");
        table.codes.cavalry = r.code("cavalry");
        table.codes.technological_singularity = r.code("technological_singularity");
        table.codes.galactic_threat = r.code("galactic_threat");
        table.codes.empyrean_mech = r.code("empyrean_mech");
        table.codes.salvage = r.code("so");
        table.codes.indoctrination = r.code("indoctrination");
        table.codes.terragenesis = r.code("terragenesis");
        table.facts.pay_kernel = Some(crate::ties::production_view::kernel);
        table.facts.production_kernel = Some(crate::ties::production_view::kernel_for_core);
        table.codes.mahact_infantry = r.code("mahact_infantry");
        table.codes.mahact_infantry2 = r.code("mahact_infantry2");
        table.codes.retreat_capacity = r.code("retreat_capacity");
        table.codes.reinforcements = r.code("reinforcements");
        table.codes.assign_casualty = r.code("assign_casualty");
        table.codes.reflective_cause = r.code("action_card:reflective_shielding");
        table.codes.not_from_supply = ["transaction", "pillage", "extreme_duress", "trade_agreement", "artuno"].map(|t| r.code(t));
        crate::ties::production_live::fill_codes(table, r);
        crate::ties::invasion::fill_codes(table, r);
        table.codes.tactical = ti4_engine_core::kernel::ability::TacticalCodes {
            tactic: r.code("tactic"),
            space_cannon: r.code("space_cannon"),
            space_cannon_cause: r.code("unit_ability:space_cannon"),
            casualty: r.code("assign_casualty"),
            commander: r.code("letnevcommander"),
            pre_roll: r.code("pre_roll"),
            faction_technology: r.code("faction_technology"),
            nullification_field: r.code("nullification_field"),
            starlancer: r.code("mahact_mech"),
            peace_prompt: r.code(crate::ties::tactical_live::PEACE_PROMPT),
            peace_source: r.code("minister_peace"),
        };
        table.codes.activation = ti4_engine_core::kernel::ability::ActivationCodes { ghost_mech: r.code("ghost_mech"), saar_agent: r.code("saaragent"), sardakk_agent: r.code("sardakkagent"), edict: r.code("edict"), awaken: r.code("awaken"), titans_flagship: r.code("titans_flagship"), yssaril_commander: r.code("yssarilcommander"), mercer: r.code("nomadagentmercer"), nova_seed_cause: r.code("breakthrough:nova_seed") };
        table.codes.turn = crate::live::turn::codes(r);
        table.codes.agenda = crate::live::agenda::codes(r);
        table.codes.g2 = ti4_engine_core::kernel::ability::G2Codes { winnu_mech: r.code("winnu_mech"), muaat: r.code("muaat"), cabal_agent: r.code("cabalagent") };
        table.codes.silent = ti4_engine_core::kernel::ability::SilentCodes {
            lw2: r.code("lw2"),
            scavenge: r.code("scavenge"),
            mitosis: r.code("mitosis"),
            naaz_agent: r.code("naazagent"),
            ..Default::default()
        };
        table.codes.silent.g3 = G3_CODE_TEXTS.map(|text| r.code(text));
        table.codes.silent.g4 = G4_CODE_TEXTS.map(|text| r.code(text));
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Integration 18 (lane A's copy x lane R's marks): lane R made the Sardakk agent's activation mark and the Winnu's note table rules
    /// (slots `tro_marks::MARK`, `winnu::RECLAMATION_TACTICAL`) and named them in the string-side `NAMED` table of `reactions_core`; lane A's copy
    /// of that table was cut before. A spec without a name panics in `id_text` ("every spec is named"), i.e. in the first window of a game with
    /// such a seat.
    #[test]
    fn every_rule_of_the_core_table_has_a_name_and_the_two_marks_are_found_by_their_ids() {
        for spec in specs() {
            if !is_reaction_slot(spec.slot) && crate::ties::ability_gaps::index_of(spec.slot).is_none() && !PAIRS.iter().any(|p| p.0 == spec.slot) {
                assert!(NAMED.iter().any(|n| n.slot == spec.slot), "no name for slot {:?}", spec.slot);
            }
        }
        assert_eq!(slot_of("leader:x:sardakkagent_mark:SYSTEM_ACTIVATED:after"), Some(ti4_engine_core_factions::silent::g3::tro_marks::MARK));
        assert_eq!(slot_of("ability:x:reclamation_tactical:SYSTEM_ACTIVATED:after"), Some(ti4_engine_core_factions::silent::winnu::RECLAMATION_TACTICAL));
    }
}
