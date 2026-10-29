//! The text of the ordered emission queue (package P1; docs/todo/P1.md 5 / P1-5): `Emit` -> the exact line of
//! `Game::events`, and back. The core holds the plain events as data (`state::PlainRec`: a closed kind and up to two typed
//! arguments); the names, the argument texts and the `:` joins are here and nowhere in the core.
//!
//! * A plain event is `NAME`, `NAME:a` or `NAME:a:b`. The name is the SCREAMING_SNAKE of the kind (the same rule as the
//!   typed event names, `kernel_sync::text`), arguments are rendered by their type: a seat by its owner id, a generated
//!   id by its corpus id, an outcome as the vote prints it (`for`, `against`, an owner id, a planet, a card, a law, a
//!   secret), a command token pool by its `Debug` name (`Tactic`, `Strategic`, `Fleet`: the string prints
//!   `{pool:?}`), a count in decimal.
//! * A typed event is its bare name, with no `#id`: what `mirror_timing_log` copies.
//! * A text outside the catalogue (or whose arguments the typed form cannot hold) is `PlainKind::Untyped` carrying the
//!   whole text as a code: it renders back verbatim, and `parse` reports it so the census can count it.

use std::sync::OnceLock;

use ti4_engine_core::choice::strategy::Pool;
use ti4_engine_core::state::{CoreState, Emit, ObjectiveRef, PlainArg, PlainKind, PlainRec, PlainSlot};
use ti4_engine_core::Seat;

use crate::core_sync::Ctx;
use crate::dual_window::Pushed;
use ti4_engine_core::generated::{Agenda, Leader, Planet, PublicObjective, SecretObjective, StrategyCard, System, Technology};

use crate::generated::IdStr;
use crate::kernel_sync::text::{event_text, snake};
use crate::render::Render;

fn kind_names() -> &'static Vec<String> {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| PlainKind::ALL.iter().map(|k| snake(&format!("{k:?}"), true)).collect())
}

/// The name of a plain event kind (`QuashUsed` is `QUASH_USED`).
#[must_use]
pub fn kind_text(kind: PlainKind) -> &'static str {
    let i = PlainKind::ALL.iter().position(|k| *k == kind).expect("a kind is in ALL");
    &kind_names()[i]
}

fn seat_text(r: &Render, seat: Seat) -> Option<&str> {
    r.seats.get(usize::from(seat.0)).map(String::as_str)
}

fn arg_text(r: &Render, cx: &Ctx, arg: PlainArg) -> Option<String> {
    Some(match arg {
        PlainArg::None => return None,
        PlainArg::Seat(s) => seat_text(r, s)?.to_owned(),
        PlainArg::System(x) => x.as_str().to_owned(),
        PlainArg::Planet(x) => x.as_str().to_owned(),
        PlainArg::Agenda(x) => x.as_str().to_owned(),
        PlainArg::Leader(x) => x.as_str().to_owned(),
        PlainArg::Card(x) => x.as_str().to_owned(),
        PlainArg::Tech(x) => x.as_str().to_owned(),
        PlainArg::Obj(ObjectiveRef::Public(x)) => x.as_str().to_owned(),
        PlainArg::Obj(ObjectiveRef::Secret(x)) => x.as_str().to_owned(),
        PlainArg::Outcome(o) => cx.keys.render_outcome(o)?,
        PlainArg::Pool(Pool::Tactic) => "Tactic".to_owned(),
        PlainArg::Pool(Pool::Fleet) => "Fleet".to_owned(),
        PlainArg::Pool(Pool::Strategic) => "Strategic".to_owned(),
        PlainArg::Count(n) => n.to_string(),
        PlainArg::Code(c) => r.code_text(c),
    })
}

/// The line of `Game::events` a plain event is. `None`: a seat or an outcome the tables cannot name.
#[must_use]
pub fn plain_text(r: &Render, cx: &Ctx, rec: &PlainRec) -> Option<String> {
    if rec.kind == PlainKind::Untyped {
        return arg_text(r, cx, rec.a);
    }
    let mut out = kind_text(rec.kind).to_owned();
    for arg in [rec.a, rec.b] {
        if arg == PlainArg::None {
            continue;
        }
        out.push(':');
        out.push_str(&arg_text(r, cx, arg)?);
    }
    Some(out)
}

/// The record of a line of `Game::events`: the catalogue kind when the name and every argument parse and render back to
/// exactly `text`, else [`PlainKind::Untyped`] holding the whole text. The `bool` is `true` for a typed record.
pub fn parse_plain(r: &Render, cx: &Ctx, text: &str) -> (PlainRec, bool) {
    let untyped = || (PlainRec::untyped(r.code(text)), false);
    let mut parts = text.split(':');
    let name = parts.next().unwrap_or(text);
    let Some(kind) = PlainKind::ALL.iter().copied().find(|k| *k != PlainKind::Untyped && kind_text(*k) == name) else { return untyped() };
    let shape = kind.shape();
    let mut arg = |slot: PlainSlot| -> Option<PlainArg> {
        match (slot, parts.next()) {
            (PlainSlot::None | PlainSlot::MaybeSystem, None) => Some(PlainArg::None),
            (_, None) | (PlainSlot::None, Some(_)) => None,
            (PlainSlot::Seat, Some(t)) => r.seat(t).map(PlainArg::Seat),
            (PlainSlot::System | PlainSlot::MaybeSystem, Some(t)) => System::from_id(t).map(PlainArg::System),
            (PlainSlot::Planet, Some(t)) => Planet::from_id(t).map(PlainArg::Planet),
            (PlainSlot::Agenda, Some(t)) => Agenda::from_id(t).map(PlainArg::Agenda),
            (PlainSlot::Leader, Some(t)) => Leader::from_id(t).map(PlainArg::Leader),
            (PlainSlot::Card, Some(t)) => StrategyCard::from_id(t).map(PlainArg::Card),
            (PlainSlot::Tech, Some(t)) => Technology::from_id(t).map(PlainArg::Tech),
            (PlainSlot::Obj, Some(t)) => PublicObjective::from_id(t)
                .map(|x| PlainArg::Obj(ObjectiveRef::Public(x)))
                .or_else(|| SecretObjective::from_id(t).map(|x| PlainArg::Obj(ObjectiveRef::Secret(x)))),
            (PlainSlot::Outcome, Some(t)) => cx.keys.parse_outcome(t, None).map(PlainArg::Outcome),
            (PlainSlot::Pool, Some(t)) => match t {
                "Tactic" => Some(PlainArg::Pool(Pool::Tactic)),
                "Fleet" => Some(PlainArg::Pool(Pool::Fleet)),
                "Strategic" => Some(PlainArg::Pool(Pool::Strategic)),
                _ => None,
            },
            (PlainSlot::Count, Some(t)) => t.parse::<u16>().ok().map(PlainArg::Count),
            (PlainSlot::Text, Some(_)) => None,
        }
    };
    let (Some(a), Some(b)) = (arg(shape.a), arg(shape.b)) else { return untyped() };
    let rec = PlainRec { kind, a, b };
    // Anything left over, or a record that does not render to the same line, is not the typed form of this text.
    if parts.next().is_some() || !rec.is_valid() || plain_text(r, cx, &rec).as_deref() != Some(text) {
        return untyped();
    }
    (rec, true)
}

/// One line of the queue as the recorder sees a push: a plain event as its text, a typed one as its mirrored name.
#[must_use]
pub fn pushed_of(r: &Render, cx: &Ctx, emit: &Emit) -> Option<Pushed> {
    match emit {
        Emit::Plain(p) => plain_text(r, cx, p).map(Pushed::Plain),
        Emit::Typed(t) => Some(Pushed::Mirrored(event_text(r, t.kind))),
    }
}

/// The lines of the queue from `from` on (a window's own pushes: take `len` before it ran), in order. `None`: a line
/// the tables cannot name.
#[must_use]
pub fn pushed_since(r: &Render, cx: &Ctx, core: &CoreState, from: usize) -> Option<Vec<Pushed>> {
    core.ledger.events.iter().skip(from).map(|e| pushed_of(r, cx, e)).collect()
}

/// The whole queue as `Game::events` holds it.
#[must_use]
pub fn events_text(r: &Render, cx: &Ctx, core: &CoreState) -> Option<Vec<String>> {
    Some(pushed_since(r, cx, core, 0)?.into_iter().map(|p| p.text().to_owned()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine_core::keys::Outcome;
    use ti4_engine_core::state::EventKind;
    use ti4_model::state::GameState;

    fn tables() -> (Ctx, Render) {
        let gs: GameState = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "xxcha"), ("c", "arborec")], ti4_model::content_types::DEFAULT);
        crate::dual_window::tables(&gs).expect("three owners")
    }

    /// One sample record per kind with the text the string prints for it (docs/todo/P1.md 3).
    fn samples() -> Vec<(PlainRec, String)> {
        use PlainArg::{Agenda as Ag, Card, Count, Leader as Ld, Obj, Outcome as Out, Planet as Pl, Pool as Pol, Seat as St, System as Sy, Tech};
        use PlainKind::*;
        let (a, b) = (St(Seat(0)), St(Seat(1)));
        let law = Ag(Agenda::ChecksAndBalances);
        let obj = Obj(ObjectiveRef::Public(PublicObjective::ALL[0]));
        let sys = Sy(System::ALL[7]);
        let (tech, sysid) = (Technology::ALL[3].as_str(), System::ALL[7].as_str());
        let (leader, planet, o) = (Leader::ALL[2].as_str(), Planet::ALL[4].as_str(), PublicObjective::ALL[0].as_str());
        let v: Vec<(PlainRec, String)> = vec![
            (PlainRec::bare(ActionPhaseBegan), "ACTION_PHASE_BEGAN".to_owned()),
            (PlainRec::one(AgendaDiscarded, law), "AGENDA_DISCARDED:checks".to_owned()),
            (PlainRec::one(AgendaEffectDeferred, law), "AGENDA_EFFECT_DEFERRED:checks".to_owned()),
            (PlainRec::one(AgendaEffectResolved, law), "AGENDA_EFFECT_RESOLVED:checks".to_owned()),
            (PlainRec::one(AgendaEffectUnresolved, law), "AGENDA_EFFECT_UNRESOLVED:checks".to_owned()),
            (PlainRec::two(AgendaOutcomeRedirected, Out(Outcome::For), b), "AGENDA_OUTCOME_REDIRECTED:for:b".to_owned()),
            (PlainRec::bare(AgendaPhaseBegan), "AGENDA_PHASE_BEGAN".to_owned()),
            (PlainRec::bare(AgendaPhaseResolved), "AGENDA_PHASE_RESOLVED".to_owned()),
            (PlainRec::one(AgendaPredictionCorrect, a), "AGENDA_PREDICTION_CORRECT:a".to_owned()),
            (PlainRec::two(AgendaResolved, law, Out(Outcome::Against)), "AGENDA_RESOLVED:checks:against".to_owned()),
            (PlainRec::one(AgendaRevealed, law), "AGENDA_REVEALED:checks".to_owned()),
            (PlainRec::bare(CeasefireUsed), "CEASEFIRE_USED".to_owned()),
            (PlainRec::one(CommandTokenGained, Pol(Pool::Strategic)), "COMMAND_TOKEN_GAINED:Strategic".to_owned()),
            (PlainRec::bare(CommandTokensGained), "COMMAND_TOKENS_GAINED".to_owned()),
            (PlainRec::bare(ComponentActionFailed), "COMPONENT_ACTION_FAILED".to_owned()),
            (PlainRec::bare(ComponentActionResolved), "COMPONENT_ACTION_RESOLVED".to_owned()),
            (PlainRec::two(CovertAgendaResolved, law, Out(Outcome::For)), "COVERT_AGENDA_RESOLVED:checks:for".to_owned()),
            (PlainRec::bare(Counteroffer), "COUNTEROFFER".to_owned()),
            (PlainRec::bare(DiplomacyPaymentFulfilled), "DIPLOMACY_PAYMENT_FULFILLED".to_owned()),
            (PlainRec::one(EResSiphons, a), "E_RES_SIPHONS:a".to_owned()),
            (PlainRec::two(EntropicScarTechnologyGained, a, Tech(Technology::ALL[3])), format!("ENTROPIC_SCAR_TECHNOLOGY_GAINED:a:{tech}")),
            (PlainRec::bare(ExpeditionSliceClaimed), "EXPEDITION_SLICE_CLAIMED".to_owned()),
            (PlainRec::one(ExtremeDuress, b), "EXTREME_DURESS:b".to_owned()),
            (PlainRec::bare(FractureEnteredPlay), "FRACTURE_ENTERED_PLAY".to_owned()),
            (PlainRec::bare(FreeTacticalAction), "FREE_TACTICAL_ACTION".to_owned()),
            (PlainRec::bare(FrontierExplored), "FRONTIER_EXPLORED".to_owned()),
            (PlainRec::bare(GameFinished), "GAME_FINISHED".to_owned()),
            (PlainRec::two(GenesisInfantry, a, sys), format!("GENESIS_INFANTRY:a:{sysid}")),
            (PlainRec::bare(InvasionResolved), "INVASION_RESOLVED".to_owned()),
            (PlainRec::two(LawEnacted, law, Out(Outcome::For)), "LAW_ENACTED:checks:for".to_owned()),
            (PlainRec::bare(LeaderAbilityResolved), "LEADER_ABILITY_RESOLVED".to_owned()),
            (PlainRec::one(LeaderUnlocked, Ld(Leader::ALL[2])), format!("LEADER_UNLOCKED:{leader}")),
            (PlainRec::bare(MilitarySupportUsed), "MILITARY_SUPPORT_USED".to_owned()),
            (PlainRec::two(ObjectiveReplaced, obj, obj), format!("OBJECTIVE_REPLACED:{o}:{o}")),
            (PlainRec::one(ObjectiveScored, obj), format!("OBJECTIVE_SCORED:{o}")),
            (PlainRec::bare(PlayerPassed), "PLAYER_PASSED".to_owned()),
            (PlainRec::bare(PoliticalFavorUsed), "POLITICAL_FAVOR_USED".to_owned()),
            (PlainRec::one(PoliticalSecretUsed, b), "POLITICAL_SECRET_USED:b".to_owned()),
            (PlainRec::bare(ProductionResolved), "PRODUCTION_RESOLVED".to_owned()),
            (PlainRec::two(QuantumDatahubSwap, a, b), "QUANTUM_DATAHUB_SWAP:a:b".to_owned()),
            (PlainRec::bare(QuashUsed), "QUASH_USED".to_owned()),
            (PlainRec::bare(RoundBegan), "ROUND_BEGAN".to_owned()),
            (PlainRec::one(ScanlinkExplored, Pl(Planet::ALL[4])), format!("SCANLINK_EXPLORED:{planet}")),
            (PlainRec::bare(ShipLostToGravityRift), "SHIP_LOST_TO_GRAVITY_RIFT".to_owned()),
            (PlainRec::bare(ShipMoved), "SHIP_MOVED".to_owned()),
            (PlainRec::bare(SpaceCombatResolved), "SPACE_COMBAT_RESOLVED".to_owned()),
            (PlainRec::two(SpatialConduit, b, sys), format!("SPATIAL_CONDUIT:b:{sysid}")),
            (PlainRec::one(SpecOpsLost, a), "SPEC_OPS_LOST:a".to_owned()),
            (PlainRec::two(SpecOpsReturned, a, Count(3)), "SPEC_OPS_RETURNED:a:3".to_owned()),
            (PlainRec::one(SpecOpsSurvived, b), "SPEC_OPS_SURVIVED:b".to_owned()),
            (PlainRec::bare(StartingTechnologyChosen), "STARTING_TECHNOLOGY_CHOSEN".to_owned()),
            (PlainRec::bare(StatusBookkeepingResolved), "STATUS_BOOKKEEPING_RESOLVED".to_owned()),
            (PlainRec::bare(StatusPhaseBegan), "STATUS_PHASE_BEGAN".to_owned()),
            (PlainRec::bare(StatusPhaseResolved), "STATUS_PHASE_RESOLVED".to_owned()),
            (PlainRec::bare(StatusScoringBegan), "STATUS_SCORING_BEGAN".to_owned()),
            (PlainRec::bare(StrategicActionBegan), "STRATEGIC_ACTION_BEGAN".to_owned()),
            (PlainRec::one(StrategicActionCancelled, Card(StrategyCard::Politics)), format!("STRATEGIC_ACTION_CANCELLED:{}", StrategyCard::Politics.as_str())),
            (PlainRec::bare(StrategicActionComplete), "STRATEGIC_ACTION_COMPLETE".to_owned()),
            (PlainRec::bare(StrategyCardChosen), "STRATEGY_CARD_CHOSEN".to_owned()),
            (PlainRec::two(StrategyCardsSwapped, a, b), "STRATEGY_CARDS_SWAPPED:a:b".to_owned()),
            (PlainRec::bare(StrategySecondaryDeclined), "STRATEGY_SECONDARY_DECLINED".to_owned()),
            (PlainRec::bare(StrategySecondaryFollowed), "STRATEGY_SECONDARY_FOLLOWED".to_owned()),
            (PlainRec::bare(StrategySecondaryPreviewComplete), "STRATEGY_SECONDARY_PREVIEW_COMPLETE".to_owned()),
            (PlainRec::one(SupportForTheThroneReturned, b), "SUPPORT_FOR_THE_THRONE_RETURNED:b".to_owned()),
            (PlainRec::bare(SystemActivated), "SYSTEM_ACTIVATED".to_owned()),
            (PlainRec::one(SystemActivated, sys), format!("SYSTEM_ACTIVATED:{sysid}")),
            (PlainRec::bare(TacticalActionBegan), "TACTICAL_ACTION_BEGAN".to_owned()),
            (PlainRec::bare(TacticalActionComplete), "TACTICAL_ACTION_COMPLETE".to_owned()),
            (PlainRec::bare(Transaction), "TRANSACTION".to_owned()),
            (PlainRec::bare(TransactionAbandoned), "TRANSACTION_ABANDONED".to_owned()),
            (PlainRec::bare(TransactionOffered), "TRANSACTION_OFFERED".to_owned()),
            (PlainRec::bare(TransactionRefused), "TRANSACTION_REFUSED".to_owned()),
            (PlainRec::bare(TransactionRejected), "TRANSACTION_REJECTED".to_owned()),
            (PlainRec::bare(TurnClosing), "TURN_CLOSING".to_owned()),
            (PlainRec::one(TurnEndedByMahactCommander, a), "TURN_ENDED_BY_MAHACT_COMMANDER:a".to_owned()),
            (PlainRec::bare(TurnEndedByMinisterOfPeace), "TURN_ENDED_BY_MINISTER_OF_PEACE".to_owned()),
            (PlainRec::one(TurnEndedByNullificationField, b), "TURN_ENDED_BY_NULLIFICATION_FIELD:b".to_owned()),
            (PlainRec::one(TurnEndedByStarlancer, a), "TURN_ENDED_BY_STARLANCER:a".to_owned()),
            (PlainRec::bare(TurnPassed), "TURN_PASSED".to_owned()),
            (PlainRec::one(TurnPuppet, b), "TURN_PUPPET:b".to_owned()),
            (PlainRec::bare(TurnRetained), "TURN_RETAINED".to_owned()),
            (PlainRec::one(TurnSkipped, a), "TURN_SKIPPED:a".to_owned()),
        ];
        v
    }

    #[test]
    fn every_kind_renders_to_the_line_the_string_prints_and_parses_back() {
        let (cx, r) = tables();
        let samples = samples();
        let mut seen: Vec<PlainKind> = samples.iter().map(|(rec, _)| rec.kind).collect();
        seen.sort();
        seen.dedup();
        let missing: Vec<_> = PlainKind::ALL.iter().filter(|k| !seen.contains(k) && **k != PlainKind::Untyped).collect();
        assert!(missing.is_empty(), "kinds without a byte test: {missing:?}");
        for (rec, text) in &samples {
            assert_eq!(plain_text(&r, &cx, rec).as_deref(), Some(text.as_str()), "{rec:?}");
            let (back, typed) = parse_plain(&r, &cx, text);
            assert!(typed, "{text} must parse to its typed form");
            assert_eq!(back, *rec, "{text}");
        }
    }

    #[test]
    fn the_name_of_a_kind_is_the_screaming_snake_of_its_variant() {
        assert_eq!(kind_text(PlainKind::QuashUsed), "QUASH_USED");
        assert_eq!(kind_text(PlainKind::EResSiphons), "E_RES_SIPHONS");
        assert_eq!(kind_text(PlainKind::TurnEndedByMinisterOfPeace), "TURN_ENDED_BY_MINISTER_OF_PEACE");
        assert_eq!(kind_text(PlainKind::SpecOpsReturned), "SPEC_OPS_RETURNED");
    }

    #[test]
    fn the_pool_prints_as_the_debug_name_the_string_uses() {
        let (cx, r) = tables();
        for (pool, text) in [(Pool::Tactic, "Tactic"), (Pool::Strategic, "Strategic"), (Pool::Fleet, "Fleet")] {
            let rec = PlainRec::one(PlainKind::CommandTokenGained, PlainArg::Pool(pool));
            assert_eq!(plain_text(&r, &cx, &rec), Some(format!("COMMAND_TOKEN_GAINED:{text}")));
            // The string prints `{pool:?}` of its own `TokenPool`; the names must stay equal.
            let string_side = format!("{:?}", pool_text(pool));
            assert_eq!(string_side, text);
        }
    }

    fn pool_text(pool: Pool) -> ti4_model::state::TokenPool {
        match pool {
            Pool::Tactic => ti4_model::state::TokenPool::Tactic,
            Pool::Fleet => ti4_model::state::TokenPool::Fleet,
            Pool::Strategic => ti4_model::state::TokenPool::Strategic,
        }
    }

    #[test]
    fn a_name_outside_the_catalogue_is_untyped_and_renders_back_verbatim() {
        let (cx, r) = tables();
        for text in ["AGENDA_TALKS_ENDED:a", "SOME_MOD_EVENT", "QUASH_USED:a", "LAW_ENACTED:not_an_agenda:for", "AGENDA_RESOLVED:checks", "TURN_SKIPPED:nobody"] {
            let (rec, typed) = parse_plain(&r, &cx, text);
            assert!(!typed, "{text}");
            assert_eq!(rec.kind, PlainKind::Untyped, "{text}");
            assert!(rec.is_valid());
            assert_eq!(plain_text(&r, &cx, &rec).as_deref(), Some(text));
        }
    }

    #[test]
    fn a_typed_event_is_its_bare_name_and_the_queue_keeps_the_order() {
        let (cx, r) = tables();
        let mut s = CoreState::all_loaded();
        ti4_engine_core::kernel::fx::plain(&mut s, PlainRec::one(PlainKind::AgendaRevealed, PlainArg::Agenda(Agenda::ChecksAndBalances)));
        s.ledger.events.push(Emit::Typed(ti4_engine_core::kernel::fx::event(EventKind::AgendaRevealed, ti4_engine_core::state::Payload::new())));
        ti4_engine_core::kernel::fx::plain(&mut s, PlainRec::bare(PlainKind::QuashUsed));
        assert_eq!(events_text(&r, &cx, &s).unwrap(), ["AGENDA_REVEALED:checks", "AGENDA_REVEALED", "QUASH_USED"]);
        assert_eq!(pushed_since(&r, &cx, &s, 2).unwrap(), [Pushed::Plain("QUASH_USED".to_owned())]);
    }

    #[test]
    fn a_strategy_card_prints_as_its_corpus_id_not_its_short_name() {
        // The string prints the `StrategyCardId` it holds (`{card}`): the corpus id (`pok3politics`), not "politics".
        let (cx, r) = tables();
        let rec = PlainRec::one(PlainKind::StrategicActionCancelled, PlainArg::Card(StrategyCard::Politics));
        assert_eq!(plain_text(&r, &cx, &rec).as_deref(), Some("STRATEGIC_ACTION_CANCELLED:pok3politics"));
        let (back, typed) = parse_plain(&r, &cx, "STRATEGIC_ACTION_CANCELLED:pok3politics");
        assert!(typed && back == rec);
        // The short name is no corpus id: it stays an untyped text, verbatim.
        let (short, typed) = parse_plain(&r, &cx, "STRATEGIC_ACTION_CANCELLED:politics");
        assert!(!typed && short.kind == PlainKind::Untyped);
        assert_eq!(plain_text(&r, &cx, &short).as_deref(), Some("STRATEGIC_ACTION_CANCELLED:politics"));
    }
}
