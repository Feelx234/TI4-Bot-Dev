//! Package B4c: the transaction window runs on the core beside the string window (`trade_window_dual`,
//! `rules::trade_window`, `query::trade_deals`) under `trade=dual`, question by question, and agrees with it. A bail
//! is silent, so every test counts the windows the core followed (`compared`) and the ones it left to the string
//! (`bailed`).

#[cfg(test)]
mod tests {
use ti4_content::ContentStore;
use ti4_model::content_types::POK;
use ti4_model::id::{ActionCardId, PlayerId, SecretObjectiveId, SystemId};
use ti4_model::state::{GameState, TransientFlags};

use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
use crate::transactions::{OfferError, Traded, TradeWindow};

fn a() -> PlayerId {
    PlayerId::new("a")
}
fn b() -> PlayerId {
    PlayerId::new("b")
}

const LABEL: &str = "transactions::answer";

/// A seated table of real factions, their notes dealt, `a` and `b` (and `c`) side by side in the centre system.
fn table(factions: &[(&str, &str)]) -> (GameState, crate::fixtures::Hub) {
    let mut state = crate::fixtures::seated_game(factions, POK);
    crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
    let hub = crate::fixtures::plain_hub();
    let centre = SystemId::new(hub.centre.clone());
    for (name, _) in factions {
        crate::fixtures::put(&mut state, &centre, "cruiser", &PlayerId::new(*name), 1);
    }
    for seat in &mut state.players {
        seat.trade_goods = 2;
        seat.commodities = 3;
    }
    (state, hub)
}

fn sol_hacan() -> (GameState, crate::fixtures::Hub) {
    table(&[("a", "sol"), ("b", "hacan")])
}

struct Done {
    traded: Vec<Traded>,
    state: GameState,
    compared: u64,
    bailed: u64,
    /// The bail reasons the report counted under the window's label.
    reasons: Vec<(String, u64)>,
}

/// Open the window of `a` with `b` and answer each question with the next entry of `script`: an option id, or `~`
/// and a prefix for the first option that starts with it. `between` edits the position after step `n` (`0` is
/// after the first answer).
fn negotiate(mut state: GameState, hub: &crate::fixtures::Hub, script: &[&str], between: impl Fn(usize, &mut GameState)) -> Done {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    let content = ContentStore::embedded();
    let mut traded = Vec::new();
    with_mask(AreaMask::STRING.with(Area::TradeDiplomacy, Backend::Dual), || {
        let mut window = TradeWindow::open_with_content(&mut state, content, &a(), &b());
        for (n, want) in script.iter().enumerate() {
            let Some(choice) = window.pending_choice(&state, content) else { break };
            // The answer goes through a table, which is what records the question for the core.
            let id = match want.strip_prefix('#') {
                // The n-th deal of the list, wrapped (a random pick).
                Some(n) => {
                    let deals: Vec<&str> = choice.options.iter().filter(|o| !o.is_decline()).map(|o| o.id.as_str()).collect();
                    deals[n.parse::<usize>().unwrap() % deals.len()].to_owned()
                }
                None => match want.strip_prefix('~') {
                Some(prefix) => choice.options.iter().find(|o| o.id.starts_with(prefix)).map(|o| o.id.clone()).unwrap_or_else(|| panic!("no option {prefix}* in {:?}", choice.ids())),
                None => (*want).to_owned(),
                },
            };
            let mut table = crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new([id])));
            let answer = |c: &crate::choice::Choice, s: &mut GameState| table.ask_seeing(c, &crate::choice::Observed::new(s, content, POK, Some(&hub.galaxy)));
            let outcome = window.answer_with(&mut state, content, &hub.galaxy, &choice, answer).expect("the script answers every question");
            traded.push(outcome);
            between(n, &mut state);
            if window.is_complete() {
                break;
            }
        }
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let (compared, bailed) = crate::dual::thread_window_counts(LABEL);
    let reasons = r.windows.iter().filter(|(k, _)| k.starts_with(LABEL) && k.contains("[bail")).map(|(k, w)| (k.clone(), w.bailed)).collect();
    Done { traded, state, compared, bailed, reasons }
}

fn plain(state: GameState, hub: &crate::fixtures::Hub, script: &[&str]) -> Done {
    negotiate(state, hub, script, |_, _| {})
}

fn goods(state: &GameState, who: &str) -> i32 {
    state.player(&PlayerId::new(who)).unwrap().trade_goods
}

#[test]
fn a_swap_of_commodities_is_proposed_priced_and_struck_in_lockstep() {
    let (state, hub) = sol_hacan();
    let done = plain(state, &hub, &["cc3", "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (2, 0), "the core followed both steps");
    assert_eq!(goods(&done.state, "a"), 2 + 3);
    assert_eq!(goods(&done.state, "b"), 2 + 3);
    assert_eq!(done.state.player(&a()).unwrap().commodities, 0);
    assert_eq!(done.state.transactions_this_round, [(a(), b())]);
}

#[test]
fn goods_for_goods_and_commodity_lines_are_listed_in_the_strings_order() {
    // Every id of the grid is in the question the core derived: the dual compares the whole option list.
    for script in [["2:1", "accept"], ["ct3:2", "accept"], ["tc2:3", "accept"], ["c3:0", "accept"], ["cc1", "accept"]] {
        let (state, hub) = sol_hacan();
        let done = plain(state, &hub, &script);
        assert_eq!(done.traded, [Traded::Offered, Traded::Resolved], "{script:?}");
        assert_eq!((done.compared, done.bailed), (2, 0), "{script:?}");
    }
}

#[test]
fn a_note_is_sold_at_its_posted_price_or_given_to_a_partner_who_cannot_pay() {
    let (state, hub) = sol_hacan();
    let done = plain(state, &hub, &["~pnps:sol:2", "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (2, 0));
    assert_eq!(done.state.promissory_notes.get("ps:sol"), Some(&b()), "the note changed hands");

    let (mut poor, hub) = sol_hacan();
    poor.player_mut(&b()).unwrap().trade_goods = 0;
    let done = plain(poor, &hub, &["~pnps:sol:0", "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (2, 0));
}

#[test]
fn the_asks_for_the_partners_notes_are_priced_by_the_core() {
    for first in ["~npcf:hacan", "~cpcf:hacan", "~pcps:sol", "~nncf:sol>"] {
        let (state, hub) = sol_hacan();
        let done = plain(state, &hub, &[first, "accept"]);
        assert_eq!((done.compared, done.bailed), (2, 0), "{first}");
        assert_eq!(done.traded.len(), 2, "{first}");
    }
}

#[test]
fn a_trade_agreement_is_priced_flat_in_its_id_and_live_in_its_nets() {
    // Hacan's commodity value is 6: live worth 4.8, posted price round(2.5) = 2.
    let (state, hub) = sol_hacan();
    let done = plain(state, &hub, &["~npta:hacan:2", "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (2, 0));
    assert_eq!(done.state.promissory_notes.get("ta:hacan"), Some(&a()));
}

#[test]
fn the_support_swap_scores_a_point_for_both_sides() {
    let (state, hub) = sol_hacan();
    let before = (state.player(&a()).unwrap().victory_points, state.player(&b()).unwrap().victory_points);
    let done = plain(state, &hub, &["ss", "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (2, 0));
    assert_eq!(done.state.player(&a()).unwrap().victory_points, before.0 + 1);
    assert_eq!(done.state.player(&b()).unwrap().victory_points, before.1 + 1);
    assert_eq!(done.state.support_holders.len(), 2);
}

#[test]
fn a_lent_support_is_not_offered_again() {
    let (mut state, hub) = sol_hacan();
    state.support_holders.insert(a(), b());
    let done = plain(state, &hub, &["~cc"]);
    assert_eq!((done.compared, done.bailed), (1, 0));
}

#[test]
fn a_counter_is_the_same_deal_from_the_other_chair_and_haggling_is_bounded() {
    let (state, hub) = sol_hacan();
    // a offers, b counters (now b proposes), a counters the counter: two proposals are spent.
    let done = plain(state, &hub, &["cc3", "counter", "cc3", "counter"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Countered, Traded::Offered, Traded::NothingOffered]);
    assert_eq!((done.compared, done.bailed), (4, 0));
    assert_eq!(goods(&done.state, "a"), 2, "nothing changed hands");

    // The counter can close the deal from the other chair.
    let (state, hub) = sol_hacan();
    let done = plain(state, &hub, &["cc3", "counter", "cc3", "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Countered, Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (4, 0));
    assert_eq!(done.state.transactions_this_round, [(b(), a())], "the counter's proposer is recorded first");
}

#[test]
fn a_refusal_a_decline_and_a_proposal_of_nothing_end_the_window() {
    let (state, hub) = sol_hacan();
    let done = plain(state, &hub, &["cc3", "refuse"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Refused]);
    assert_eq!((done.compared, done.bailed), (2, 0));
    let (state, hub) = sol_hacan();
    let done = plain(state, &hub, &["decline"]);
    assert_eq!(done.traded, [Traded::NothingOffered]);
    assert_eq!((done.compared, done.bailed), (1, 0));
}

#[test]
fn a_deal_that_stopped_being_payable_is_rejected_at_the_accept() {
    let (state, hub) = sol_hacan();
    let done = negotiate(state, &hub, &["cc3", "accept"], |n, s| {
        if n == 0 {
            s.player_mut(&a()).unwrap().commodities = 0;
        }
    });
    assert_eq!(done.traded, [Traded::Offered, Traded::Rejected(OfferError::CannotPay(a()))]);
    assert_eq!((done.compared, done.bailed), (2, 0));
}

#[test]
fn strangers_cannot_deal_outside_the_agenda_phase() {
    // `b` has no unit near `a`: the proposal itself is rejected before the partner is troubled with it.
    let (mut state, hub) = table(&[("a", "sol"), ("b", "letnev")]);
    let centre = SystemId::new(hub.centre.clone());
    state.system_mut(&centre).units.retain(|u| u.owner != b());
    let done = plain(state, &hub, &["cc3"]);
    assert_eq!(done.traded, [Traded::Rejected(OfferError::NotNeighbours(a(), b()))]);
    assert_eq!((done.compared, done.bailed), (1, 0));
}

#[test]
fn any_two_seats_may_deal_in_the_agenda_phase() {
    let (mut state, hub) = table(&[("a", "sol"), ("b", "letnev")]);
    let centre = SystemId::new(hub.centre.clone());
    state.system_mut(&centre).units.retain(|u| u.owner != b());
    state.phase = ti4_model::state::Phase::Agenda;
    let done = plain(state, &hub, &["cc3", "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (2, 0));
}

#[test]
fn hacans_arbiters_put_an_action_card_on_the_table() {
    let (mut state, hub) = sol_hacan();
    state.player_mut(&a()).unwrap().action_cards = ti4_engine_core::generated::ActionCard::ALL[..2].iter().map(|c| ActionCardId::new(crate::generated::IdStr::as_str(*c))).collect();
    let done = plain(state, &hub, &["~ac", "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (2, 0));
    assert!(done.state.player(&b()).unwrap().action_cards.iter().any(|c| c.as_str() == crate::generated::IdStr::as_str(ti4_engine_core::generated::ActionCard::ALL[0]) || c.as_str() == crate::generated::IdStr::as_str(ti4_engine_core::generated::ActionCard::ALL[1])));

    let (mut state, hub) = sol_hacan();
    state.player_mut(&a()).unwrap().action_cards = vec![ActionCardId::new(crate::generated::IdStr::as_str(ti4_engine_core::generated::ActionCard::ALL[0]))];
    let done = plain(state, &hub, &["~cn", "accept"]);
    assert_eq!((done.compared, done.bailed), (2, 0));
}

#[test]
fn black_market_dealings_puts_secrets_and_fragments_on_the_table() {
    let (mut state, hub) = table(&[("a", "sol"), ("b", "letnev")]);
    state.transient_flags.set(TransientFlags::BLACK_MARKET);
    state.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new("eap")];
    state.player_mut(&a()).unwrap().relic_fragments.insert("CULTURAL".to_owned(), 2);
    let done = plain(state.clone(), &hub, &["~soeap:1", "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (2, 0));
    assert!(done.state.player(&b()).unwrap().secret_objectives.iter().any(|s| s.as_str() == "eap"));
    let done = plain(state, &hub, &["~frCULTURAL:1", "accept"]);
    assert_eq!((done.compared, done.bailed), (2, 0));
    assert_eq!(done.state.player(&b()).unwrap().relic_fragments.get("CULTURAL"), Some(&1));
}

#[test]
fn an_alliance_is_withheld_until_its_commanders_unlock_and_mahact_cannot_be_given_one() {
    let (mut state, hub) = table(&[("a", "sol"), ("b", "mahact")]);
    // Locked: no Alliance in the list (the questions are compared whole).
    let done = plain(state.clone(), &hub, &["decline"]);
    assert_eq!((done.compared, done.bailed), (1, 0));
    // Unlocked: the sol Alliance is on offer to the Mahact seat? No: Hubris. The other way it is.
    let commander = ti4_model::id::LeaderId::new("solcommander");
    state.player_mut(&a()).unwrap().leaders.insert(commander, ti4_model::state::LeaderStatus::Unlocked);
    let done = plain(state.clone(), &hub, &["decline"]);
    assert_eq!((done.compared, done.bailed), (1, 0));
    let (mut state, hub) = table(&[("a", "sol"), ("b", "hacan")]);
    state.player_mut(&a()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("solcommander"), ti4_model::state::LeaderStatus::Unlocked);
    let done = plain(state, &hub, &["~pnan:sol:", "accept"]);
    assert_eq!((done.compared, done.bailed), (2, 0));
}

// -- what the core leaves to the string ------------------------------------------------------------

fn terms(goods: i32, commodities: i32) -> crate::transactions::Terms {
    crate::transactions::Terms { trade_goods: goods, commodities, ..Default::default() }
}

/// A free-form deal (package W15): the proposer names both sides, the core reads the terms the adapter parsed from the id.
/// Was a bail ("a free-form deal") at both steps; now both steps are compared and the state agrees.
#[test]
fn a_free_form_deal_is_followed_by_the_core_and_struck_in_lockstep() {
    let (state, hub) = sol_hacan();
    // Not on the listed menu (it stops at three): 5 trade goods for 2 commodities, with the holdings to pay it.
    let mut state = state;
    state.player_mut(&a()).unwrap().trade_goods = 6;
    state.player_mut(&b()).unwrap().commodities = 4;
    state.player_mut(&b()).unwrap().trade_goods = 0;
    let id = crate::transactions::free_form_id(&terms(5, 0), &terms(0, 2));
    let done = plain(state, &hub, &[&id, "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (2, 0), "the core followed both steps: {:?}", done.reasons);
    assert_eq!(goods(&done.state, "a"), 1 + 2, "the commodities arrive as trade goods");
    assert_eq!(goods(&done.state, "b"), 5);
    assert_eq!(done.state.player(&b()).unwrap().commodities, 2);
}

#[test]
fn free_form_deals_with_every_kind_of_term_agree_with_the_string() {
    // Notes: a's Political Secret for b's Trade Agreement plus a trade good; a relic fragment; both sides at once.
    let (mut state, hub) = sol_hacan();
    state.player_mut(&a()).unwrap().relic_fragments.insert("CULTURAL".to_owned(), 2);
    let notes = |gs: &GameState, who: &str| crate::promissory::available_notes(gs, ContentStore::embedded(), &PlayerId::new(who));
    let (mine, theirs) = (notes(&state, "a"), notes(&state, "b"));
    assert!(!mine.is_empty() && !theirs.is_empty());
    let with_note = |note: &str, goods: i32| crate::transactions::Terms { promissory: Some(note.to_owned()), trade_goods: goods, ..Default::default() };
    let with_fragments = crate::transactions::Terms { fragments: vec!["CULTURAL".to_owned(), "CULTURAL".to_owned()], ..Default::default() };
    for (given, received) in [
        (with_note(&mine[0], 1), with_note(&theirs[0], 0)),
        (with_fragments.clone(), terms(2, 0)),
        (with_note(&mine[0], 0), terms(0, 3)),
        (terms(1, 1), with_note(&theirs[0], 1)),
    ] {
        let id = crate::transactions::free_form_id(&given, &received);
        let done = plain(state.clone(), &hub, &[&id, "accept"]);
        assert_eq!(done.traded.first(), Some(&Traded::Offered), "{id}: {:?}", done.traded);
        assert_eq!(done.compared + done.bailed, done.traded.len() as u64, "{id}");
        assert_eq!(done.bailed, 0, "{id}: {:?}", done.reasons);
    }
}

#[test]
fn an_illegal_free_form_deal_is_rejected_the_same_way_and_a_refusal_or_counter_follows() {
    let (state, hub) = sol_hacan();
    // More trade goods than a holds (2): rejected before the partner is asked; nothing is offered.
    let id = crate::transactions::free_form_id(&terms(9, 0), &terms(0, 0));
    let done = plain(state.clone(), &hub, &[&id]);
    assert_eq!(done.traded, [Traded::Rejected(OfferError::CannotPay(a()))]);
    assert_eq!((done.compared, done.bailed), (1, 0), "{:?}", done.reasons);
    // A planet card nobody could trade (the planets are not even real): cannot pay, the same on both sides.
    let planet = crate::transactions::Terms {
        planet: Some(crate::transactions::PlanetTerm { planet: ti4_model::id::PlanetId::new("nowhere"), moves_to: ti4_model::id::PlanetId::new("nothing") }),
        ..Default::default()
    };
    let done = plain(state.clone(), &hub, &[&crate::transactions::free_form_id(&planet, &terms(0, 1))]);
    assert_eq!(done.traded, [Traded::Rejected(OfferError::CannotPay(a()))]);
    assert_eq!(done.bailed, 0, "{:?}", done.reasons);
    // The partner refuses; or counters with a free-form deal of its own (the chairs swap).
    let id = crate::transactions::free_form_id(&terms(1, 0), &terms(0, 1));
    let done = plain(state.clone(), &hub, &[&id, "refuse"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Refused]);
    assert_eq!((done.compared, done.bailed), (2, 0), "{:?}", done.reasons);
    let back = crate::transactions::free_form_id(&terms(0, 2), &terms(2, 0));
    let done = plain(state, &hub, &[&id, "counter", &back, "accept"]);
    assert_eq!(done.traded.len(), 4, "{:?}", done.traded);
    assert_eq!(done.bailed, 0, "{:?}", done.reasons);
}

/// Hostile text: an id the string does not read offers nothing, and the core does not follow (it never sees a deal).
#[test]
fn a_free_form_id_the_string_does_not_read_is_a_bail_and_offers_nothing() {
    let (state, hub) = sol_hacan();
    for id in ["x;gt=-2;", "x;gt=2;gt=3;", "x;zz=1;", "x;gt=99999;", "x;rc=;"] {
        let done = plain(state.clone(), &hub, &[id]);
        assert_eq!(done.traded.len(), 1, "{id}");
        assert_eq!(done.traded, [Traded::NothingOffered], "{id}: the string offers nothing");
        assert_eq!((done.compared, done.bailed), (0, 1), "{id}");
        assert!(done.reasons.iter().any(|(k, _)| k.contains("a free-form deal the string does not read or the core tables do not name")), "{id}: {:?}", done.reasons);
    }
}

#[test]
fn a_table_where_two_seats_play_one_faction_is_left_to_the_string_engine() {
    let (state, hub) = table(&[("a", "sol"), ("b", "sol")]);
    let done = plain(state, &hub, &["cc3", "accept"]);
    assert_eq!(done.compared, 0);
    assert!(done.reasons.iter().any(|(k, _)| k.contains("a faction played by two seats")), "{:?}", done.reasons);
}

#[test]
fn diplomacy_keeps_the_window_on_the_string_engine() {
    let (mut state, hub) = sol_hacan();
    let ids: Vec<PlayerId> = state.players.iter().map(|x| x.id.clone()).collect();
    state.diplomacy = ti4_model::diplomacy::DiplomacyState::for_players(&ids, true);
    let done = plain(state, &hub, &["decline"]);
    assert_eq!(done.compared, 0);
}

// -- a census over random tables -------------------------------------------------------------------

struct Xor(u64);
impl Xor {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// A table of four factions with random holdings: goods, commodities, notes lent around the table (some faceup),
/// Support loans, action cards, secrets, fragments, the Black Market flag, unlocked commanders and the agenda phase.
fn random_table(rng: &mut Xor) -> (GameState, crate::fixtures::Hub) {
    use ti4_engine_core::generated::{ActionCard, SecretObjective};
    let (mut state, hub) = table(&[("a", "sol"), ("b", "hacan"), ("c", "mahact"), ("d", "jolnar")]);
    let ids = ["a", "b", "c", "d"].map(PlayerId::new);
    for seat in &mut state.players {
        seat.trade_goods = rng.below(6) as i32;
        seat.commodities = rng.below(7) as i32;
        for _ in 0..rng.below(3) {
            let card = ActionCard::ALL[rng.below(ActionCard::COUNT)];
            seat.action_cards.push(ActionCardId::new(crate::generated::IdStr::as_str(card)));
        }
        if rng.below(3) == 0 {
            let secret = SecretObjective::ALL[rng.below(SecretObjective::COUNT)];
            seat.secret_objectives.push(SecretObjectiveId::new(crate::generated::IdStr::as_str(secret)));
        }
        if rng.below(3) == 0 {
            seat.relic_fragments.insert(["CULTURAL", "FRONTIER", "HAZARDOUS", "INDUSTRIAL"][rng.below(4)].to_owned(), 1 + rng.below(2) as i32);
        }
        if rng.below(4) == 0 {
            let commander = format!("{}commander", seat.faction.as_str());
            seat.leaders.insert(ti4_model::id::LeaderId::new(commander), ti4_model::state::LeaderStatus::Unlocked);
        }
    }
    let notes: Vec<String> = state.promissory_notes.keys().cloned().collect();
    for note in notes {
        if rng.below(3) == 0 {
            let to = ids[rng.below(4)].clone();
            state.promissory_notes.insert(note.clone(), to);
        }
        if rng.below(8) == 0 && !crate::promissory::alias_of(&note).is_empty() {
            state.promissory_faceup.insert(note);
        }
    }
    for owner in &ids {
        if rng.below(4) == 0 {
            let holder = ids[rng.below(4)].clone();
            if holder != *owner {
                state.support_holders.insert(owner.clone(), holder);
            }
        }
    }
    if rng.below(3) == 0 {
        state.transient_flags.set(TransientFlags::BLACK_MARKET);
    }
    if rng.below(5) == 0 {
        state.phase = ti4_model::state::Phase::Agenda;
    }
    (state, hub)
}

#[test]
fn random_tables_list_and_price_the_same_deals_on_both_sides() {
    let mut rng = Xor(0x9E37_79B9_7F4A_7C15);
    let (mut compared, mut bailed) = (0, 0);
    let mut settled = 0;
    for _ in 0..40 {
        let (state, hub) = random_table(&mut rng);
        for (x, y) in [("a", "b"), ("b", "a"), ("c", "d"), ("d", "c"), ("a", "c"), ("b", "d")] {
            let (from, to) = (PlayerId::new(x), PlayerId::new(y));
            let pick = format!("#{}", rng.below(1000));
            let then = ["accept", "counter", "refuse"][rng.below(3)];
            let again = format!("#{}", rng.below(1000));
            let done = negotiate_pair(state.clone(), &hub, &from, &to, &[&pick, then, &again, "accept"]);
            compared += done.compared;
            bailed += done.bailed;
            settled += done.traded.iter().filter(|t| **t == Traded::Resolved).count();
        }
    }
    eprintln!("census: compared {compared} bailed {bailed} settled {settled}");
    assert!(compared > 150, "the core followed {compared} steps, left {bailed}");
    assert!(settled > 10, "{settled} deals settled");
}

/// Random terms over what the table holds (and a little that it does not, so some deals are illegal).
fn random_terms(rng: &mut Xor, state: &GameState, who: &str) -> crate::transactions::Terms {
    let seat = state.player(&PlayerId::new(who)).unwrap();
    let mut terms = crate::transactions::Terms::default();
    if rng.below(2) == 0 {
        terms.trade_goods = rng.below(seat.trade_goods.max(0) as usize + 2) as i32;
    }
    if rng.below(2) == 0 {
        terms.commodities = rng.below(seat.commodities.max(0) as usize + 2) as i32;
    }
    if rng.below(4) == 0 {
        let held: Vec<&String> = seat.relic_fragments.keys().collect();
        if !held.is_empty() {
            terms.fragments = vec![held[rng.below(held.len())].clone()];
        }
    }
    if rng.below(3) == 0 {
        let notes: Vec<String> = crate::promissory::available_notes(state, ContentStore::embedded(), &PlayerId::new(who));
        if rng.below(5) == 0 {
            terms.promissory = Some(crate::promissory::support(seat.faction.as_str()));
        } else if !notes.is_empty() {
            terms.promissory = Some(notes[rng.below(notes.len())].clone());
        }
    }
    if rng.below(5) == 0 {
        terms.action_card = seat.action_cards.first().cloned();
    }
    if rng.below(6) == 0 {
        terms.secret = seat.secret_objectives.first().cloned();
    }
    terms
}

/// The free-form deals of random tables, struck, refused or countered: both sides agree on the question, the legality, the state.
/// (Nobody sends one but a human at the trade desk, so the golden games never reach them.)
#[test]
fn random_free_form_deals_agree_on_both_sides() {
    let mut rng = Xor(0xD1B5_4A32_D192_ED03);
    let (mut compared, mut bailed, mut settled, mut rejected) = (0, 0, 0, 0);
    for _ in 0..40 {
        let (state, hub) = random_table(&mut rng);
        for (x, y) in [("a", "b"), ("b", "a"), ("c", "d"), ("d", "c"), ("a", "c"), ("b", "d")] {
            let (from, to) = (PlayerId::new(x), PlayerId::new(y));
            let given = random_terms(&mut rng, &state, x);
            let received = random_terms(&mut rng, &state, y);
            if given.is_empty() && received.is_empty() {
                continue;
            }
            let id = crate::transactions::free_form_id(&given, &received);
            let then = ["accept", "counter", "refuse"][rng.below(3)];
            let back = crate::transactions::free_form_id(&random_terms(&mut rng, &state, y), &random_terms(&mut rng, &state, x));
            let script: Vec<&str> = if back == "x;" { vec![&id, then] } else { vec![&id, then, &back, "accept"] };
            let done = negotiate_pair(state.clone(), &hub, &from, &to, &script);
            compared += done.compared;
            bailed += done.bailed;
            settled += done.traded.iter().filter(|t| **t == Traded::Resolved).count();
            rejected += done.traded.iter().filter(|t| matches!(t, Traded::Rejected(_))).count();
        }
    }
    eprintln!("free-form census: compared {compared} bailed {bailed} settled {settled} rejected {rejected}");
    assert!(compared > 150, "the core followed {compared} steps, left {bailed}");
    assert!(settled > 5 && rejected > 5, "{settled} settled, {rejected} rejected");
}

/// [`negotiate`] for any two seats.
fn negotiate_pair(state: GameState, hub: &crate::fixtures::Hub, from: &PlayerId, to: &PlayerId, script: &[&str]) -> Done {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    let content = ContentStore::embedded();
    let mut state = state;
    let mut traded = Vec::new();
    with_mask(AreaMask::STRING.with(Area::TradeDiplomacy, Backend::Dual), || {
        let mut window = TradeWindow::open_with_content(&mut state, content, from, to);
        for want in script {
            let Some(choice) = window.pending_choice(&state, content) else { break };
            let deals: Vec<&str> = choice.options.iter().filter(|o| !o.is_decline()).map(|o| o.id.as_str()).collect();
            let id = match want.strip_prefix('#') {
                Some(n) => deals[n.parse::<usize>().unwrap() % deals.len()].to_owned(),
                None => (*want).to_owned(),
            };
            let id = if choice.option(&id).is_some() || id.starts_with(crate::transactions::FREE_FORM_PREFIX) { id } else { deals[0].to_owned() };
            let mut table = crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new([id])));
            let answer = |c: &crate::choice::Choice, s: &mut GameState| table.ask_seeing(c, &crate::choice::Observed::new(s, content, POK, Some(&hub.galaxy)));
            traded.push(window.answer_with(&mut state, content, &hub.galaxy, &choice, answer).expect("answered"));
            if window.is_complete() {
                break;
            }
        }
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let (compared, bailed) = crate::dual::thread_window_counts(LABEL);
    Done { traded, state, compared, bailed, reasons: Vec::new() }
}

/// Put a Pride of Kenara on the first planet `who` controls; the planets it controls, in the string's order.
fn with_kenara(state: &mut GameState, who: &PlayerId) -> Vec<ti4_model::id::PlanetId> {
    // Two planets, or the units have nowhere to go.
    if state.controlled_planets(who).len() < 2 {
        let (system, planet) = crate::fixtures::a_placed_planet();
        state.system_mut(&system).set_control(planet, who.clone());
    }
    let (system, planet) = {
        let owned = state.controlled_planets(who);
        let (system, planet) = owned.first().map(|(s, p)| ((*s).clone(), (*p).clone())).expect("a seat holds a planet");
        (system, planet)
    };
    state.system_mut(&system).planet_units.entry(planet).or_default().push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("hacan_mech"), who.clone()));
    assert!(!crate::transactions::pride_of_kenara_planets(state, who).is_empty(), "the position gives {who} a Pride of Kenara");
    state.controlled_planets(who).into_iter().map(|(_, p)| p.clone()).collect()
}

#[test]
fn a_planet_card_is_given_sold_and_bought_with_its_units_moving_in_lockstep() {
    // The proposer's own card: a gift (price 0) and a sale, the units move to the planet it names.
    for price in ["0", "~"] {
        let (mut state, hub) = sol_hacan();
        let planets = with_kenara(&mut state, &a());
        let planet = planets[0].clone();
        let before = state.system_state(&crate::planets::system_of(&state, ContentStore::embedded(), ti4_model::content_types::DEFAULT, planet.as_str()).unwrap()).on_planet_of(&planet, &a()).len();
        let pick = if price == "0" { format!("kp{}>{}:0", planet, planets[1]) } else { format!("~kp{planet}>") };
        let done = plain(state, &hub, &[&pick, "accept"]);
        assert_eq!(done.traded, [Traded::Offered, Traded::Resolved], "{price}");
        assert_eq!((done.compared, done.bailed), (2, 0), "{price}: {:?}", done.reasons);
        assert_eq!(done.state.controlled_planets(&b()).iter().filter(|(_, p)| **p == planet).count(), 1, "b holds the card now");
        assert!(done.state.controlled_planets(&a()).iter().all(|(_, p)| **p != planet));
        let _ = before;
    }
    // The partner's card, paid for in trade goods: their units move to their planet.
    let (mut state, hub) = sol_hacan();
    let planets = with_kenara(&mut state, &b());
    let done = plain(state, &hub, &[&format!("~kq{}>", planets[0]), "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (2, 0), "{:?}", done.reasons);
    assert!(done.state.controlled_planets(&a()).iter().any(|(_, p)| **p == planets[0]));
    assert!(done.state.controlled_planets(&b()).iter().all(|(_, p)| **p != planets[0]));
}

#[test]
fn a_planet_card_is_priced_at_the_partners_goods_and_follows_the_rules_version_and_the_owner_of_the_mech() {
    use ti4_engine_core::generated::Planet;
    let value = |planet: &ti4_model::id::PlanetId| crate::render::trade::planet_values()[crate::core_adapter::typed::<Planet>(planet.as_str()).unwrap().index()];
    // The partner holds exactly the price: the sale is offered (`>=`), and its label is the sale's.
    let (mut state, hub) = sol_hacan();
    let planets = with_kenara(&mut state, &a());
    let price = value(&planets[0]);
    state.player_mut(&b()).unwrap().trade_goods = i32::from(price);
    let pick = format!("kp{}>{}:{price}", planets[0], planets[1]);
    let done = plain(state, &hub, &[&pick, "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (2, 0), "{:?}", done.reasons);
    assert_eq!(done.state.player(&b()).unwrap().trade_goods, 0, "the partner paid the price");
    // Version 17 is the first with the trade; 16 offers none.
    for (version, offered) in [(16, false), (17, true)] {
        let (mut state, hub) = sol_hacan();
        let planets = with_kenara(&mut state, &a());
        state.rules_version = version;
        let gift = format!("kp{}>{}:0", planets[0], planets[1]);
        let content = ContentStore::embedded();
        let ids: Vec<String> = TradeWindow::open_with_content(&mut state.clone(), content, &a(), &b()).pending_choice(&state, content).unwrap().ids().into_iter().map(str::to_owned).collect();
        assert_eq!(ids.contains(&gift), offered, "rules version {version}");
        let done = plain(state, &hub, &["cc1"]);
        assert_eq!((done.compared, done.bailed), (1, 0), "{:?}", done.reasons);
    }
    // A mech of the other seat on my planet is not mine: the partner's Pride of Kenara brings the board into the
    // window, and the core must not offer me a card for it.
    let (mut state, hub) = sol_hacan();
    let theirs = with_kenara(&mut state, &b());
    let mine = with_kenara(&mut state, &a());
    let (system, planet) = {
        let owned = state.controlled_planets(&a());
        let (s, p) = owned.iter().find(|(_, p)| **p != mine[0]).map(|(s, p)| ((*s).clone(), (*p).clone())).expect("a second planet");
        (s.clone(), p.clone())
    };
    crate::fixtures::put_on_planet(&mut state, &system, &planet, "hacan_mech", &b(), 1);
    let _ = theirs;
    let done = plain(state, &hub, &["cc1"]);
    assert_eq!((done.compared, done.bailed), (1, 0), "{:?}", done.reasons);
}

#[test]
fn a_planet_card_whose_mech_left_is_rejected_at_the_accept() {
    let (mut state, hub) = sol_hacan();
    let planets = with_kenara(&mut state, &a());
    let pick = format!("kp{}>{}:0", planets[0], planets[1]);
    let done = negotiate(state, &hub, &[&pick, "accept"], |n, s| {
        if n == 0 {
            // The mech is gone: the card can no longer be given.
            for sys in s.board.values_mut() {
                for units in sys.planet_units.values_mut() {
                    units.retain(|u| u.type_id.as_str() != "hacan_mech");
                }
            }
        }
    });
    assert!(matches!(done.traded[1], Traded::Rejected(_)), "{:?}", done.traded);
    assert_eq!((done.compared, done.bailed), (2, 0));
}

#[test]
fn a_whole_negotiation_runs_on_the_core_alone() {
    use ti4_engine_core::choice::trade::{Opt, Shape};
    use ti4_engine_core::choice::{Opt as COpt, Pick};
    use ti4_engine_core::driver::{Driver, Status};
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::rules::trade_window::{Outcome, TradeWindow as Core};

    let (mut state, hub) = sol_hacan();
    let content = ContentStore::embedded();
    let (mut cx, r) = crate::dual_window::tables(&state).unwrap();
    let (sa, sb) = (r.seat("a").unwrap(), r.seat("b").unwrap());
    let facts = crate::trade_window_dual::facts_of(&state, content, &hub.galaxy, &r, &a(), &b()).unwrap();
    let kernel = Kernel::plain();
    let mut driver = Driver::new(Core::open(sa, sb, facts), Resolver::new(&[sa, sb], Some(sa)), AbilityTable::default(), 1);
    let swap = |c: &ti4_engine_core::choice::Choice| {
        let o = COpt::Trade(Opt::Deal(Shape::Swap { n: 3 }));
        assert!(c.opts.iter().any(|x| *x == o), "the swap is on offer");
        Pick::Opt(o)
    };
    let outcome = crate::core_sync::with_core(&mut state, &mut cx, crate::core_sync::Sources::default(), crate::trade_window_dual::WINDOW, |core, _| {
        let mut seen = Vec::new();
        let mut status = driver.run(core, &kernel).unwrap();
        loop {
            let Status::Asking(choice) = status else { break };
            let ti4_engine_core::choice::Ask::Trade(ask) = choice.ask else { panic!("a trade question") };
            let pick = match ask {
                ti4_engine_core::choice::trade::Ask::Propose { .. } => swap(&choice),
                ti4_engine_core::choice::trade::Ask::Answer { .. } if seen.len() == 1 => Pick::Opt(COpt::Trade(Opt::Counter)),
                ti4_engine_core::choice::trade::Ask::Answer { .. } => Pick::Opt(COpt::Trade(Opt::Accept)),
            };
            seen.push(choice.actor);
            status = driver.resume(core, &kernel, pick).unwrap();
        }
        (seen, driver.window.outcome)
    })
    .unwrap();
    assert_eq!(outcome.0, [sa, sb, sb, sa], "propose, counter, the counter's proposal, accept");
    assert_eq!(outcome.1, Some(Outcome::Resolved));
    assert_eq!(goods(&state, "a"), 2 + 3);
    assert_eq!(goods(&state, "b"), 2 + 3);

    // Haggling is bounded: a second counter ends the negotiation with nothing changed.
    let (mut state, _) = sol_hacan();
    let (mut cx, r) = crate::dual_window::tables(&state).unwrap();
    let mut driver = Driver::new(Core::open(sa, sb, facts), Resolver::new(&[sa, sb], Some(sa)), AbilityTable::default(), 1);
    let outcome = crate::core_sync::with_core(&mut state, &mut cx, crate::core_sync::Sources::default(), crate::trade_window_dual::WINDOW, |core, _| {
        let mut asked = 0;
        let mut status = driver.run(core, &kernel).unwrap();
        while let Status::Asking(choice) = status {
            assert!(asked < 8, "haggling does not end");
            let pick = if matches!(choice.ask, ti4_engine_core::choice::Ask::Trade(ti4_engine_core::choice::trade::Ask::Answer { .. })) {
                Pick::Opt(COpt::Trade(Opt::Counter))
            } else {
                swap(&choice)
            };
            asked += 1;
            status = driver.resume(core, &kernel, pick).unwrap();
        }
        (asked, driver.window.outcome)
    })
    .unwrap();
    let _ = r;
    assert_eq!(outcome, (4, Some(Outcome::NothingOffered)), "propose, counter, propose, counter: two proposals are spent");
    assert_eq!(goods(&state, "a"), 2);
}

#[test]
fn a_note_that_left_the_hand_before_the_accept_is_rejected() {
    // The deal on the table sells a note; before the partner answers, the note is lent to a third seat.
    let (state, hub) = table(&[("a", "sol"), ("b", "hacan"), ("c", "letnev")]);
    let done = negotiate(state, &hub, &["~pnps:sol", "accept"], |n, s| {
        if n == 0 {
            s.promissory_notes.insert("ps:sol".to_owned(), PlayerId::new("c"));
        }
    });
    assert_eq!(done.traded, [Traded::Offered, Traded::Rejected(OfferError::CannotPay(a()))]);
    assert_eq!((done.compared, done.bailed), (2, 0));
    assert_eq!(done.state.promissory_notes.get("ps:sol"), Some(&PlayerId::new("c")), "nothing moved");
}

#[test]
fn an_action_card_sale_whose_door_closed_before_the_accept_is_rejected() {
    // Black Market Dealings opens the door for one negotiation; when it is over the card cannot change hands.
    let (mut state, hub) = table(&[("a", "sol"), ("b", "letnev")]);
    state.transient_flags.set(TransientFlags::BLACK_MARKET);
    state.player_mut(&a()).unwrap().action_cards = vec![ActionCardId::new(crate::generated::IdStr::as_str(ti4_engine_core::generated::ActionCard::ALL[0]))];
    let done = negotiate(state, &hub, &["~ac", "accept"], |n, s| {
        if n == 0 {
            s.transient_flags.clear(TransientFlags::BLACK_MARKET);
        }
    });
    assert_eq!(done.traded, [Traded::Offered, Traded::Rejected(OfferError::ActionCardsNotTradeable)]);
    assert_eq!((done.compared, done.bailed), (2, 0));
}

#[test]
fn dark_pact_gains_follow_an_accepted_gift_of_a_full_commodity_value() {
    // Empyrean's Dark Pact (faceup with the giver): a gift of exactly the receiver's... the giver's commodity
    // value pays each side a trade good. The core settles the gift; the string's tail pays the gains.
    let (mut state, hub) = table(&[("a", "sol"), ("b", "empyrean")]);
    state.promissory_notes.insert("dark_pact:empyrean".to_owned(), a());
    state.promissory_faceup.insert("dark_pact:empyrean".to_owned());
    let value = ti4_content::factions::get(ContentStore::embedded(), "sol").unwrap().commodities();
    state.player_mut(&a()).unwrap().commodities = value;
    let before = (goods(&state, "a"), goods(&state, "b"));
    let done = plain(state, &hub, &[&format!("c{value}:0"), "accept"]);
    assert_eq!(done.traded, [Traded::Offered, Traded::Resolved]);
    assert_eq!((done.compared, done.bailed), (2, 0));
    assert_eq!(goods(&done.state, "a"), before.0 + 1, "the giver gains one");
    assert_eq!(goods(&done.state, "b"), before.1 + value + 1, "the receiver gains the gift and one");
}
}
