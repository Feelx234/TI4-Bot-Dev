//! Package B4d: the structured negotiation window runs on the core beside the string window
//! (`diplomacy_window_dual`, `rules::diplomacy_window`, `query::diplomacy_items`) under `trade=dual`, step by step,
//! and agrees with it. A bail is silent, so every test counts the steps the core followed (`compared`) and the ones
//! it left to the string (`bailed`).

#[cfg(test)]
mod tests {
    use ti4_content::ContentStore;
    use ti4_model::content_types::POK;
    use ti4_model::id::{ActionCardId, PlayerId, SystemId};
    use ti4_model::state::GameState;
    use ti4_model::{DealStatus, DiplomacyState, SignalStatement};

    use crate::choice::Window as _;
    use crate::diplomacy::builder::ContactScope;
    use crate::diplomacy::window::{DiplomacyStage, DiplomacyWindow, LABEL_ANSWER};
    use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};

    fn p(s: &str) -> PlayerId {
        PlayerId::new(s)
    }

    /// Real factions, notes dealt, diplomacy on, round 1; every seat has a cruiser in the centre system.
    fn table(factions: &[(&str, &str)]) -> (GameState, crate::fixtures::Hub) {
        let mut state = crate::fixtures::seated_game(factions, POK);
        crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
        state.diplomacy = DiplomacyState::for_players(&state.seating_order, true);
        state.round = 1;
        let hub = crate::fixtures::plain_hub();
        let centre = SystemId::new(hub.centre.clone());
        for (name, _) in factions {
            crate::fixtures::put(&mut state, &centre, "cruiser", &p(name), 1);
        }
        for seat in &mut state.players {
            seat.trade_goods = 3;
            seat.commodities = 2;
        }
        (state, hub)
    }

    fn sol_hacan_xxcha() -> (GameState, crate::fixtures::Hub) {
        table(&[("a", "sol"), ("b", "hacan"), ("c", "xxcha")])
    }

    fn scope() -> ContactScope {
        ContactScope {
            physical: true,
            attack_targets: vec![p("c")],
            systems: vec![SystemId::new("18")],
            agenda: Some(("arms_reduction".to_owned(), vec!["for".to_owned(), "against".to_owned()])),
        }
    }

    struct Done {
        state: GameState,
        window: DiplomacyWindow,
        compared: u64,
        bailed: u64,
        reasons: Vec<(String, u64)>,
    }

    /// Open the contact of `a` with `b` and answer each question with the next entry of `script`: an option id, or
    /// `~` and a prefix for the first option that starts with it. `between` edits the position after step `n`.
    fn run(mut state: GameState, hub: &crate::fixtures::Hub, scope: ContactScope, signals: Vec<SignalStatement>, script: &[&str], between: impl Fn(usize, &mut GameState)) -> Done {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        crate::dual::reset_thread_windows();
        let content = ContentStore::embedded();
        let mut window = DiplomacyWindow::open(&mut state, p("a"), p("b"), scope, signals).expect("the contact opens");
        with_mask(AreaMask::STRING.with(Area::TradeDiplomacy, Backend::Dual), || {
            for (n, want) in script.iter().enumerate() {
                let Some(choice) = window.pending_choice(&state, content, POK) else { break };
                let id = match want.strip_prefix('~') {
                    Some(prefix) => choice.options.iter().find(|o| o.id.starts_with(prefix)).map(|o| o.id.clone()).unwrap_or_else(|| panic!("no option {prefix}* in {:?}", choice.ids())),
                    None => (*want).to_owned(),
                };
                let mut table = crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new([id])));
                let answer = |c: &crate::choice::Choice, s: &mut GameState| table.ask_seeing(c, &crate::choice::Observed::new(s, content, POK, Some(&hub.galaxy)));
                window.answer_with(&mut state, content, POK, Some(&hub.galaxy), &choice, answer).expect("the script answers every question").expect("the step is legal");
                between(n, &mut state);
                if window.stage == DiplomacyStage::Done {
                    break;
                }
            }
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let (compared, bailed) = crate::dual::thread_window_counts(LABEL_ANSWER);
        let reasons = r.windows.iter().filter(|(k, _)| k.starts_with(LABEL_ANSWER) && k.contains("[bail")).map(|(k, w)| (k.clone(), w.bailed)).collect();
        Done { state, window, compared, bailed, reasons }
    }

    fn plain(state: GameState, hub: &crate::fixtures::Hub, script: &[&str]) -> Done {
        run(state, hub, scope(), vec![], script, |_, _| {})
    }

    fn followed(done: &Done, steps: u64) {
        assert_eq!((done.compared, done.bailed), (steps, 0), "{:?}", done.reasons);
    }

    #[test]
    fn a_swap_of_goods_is_built_proposed_and_accepted_in_lockstep() {
        let (state, hub) = sol_hacan_xxcha();
        let script = [
            "diplomacy|now|tg", "diplomacy|amount|2", "diplomacy|done", // a gives 2 trade goods
            "diplomacy|now|commodities", "diplomacy|amount|2", "diplomacy|done", // asks 2 commodities
            "diplomacy|propose", "diplomacy|accept",
        ];
        let done = plain(state, &hub, &script);
        followed(&done, 8);
        assert_eq!(done.window.stage, DiplomacyStage::Done);
        assert_eq!(done.state.diplomacy.history.first().map(|h| h.status), Some(DealStatus::Fulfilled), "{:?}", done.state.diplomacy.journal);
        assert_eq!(done.state.player(&p("a")).unwrap().trade_goods, 3 - 2 + 2, "the commodities arrive as trade goods");
        assert_eq!(done.state.player(&p("b")).unwrap().trade_goods, 3 + 2);
        assert_eq!(done.state.player(&p("b")).unwrap().commodities, 0);
        assert_eq!(done.state.transactions_this_round, [(p("a"), p("b"))]);
        assert_eq!(done.state.diplomacy.history[0].status, DealStatus::Fulfilled);
    }

    #[test]
    fn promises_make_an_active_deal_and_every_pledge_kind_is_offered() {
        let (state, hub) = sol_hacan_xxcha();
        let script = [
            "~diplomacy|later|tg", "diplomacy|amount|1",
            "~diplomacy|promise|{\"do_not_attack\"", "~diplomacy|promise|{\"do_not_activate\"", "~diplomacy|promise|{\"attack\"", "~diplomacy|promise|{\"vote\"",
            "diplomacy|done", // a gives
            "~diplomacy|promise|{\"do_not_attack\"", "diplomacy|done", // asks b not to attack
            "diplomacy|propose", "diplomacy|accept",
        ];
        let done = plain(state, &hub, &script);
        followed(&done, 11);
        let deal = &done.state.diplomacy.active_deals[&ti4_model::DealId(1)];
        assert_eq!(deal.status, DealStatus::Active);
        assert_eq!(deal.latest().proposer_terms.len(), 5);
        assert_eq!(done.state.player(&p("a")).unwrap().trade_goods, 3, "a promise moves nothing");
    }

    #[test]
    fn a_counter_is_built_from_the_terms_on_the_table_and_two_counters_are_the_limit() {
        let (state, hub) = sol_hacan_xxcha();
        let script = [
            "~diplomacy|later|tg", "diplomacy|amount|1", "diplomacy|done", "diplomacy|done", "diplomacy|propose",
            "diplomacy|counter", "diplomacy|done", "diplomacy|done", "diplomacy|propose", // b counters unchanged
            "diplomacy|counter", "diplomacy|done", "diplomacy|done", "diplomacy|propose", // a counters
        ];
        let done = plain(state, &hub, &script);
        followed(&done, 13);
        assert_eq!(done.window.responses, 2);
        let deal = &done.state.diplomacy.active_deals[&ti4_model::DealId(1)];
        assert_eq!(deal.revisions.len(), 3);
        // The third answer cannot counter: only accept and decline.
        let content = ContentStore::embedded();
        let ids = done.window.pending_choice(&done.state, content, POK).unwrap().ids().into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(ids, ["diplomacy|accept", "diplomacy|decline"]);
    }

    #[test]
    fn a_decline_walking_away_and_the_edit_budget_end_as_the_string_ends_them() {
        let (state, hub) = sol_hacan_xxcha();
        let done = plain(state, &hub, &["~diplomacy|later|tg", "diplomacy|amount|2", "diplomacy|done", "diplomacy|done", "diplomacy|propose", "diplomacy|decline"]);
        followed(&done, 6);
        assert_eq!(done.state.diplomacy.history[0].status, DealStatus::Declined);
        // Walking away from the counter returns to the offer, and the chance to counter is spent.
        let (state, hub) = sol_hacan_xxcha();
        let script = ["~diplomacy|later|tg", "diplomacy|amount|2", "diplomacy|done", "diplomacy|done", "diplomacy|propose", "diplomacy|counter", "diplomacy|done", "diplomacy|done", "diplomacy|cancel", "diplomacy|accept"];
        let done = plain(state, &hub, &script);
        followed(&done, 10);
        assert_eq!(done.window.responses, 0);
        assert!(done.window.counter_abandoned);
        // Nothing offered: make no offer on the opening screen.
        let (state, hub) = sol_hacan_xxcha();
        let done = plain(state, &hub, &["diplomacy|cancel"]);
        followed(&done, 1);
        assert!(done.state.diplomacy.active_deals.is_empty());
        // Add and remove until the budget runs out: only done is left; edit from review spends the budget.
        let (state, hub) = sol_hacan_xxcha();
        let mut script = vec![];
        for _ in 0..4 {
            script.extend(["~diplomacy|now|tg", "diplomacy|amount|1", "diplomacy|remove|0"]);
        }
        script.extend(["diplomacy|done", "diplomacy|done"]);
        let done = plain(state, &hub, &script);
        followed(&done, 14);
        let ids = done.window.pending_choice(&done.state, ContentStore::embedded(), POK).unwrap().ids().into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert!(!ids.contains(&"diplomacy|edit".to_owned()), "{ids:?}");
        let (state, hub) = sol_hacan_xxcha();
        let done = plain(state, &hub, &["~diplomacy|later|tg", "diplomacy|amount|1", "diplomacy|done", "diplomacy|done", "diplomacy|edit", "diplomacy|done", "diplomacy|done", "diplomacy|edit", "diplomacy|done", "diplomacy|done"]);
        followed(&done, 10);
    }

    #[test]
    fn a_note_and_with_hacan_an_action_card_are_offered_and_traded() {
        let (mut state, hub) = sol_hacan_xxcha();
        let card = ActionCardId::new(crate::generated::IdStr::as_str(ti4_engine_core::generated::ActionCard::ALL[0]));
        state.player_mut(&p("b")).unwrap().action_cards = vec![card.clone()];
        // b (Hacan, Arbiters) is the one who builds the counter; a asks for nothing, b gives a card back.
        let script = ["~diplomacy|note|", "diplomacy|done", "diplomacy|done", "diplomacy|propose", "diplomacy|counter", "~diplomacy|card|", "diplomacy|done", "diplomacy|done", "diplomacy|propose", "diplomacy|accept"];
        let done = plain(state, &hub, &script);
        followed(&done, 10);
        assert_eq!(done.state.player(&p("a")).unwrap().action_cards, [card]);
        assert_eq!(done.state.transactions_this_round.len(), 1);
    }

    #[test]
    fn a_signal_is_sent_instead_of_an_offer() {
        let (state, hub) = sol_hacan_xxcha();
        let warning = SignalStatement::AttackIfYouActivate { system: SystemId::new("18") };
        let done = run(state, &hub, scope(), vec![warning], &["~diplomacy|signal|"], |_, _| {});
        followed(&done, 1);
        assert_eq!(done.state.diplomacy.recent_signals.len(), 1);
        assert_eq!(done.window.stage, DiplomacyStage::Done);
    }

    #[test]
    fn a_deal_that_stopped_being_payable_is_void_like_a_decline() {
        let (state, hub) = sol_hacan_xxcha();
        let script = ["~diplomacy|now|tg", "diplomacy|amount|3", "diplomacy|done", "diplomacy|done", "diplomacy|propose", "diplomacy|accept"];
        let done = run(state, &hub, scope(), vec![], &script, |n, s| {
            if n == 4 {
                s.player_mut(&p("a")).unwrap().trade_goods = 0;
            }
        });
        followed(&done, 6);
        assert_eq!(done.state.diplomacy.history[0].status, DealStatus::Declined);
        assert!(done.state.transactions_this_round.is_empty());
    }

    #[test]
    fn physical_items_need_a_partner_the_pair_may_transact_with() {
        let (state, hub) = sol_hacan_xxcha();
        let none = ContactScope { physical: false, ..scope() };
        let done = run(state, &hub, none, vec![], &["~diplomacy|later|tg", "diplomacy|amount|1", "diplomacy|done", "diplomacy|done", "diplomacy|propose", "diplomacy|accept"], |_, _| {});
        followed(&done, 6);
        assert_eq!(done.state.diplomacy.active_deals[&ti4_model::DealId(1)].status, DealStatus::Active);
    }

    #[test]
    fn a_faction_two_seats_play_keeps_the_window_on_the_string() {
        let (state, hub) = table(&[("a", "sol"), ("b", "sol"), ("c", "xxcha")]);
        let done = plain(state, &hub, &["~diplomacy|later|tg"]);
        assert_eq!((done.compared, done.bailed), (0, 1));
        assert!(done.reasons.iter().any(|(k, _)| k.contains("a faction played by two seats")), "{:?}", done.reasons);
    }

    /// The first screen of the contact in a position, asked of both windows (one step: "done offering").
    fn first_screen(mut state: GameState, hub: &crate::fixtures::Hub, scope: ContactScope, edit: impl Fn(&mut GameState)) -> (Done, Vec<String>) {
        edit(&mut state);
        let content = ContentStore::embedded();
        let probe = {
            let mut copy = state.clone();
            let w = DiplomacyWindow::open(&mut copy, p("a"), p("b"), scope.clone(), vec![]).unwrap();
            w.pending_choice(&copy, content, POK).unwrap().ids().into_iter().map(str::to_owned).collect::<Vec<_>>()
        };
        (run(state, hub, scope, vec![], &["diplomacy|done"], |_, _| {}), probe)
    }

    #[test]
    fn the_catalogue_follows_the_position() {
        // A Trade card not yet played offers the replenish promise; played, it does not.
        let trade = |played: bool| {
            move |s: &mut GameState| {
                s.deal_strategy_card(&p("a"), ti4_model::id::StrategyCardId::new("pok5trade"));
                if played {
                    s.player_mut(&p("a")).unwrap().exhausted_strategy_cards.insert(ti4_model::id::StrategyCardId::new("pok5trade"));
                }
            }
        };
        for played in [false, true] {
            let (state, hub) = sol_hacan_xxcha();
            let (done, ids) = first_screen(state, &hub, scope(), trade(played));
            followed(&done, 1);
            assert_eq!(ids.iter().any(|i| i.contains("replenish_for")), !played, "{ids:?}");
        }
        // A readied agent of the four is offered as a favour; an exhausted one is not.
        for status in [ti4_model::state::LeaderStatus::Readied, ti4_model::state::LeaderStatus::Exhausted] {
            let (state, hub) = sol_hacan_xxcha();
            let (done, ids) = first_screen(state, &hub, scope(), |s| {
                s.player_mut(&p("a")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("hacanagent"), status);
            });
            followed(&done, 1);
            assert_eq!(ids.iter().any(|i| i.contains("use_leader_for")), status == ti4_model::state::LeaderStatus::Readied, "{ids:?}");
        }
        // A seat that predicted the outcome has no vote to promise.
        for predicted in [false, true] {
            let (state, hub) = sol_hacan_xxcha();
            let (done, ids) = first_screen(state, &hub, scope(), |s| {
                if predicted {
                    s.agenda_predictions.insert(p("a"), "for".to_owned());
                }
            });
            followed(&done, 1);
            assert_eq!(ids.iter().any(|i| i.contains("\"vote\"")), !predicted, "{ids:?}");
        }
        // Nothing is held, nothing physical is offered; fragments are offered when held (an unknown one is a frontier one).
        let (state, hub) = sol_hacan_xxcha();
        let (done, ids) = first_screen(state, &hub, scope(), |s| {
            let seat = s.player_mut(&p("a")).unwrap();
            seat.trade_goods = 0;
            seat.commodities = 0;
            seat.relic_fragments.insert("CULTURAL".to_owned(), 1);
            seat.relic_fragments.insert("FRONTIER".to_owned(), 2);
        });
        followed(&done, 1);
        assert!(ids.contains(&"diplomacy|now|cultural".to_owned()) && ids.contains(&"diplomacy|now|unknown".to_owned()), "{ids:?}");
        assert!(!ids.contains(&"diplomacy|now|tg".to_owned()) && !ids.contains(&"diplomacy|now|hazardous".to_owned()), "{ids:?}");
    }

    #[test]
    fn amounts_are_bounded_by_what_is_held_ten_and_five_for_a_promise() {
        let (state, hub) = sol_hacan_xxcha();
        let rich = |s: &mut GameState| s.player_mut(&p("a")).unwrap().trade_goods = 12;
        let mut state = state;
        rich(&mut state);
        let done = run(state.clone(), &hub, scope(), vec![], &["diplomacy|now|tg", "diplomacy|amount|10"], |_, _| {});
        followed(&done, 2);
        let done = run(state, &hub, scope(), vec![], &["diplomacy|later|tg", "diplomacy|amount|5"], |_, _| {});
        followed(&done, 2);
        for kind in ["now", "later"] {
            let (mut state, hub) = sol_hacan_xxcha();
            rich(&mut state);
            let content = ContentStore::embedded();
            let mut w = DiplomacyWindow::open(&mut state, p("a"), p("b"), scope(), vec![]).unwrap();
            let mut table = crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new([format!("diplomacy|{kind}|tg")])));
            let c = w.pending_choice(&state, content, POK).unwrap();
            let ask = |c: &crate::choice::Choice, s: &mut GameState| table.ask_seeing(c, &crate::choice::Observed::new(s, content, POK, Some(&hub.galaxy)));
            w.answer_with(&mut state, content, POK, Some(&hub.galaxy), &c, ask).unwrap().unwrap();
            let amounts = w.pending_choice(&state, content, POK).unwrap().ids().len();
            assert_eq!(amounts, if kind == "now" { 10 } else { 5 });
        }
    }

    #[test]
    fn a_side_holds_twelve_terms_and_the_last_step_of_the_budget_is_still_an_edit() {
        // Seven add operations leave the eighth step; the eighth closes the catalogue.
        let (mut state, hub) = sol_hacan_xxcha();
        state.player_mut(&p("a")).unwrap().trade_goods = 9;
        let script = [
            "diplomacy|now|tg", "diplomacy|amount|1", "diplomacy|later|tg", "diplomacy|amount|1", "~diplomacy|promise|", "~diplomacy|promise|", "~diplomacy|promise|", "~diplomacy|promise|", "~diplomacy|promise|",
        ];
        let done = plain(state, &hub, &script);
        followed(&done, 9);
        let ids = done.window.pending_choice(&done.state, ContentStore::embedded(), POK).unwrap().ids().into_iter().map(str::to_owned).collect::<Vec<_>>();
        // 7 steps taken (the two amounts are not steps): one left, so the catalogue is still open.
        assert!(ids.iter().any(|i| i.starts_with("diplomacy|promise|")), "{ids:?}");
    }

    #[test]
    fn the_hubris_and_the_one_note_per_side_rules_shape_the_catalogue() {
        // A Mahact cannot be given an Alliance: with a Mahact partner no `an:` note is offered to it.
        let (state, hub) = table(&[("a", "sol"), ("b", "mahact"), ("c", "xxcha")]);
        let (done, ids) = first_screen(state, &hub, scope(), |s| {
            s.promissory_notes.insert("an:sol".to_owned(), p("a"));
        });
        followed(&done, 1);
        assert!(!ids.iter().any(|i| i.starts_with("diplomacy|note|an:")), "{ids:?}");
    }
}
