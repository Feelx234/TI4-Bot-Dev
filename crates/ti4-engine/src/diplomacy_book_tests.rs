//! Package B4d: the deal book, the signals and the promise ledger run on the core beside the string code under
//! `trade=dual` (`diplomacy::book`, `diplomacy_write`, `rules::diplomacy_book`), and agree with it. A write the core
//! leaves to the string is silent, so every test reads `book::counts` (how often the core answered).

#[cfg(test)]
mod tests {
    use ti4_content::ContentStore;
    use ti4_model::content_types::POK;
    use ti4_model::id::{PlayerId, SystemId};
    use ti4_model::state::GameState;
    use ti4_model::{
        DealId, DealRevision, DealStatus, DealTerm, DiplomacyError, DiplomacyState, PromiseStatus, SignalStatement, SignalStatus,
        TransferAsset,
    };

    use crate::diplomacy::book::{counts, reset_counts};
    use crate::diplomacy::promises::{DiplomacyEventContext as Ev, PromiseError};
    use crate::diplomacy::signals::{SignalDraft, SignalError};
    use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};

    fn p(s: &str) -> PlayerId {
        PlayerId::new(s)
    }

    /// Three real factions seated, diplomacy on, round 1.
    fn table() -> GameState {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "xxcha")], POK);
        state.diplomacy = DiplomacyState::for_players(&state.seating_order, true);
        state.round = 1;
        state
    }

    /// Run `body` under `trade=dual` with the report and the counts cleared; no difference may be left.
    fn lockstep<T>(body: impl FnOnce() -> T) -> T {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        reset_counts();
        let out = with_mask(AreaMask::STRING.with(Area::TradeDiplomacy, Backend::Dual), body);
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        out
    }

    fn gift(n: u8) -> DealTerm {
        DealTerm::FuturePayment { asset: TransferAsset::TradeGoods(n), deadline_round: 2 }
    }
    fn now(n: u8) -> DealTerm {
        DealTerm::ImmediateTransfer(TransferAsset::TradeGoods(n))
    }
    fn rev(number: u8, author: &str, give: Vec<DealTerm>, take: Vec<DealTerm>) -> DealRevision {
        DealRevision::new(number, p(author), give, take, 1).unwrap()
    }
    fn ran(label: &str) -> u64 {
        let (ran, bailed) = counts(label);
        assert_eq!(bailed, 0, "{label}: the core left a write to the string");
        ran
    }
    fn trust(state: &GameState, observer: &str, subject: &str) -> i16 {
        state.diplomacy.relationship(&p(observer), &p(subject)).trust
    }

    #[test]
    fn an_offer_is_countered_twice_and_declined_in_lockstep() {
        let mut state = table();
        let id = lockstep(|| {
            let id = crate::diplomacy::book::create_deal(&mut state, &p("a"), &p("b"), &rev(0, "a", vec![gift(1)], vec![now(1)])).unwrap();
            crate::diplomacy::book::counter(&mut state, id, &rev(1, "b", vec![now(1)], vec![gift(2)])).unwrap();
            crate::diplomacy::book::counter(&mut state, id, &rev(2, "a", vec![gift(2)], vec![now(1)])).unwrap();
            // A third counter is refused, by both.
            assert_eq!(crate::diplomacy::book::counter(&mut state, id, &rev(3, "b", vec![now(1)], vec![])), Err(DiplomacyError::TooManyCounters));
            crate::diplomacy::book::decline(&mut state, id, &p("b")).unwrap();
            assert_eq!(ran("diplomacy::create_deal"), 1);
            assert_eq!(ran("diplomacy::counter"), 3);
            assert_eq!(ran("diplomacy::decline"), 1);
            id
        });
        assert!(state.diplomacy.active_deals.is_empty());
        let h = &state.diplomacy.history[0];
        assert_eq!((h.id, h.status, h.latest_revision.as_ref().unwrap().number), (id, DealStatus::Declined, 2));
    }

    #[test]
    fn the_book_refuses_what_the_string_refuses() {
        let mut state = table();
        lockstep(|| {
            let ok = rev(0, "a", vec![gift(1)], vec![]);
            assert_eq!(crate::diplomacy::book::create_deal(&mut state, &p("a"), &p("a"), &ok), Err(DiplomacyError::SelfPair));
            assert_eq!(crate::diplomacy::book::create_deal(&mut state, &p("a"), &p("b"), &rev(0, "b", vec![gift(1)], vec![])), Err(DiplomacyError::InvalidRevision));
            assert_eq!(crate::diplomacy::book::create_deal(&mut state, &p("a"), &p("b"), &rev(1, "a", vec![gift(1)], vec![])), Err(DiplomacyError::InvalidRevision));
            assert_eq!(crate::diplomacy::book::counter(&mut state, DealId(9), &ok), Err(DiplomacyError::UnknownDeal));
            assert_eq!(crate::diplomacy::book::decline(&mut state, DealId(9), &p("a")), Err(DiplomacyError::UnknownDeal));
            assert_eq!(crate::diplomacy::book::accept(&mut state, DealId(9), &p("a"), true), Err(DiplomacyError::UnknownDeal));
            assert_eq!(ran("diplomacy::create_deal"), 3);
            assert_eq!(ran("diplomacy::counter") + ran("diplomacy::decline") + ran("diplomacy::accept"), 3);
        });
        assert_eq!(state.diplomacy.next_deal_id, 1, "a refusal takes no id");
    }

    #[test]
    fn a_counter_after_the_deal_ended_is_refused() {
        let mut state = table();
        lockstep(|| {
            let id = crate::diplomacy::book::create_deal(&mut state, &p("a"), &p("b"), &rev(0, "a", vec![now(1)], vec![])).unwrap();
            crate::diplomacy::book::accept(&mut state, id, &p("b"), true).unwrap();
            // Immediate only: fulfilled and archived at once.
            assert_eq!(crate::diplomacy::book::counter(&mut state, id, &rev(1, "b", vec![now(1)], vec![])), Err(DiplomacyError::UnknownDeal));
        });
        assert_eq!(state.diplomacy.history[0].status, DealStatus::Fulfilled);
    }

    #[test]
    fn accepting_a_promise_is_a_deal_and_a_fair_swap_a_transaction() {
        // A promise: the deal stays active, DealAccepted (+5 trust each way).
        let mut state = table();
        lockstep(|| {
            let id = crate::diplomacy::book::create_deal(&mut state, &p("a"), &p("b"), &rev(0, "a", vec![gift(1)], vec![now(1)])).unwrap();
            crate::diplomacy::book::accept(&mut state, id, &p("b"), true).unwrap();
            assert_eq!(ran("diplomacy::accept"), 1);
        });
        assert_eq!(state.diplomacy.active_deals[&DealId(1)].status, DealStatus::Active);
        assert_eq!((trust(&state, "a", "b"), trust(&state, "b", "a")), (5, 5));
        // Immediate only, fair: FairTransaction (+3), archived. Unfair: nothing builds.
        for (fair, want) in [(true, 3), (false, 0)] {
            let mut state = table();
            lockstep(|| {
                let id = crate::diplomacy::book::create_deal(&mut state, &p("a"), &p("b"), &rev(0, "a", vec![now(1)], vec![now(1)])).unwrap();
                crate::diplomacy::book::accept(&mut state, id, &p("b"), fair).unwrap();
            });
            assert_eq!(trust(&state, "a", "b"), want, "fair {fair}");
            assert_eq!(state.diplomacy.history[0].status, DealStatus::Fulfilled);
            assert!(state.diplomacy.active_deals.is_empty());
        }
    }

    #[test]
    fn a_promise_to_attack_a_third_party_costs_its_victim_trust_in_both() {
        let mut state = table();
        lockstep(|| {
            let hit = DealTerm::Attack { player: p("c"), deadline_round: 2 };
            let id = crate::diplomacy::book::create_deal(&mut state, &p("a"), &p("b"), &rev(0, "a", vec![now(1)], vec![hit])).unwrap();
            crate::diplomacy::book::accept(&mut state, id, &p("b"), true).unwrap();
        });
        assert_eq!((trust(&state, "c", "a"), trust(&state, "c", "b")), (-3, -3));
    }

    fn warning(system: &str) -> SignalStatement {
        SignalStatement::AttackIfYouActivate { system: SystemId::new(system) }
    }
    fn draft(statement: SignalStatement, expires_round: u32) -> SignalDraft {
        SignalDraft { speaker: p("a"), target: p("b"), statement, expires_round }
    }

    #[test]
    fn a_signal_is_emitted_once_per_pair_and_turn_and_judged_by_what_follows() {
        let mut state = table();
        lockstep(|| {
            let first = crate::diplomacy::signals::emit_signal(&mut state, draft(warning("18"), 2)).unwrap();
            assert_eq!(
                crate::diplomacy::signals::emit_signal(&mut state, draft(warning("19"), 2)),
                Err(SignalError::AlreadyInitiated)
            );
            assert_eq!(crate::diplomacy::signals::emit_signal(&mut state, SignalDraft { speaker: p("a"), target: p("a"), ..draft(warning("18"), 2) }), Err(SignalError::SelfSignal));
            assert_eq!(
                crate::diplomacy::signals::emit_signal(&mut state, SignalDraft { speaker: p("b"), target: p("c"), ..draft(warning("18"), 3) }),
                Err(SignalError::InvalidExpiry)
            );
            assert_eq!(first.0, 1);
            // b activates 18: the warning is triggered; a then attacks b: carried out (+10 threat on b's row).
            crate::diplomacy::evaluate_event(&mut state, &Ev::SystemActivated { player: p("b"), system: SystemId::new("18") }).unwrap();
            assert_eq!(state.diplomacy.recent_signals[0].status, SignalStatus::Triggered);
            crate::diplomacy::evaluate_event(&mut state, &Ev::HostileEngagement { attacker: p("a"), victim: p("b"), activation_seq: 1 }).unwrap();
            assert_eq!(ran("diplomacy::emit_signal"), 4);
        });
        assert_eq!(state.diplomacy.recent_signals[0].status, SignalStatus::CarriedOut);
    }

    #[test]
    fn a_disabled_diplomacy_and_a_signal_in_an_open_contact_agree() {
        let mut off = table();
        off.diplomacy.enabled = false;
        lockstep(|| {
            assert_eq!(crate::diplomacy::signals::emit_signal(&mut off, draft(warning("18"), 1)), Err(SignalError::Disabled));
        });
        let mut state = table();
        lockstep(|| {
            // The contact window consumed the allowance already: no second consumption.
            assert!(crate::diplomacy::relations::consume_initiation(&mut state, &p("a"), &p("b")));
            crate::diplomacy::signals::emit_signal_in_open_contact(&mut state, draft(warning("18"), 1)).unwrap();
            assert_eq!(ran("diplomacy::emit_signal"), 1);
        });
        assert_eq!(state.diplomacy.recent_signals.len(), 1);
    }

    #[test]
    fn a_vote_assurance_is_honoured_or_broken_by_the_ballot() {
        for (cast, want) in [("for", SignalStatus::Honoured), ("against", SignalStatus::Broken)] {
            let mut state = table();
            lockstep(|| {
                let say = SignalStatement::WillVote { agenda: "arms_reduction".into(), outcome: "for".into() };
                crate::diplomacy::signals::emit_signal(&mut state, draft(say, 1)).unwrap();
                state.agenda_votes.insert(p("a"), cast.to_owned());
                crate::diplomacy::evaluate_event(&mut state, &Ev::VotesRecorded { agenda: "arms_reduction".into() }).unwrap();
                assert_eq!(ran("diplomacy::evaluate_event"), 1);
            });
            assert_eq!(state.diplomacy.recent_signals[0].status, want, "{cast}");
        }
    }

    #[test]
    fn the_signals_that_expire_with_the_round_are_settled() {
        let mut state = table();
        lockstep(|| {
            crate::diplomacy::signals::emit_signal(&mut state, draft(warning("18"), 1)).unwrap();
            crate::diplomacy::signals::emit_signal(&mut state, SignalDraft { speaker: p("b"), target: p("a"), ..draft(warning("18"), 1) }).unwrap();
            crate::diplomacy::evaluate_event(&mut state, &Ev::SystemActivated { player: p("a"), system: SystemId::new("18") }).unwrap();
            crate::diplomacy::settle_deadlines(&mut state, 1).unwrap();
            assert_eq!(ran("diplomacy::settle_deadlines"), 1);
        });
        // a's warning to b never triggered: heeded; b's was triggered by a and b never acted: a bluff.
        assert_eq!(state.diplomacy.recent_signals[0].status, SignalStatus::Heeded);
        assert_eq!(state.diplomacy.recent_signals[1].status, SignalStatus::Bluffed);
    }

    /// An active deal whose promises are `give` (a's) and `take` (b's).
    fn active(state: &mut GameState, give: Vec<DealTerm>, take: Vec<DealTerm>) -> DealId {
        let id = crate::diplomacy::book::create_deal(state, &p("a"), &p("b"), &rev(0, "a", give, take)).unwrap();
        crate::diplomacy::book::accept(state, id, &p("b"), true).unwrap();
        id
    }

    fn statuses(state: &GameState, id: DealId) -> Vec<PromiseStatus> {
        let d = state.diplomacy.active_deals.get(&id).map(|d| d.latest().clone()).or_else(|| state.diplomacy.history.iter().find(|h| h.id == id).and_then(|h| h.latest_revision.clone())).unwrap();
        d.proposer_statuses.into_iter().chain(d.recipient_statuses).collect()
    }

    #[test]
    fn every_kind_of_promise_is_judged_by_its_moment() {
        use DealTerm as T;
        let sys = SystemId::new("18");
        let cases: Vec<(DealTerm, Ev, PromiseStatus)> = vec![
            (T::DoNotActivate { system: sys.clone(), deadline_round: 2 }, Ev::SystemActivated { player: p("a"), system: sys.clone() }, PromiseStatus::Broken),
            (T::DoNotActivate { system: sys.clone(), deadline_round: 2 }, Ev::SystemActivated { player: p("b"), system: sys.clone() }, PromiseStatus::Pending),
            (T::DoNotAttack { player: p("b"), deadline_round: 2 }, Ev::HostileEngagement { attacker: p("a"), victim: p("b"), activation_seq: 1 }, PromiseStatus::Broken),
            (T::Attack { player: p("c"), deadline_round: 2 }, Ev::HostileEngagement { attacker: p("a"), victim: p("c"), activation_seq: 2 }, PromiseStatus::Fulfilled),
            (T::Attack { player: p("c"), deadline_round: 2 }, Ev::HostileEngagement { attacker: p("a"), victim: p("b"), activation_seq: 3 }, PromiseStatus::Pending),
            (T::Vote { agenda: "arms_reduction".into(), outcome: "for".into(), deadline_round: 2 }, Ev::VotesRecorded { agenda: "arms_reduction".into() }, PromiseStatus::Fulfilled),
            (T::ReplenishFor { beneficiary: p("b"), deadline_round: 2 }, Ev::CommoditiesReplenished { by: p("a"), beneficiary: p("b") }, PromiseStatus::Fulfilled),
            (T::ReplenishFor { beneficiary: p("b"), deadline_round: 2 }, Ev::CommoditiesReplenished { by: p("c"), beneficiary: p("b") }, PromiseStatus::Pending),
            (T::UseLeaderFor { leader: "arborecagent".into(), beneficiary: p("b"), deadline_round: 2 }, Ev::LeaderUsedFor { user: p("a"), leader: "arborecagent".into(), beneficiary: p("b") }, PromiseStatus::Fulfilled),
        ];
        for (term, event, want) in cases {
            let mut state = table();
            state.agenda_votes.insert(p("a"), "for".to_owned());
            lockstep(|| {
                let id = active(&mut state, vec![term.clone()], vec![gift(1)]);
                crate::diplomacy::evaluate_event(&mut state, &event).unwrap();
                assert_eq!(statuses(&state, id)[0], want, "{term:?} {event:?}");
                assert!(ran("diplomacy::evaluate_event") >= 1);
            });
        }
    }

    #[test]
    fn deadlines_keep_restraints_and_break_payments_and_the_game_end_expires_the_rest() {
        let mut state = table();
        lockstep(|| {
            let id = active(
                &mut state,
                vec![DealTerm::DoNotAttack { player: p("b"), deadline_round: 1 }, gift(1)],
                vec![DealTerm::FuturePayment { asset: TransferAsset::Commodities(1), deadline_round: 1 }],
            );
            crate::diplomacy::settle_deadlines(&mut state, 1).unwrap();
            // The restraint was kept, the payments never made: the deal is broken and archived.
            assert!(state.diplomacy.active_deals.is_empty());
            assert_eq!(statuses(&state, id), [PromiseStatus::Fulfilled, PromiseStatus::Pending, PromiseStatus::Broken]);
        });
        assert_eq!(state.diplomacy.history[0].status, DealStatus::Broken);
        assert!(trust(&state, "b", "a") < 0 || trust(&state, "a", "b") < 0, "a broken promise costs trust");
        let mut late = table();
        lockstep(|| {
            let id = active(&mut late, vec![gift(1)], vec![]);
            crate::diplomacy::settle_game_end(&mut late).unwrap();
            assert_eq!(statuses(&late, id), [PromiseStatus::Expired]);
            assert_eq!(ran("diplomacy::settle_game_end"), 1);
        });
        assert_eq!(late.diplomacy.history[0].status, DealStatus::Expired);
    }

    #[test]
    fn a_promise_is_paid_when_it_is_due_and_not_twice() {
        let mut state = table();
        let hub = crate::fixtures::plain_hub();
        let centre = SystemId::new(hub.centre.clone());
        for who in ["a", "b", "c"] {
            crate::fixtures::put(&mut state, &centre, "cruiser", &p(who), 1);
        }
        state.player_mut(&p("a")).unwrap().trade_goods = 3;
        let b_before = state.player(&p("b")).unwrap().trade_goods;
        let content = ContentStore::embedded();
        lockstep(|| {
            let id = active(&mut state, vec![gift(2)], vec![]);
            // b's own side has no promise; a's term 0 is the payment.
            assert_eq!(crate::diplomacy::fulfill_payment(&mut state, content, &hub.galaxy, id, &p("b"), 0), Err(PromiseError::UnknownTerm));
            crate::diplomacy::fulfill_payment(&mut state, content, &hub.galaxy, id, &p("a"), 0).unwrap();
            assert_eq!(crate::diplomacy::fulfill_payment(&mut state, content, &hub.galaxy, id, &p("a"), 0), Err(PromiseError::UnknownDeal(id)));
            assert!(ran("diplomacy::payment_due") >= 3);
            assert_eq!(ran("diplomacy::payment_made"), 1);
        });
        assert_eq!(state.player(&p("a")).unwrap().trade_goods, 1);
        assert_eq!(state.player(&p("b")).unwrap().trade_goods, b_before + 2);
        assert_eq!(state.diplomacy.history[0].status, DealStatus::Fulfilled);
    }

    #[test]
    fn the_history_keeps_the_last_thirty_two_deals() {
        let mut state = table();
        lockstep(|| {
            for _ in 0..33 {
                let id = crate::diplomacy::book::create_deal(&mut state, &p("a"), &p("b"), &rev(0, "a", vec![now(1)], vec![])).unwrap();
                crate::diplomacy::book::accept(&mut state, id, &p("b"), true).unwrap();
            }
            assert_eq!(ran("diplomacy::accept"), 33);
        });
        assert_eq!(state.diplomacy.history.len(), 32);
        assert_eq!(state.diplomacy.history[0].id, DealId(2), "the oldest was dropped");
    }

    #[test]
    fn only_a_promise_to_attack_a_third_party_is_an_anti_third_party_deal() {
        // The recipient promising to attack the proposer, or itself, is nobody's third party.
        for target in ["a", "b"] {
            let mut state = table();
            lockstep(|| {
                let hit = DealTerm::Attack { player: p(target), deadline_round: 2 };
                let id = crate::diplomacy::book::create_deal(&mut state, &p("a"), &p("b"), &rev(0, "a", vec![now(1)], vec![hit])).unwrap();
                crate::diplomacy::book::accept(&mut state, id, &p("b"), true).unwrap();
            });
            assert_eq!((trust(&state, "c", "a"), trust(&state, "c", "b")), (0, 0), "{target}");
        }
    }

    #[test]
    fn a_vote_assurance_nobody_bore_out_is_broken_when_its_round_ends() {
        let mut state = table();
        lockstep(|| {
            let say = SignalStatement::WillVote { agenda: "arms_reduction".into(), outcome: "for".into() };
            crate::diplomacy::signals::emit_signal(&mut state, draft(say, 1)).unwrap();
            crate::diplomacy::settle_deadlines(&mut state, 1).unwrap();
        });
        assert_eq!(state.diplomacy.recent_signals[0].status, SignalStatus::Broken);
        assert!(trust(&state, "b", "a") < 2, "a broken assurance costs the trust it earned");
    }
}
