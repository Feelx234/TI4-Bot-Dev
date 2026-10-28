use ti4_engine_core::containers::{Set, SeatSet};
use ti4_engine_core::generated::{
    ActionCard, Breakthrough, Explore, ExploreType, Faction, Leader, Planet, Relic, SecretObjective, StrategyCard,
    System, Technology, Unit,
};
use ti4_engine_core::state::{
    ExtraVotes, Feat, FragmentCounts, LeaderStatus, Plot, PlotCard, Player, ReactionMode, RoundKind, Slice,
};
use ti4_engine_core::state::{CoreState, Players as CorePlayers};
use ti4_model::id::{
    ActionCardId, BreakthroughId, FactionId, LeaderId, PlanetId, PlayerId, RelicId, SecretObjectiveId, StrategyCardId, SystemId,
    TechnologyId, UnitTypeId,
};
use ti4_model::state::{self as m, GameState};

use super::{Sources, SliceCodec};
use crate::core_sync::conv::{id, list, narrow, opt32, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};

const S: Slice = Slice::Players;

pub struct Players;

const FRAGMENT_KEYS: [(ExploreType, &str); 4] = [
    (ExploreType::Cultural, "CULTURAL"),
    (ExploreType::Frontier, "FRONTIER"),
    (ExploreType::Hazardous, "HAZARDOUS"),
    (ExploreType::Industrial, "INDUSTRIAL"),
];

fn plot_card(text: &str) -> Option<PlotCard> {
    Some(match text {
        "assail" => PlotCard::Assail,
        "enervate" => PlotCard::Enervate,
        "extract" => PlotCard::Extract,
        "seethe" => PlotCard::Seethe,
        "siphon" => PlotCard::Siphon,
        _ => return None,
    })
}

fn plot_card_text(c: PlotCard) -> &'static str {
    match c {
        PlotCard::Assail => "assail",
        PlotCard::Enervate => "enervate",
        PlotCard::Extract => "extract",
        PlotCard::Seethe => "seethe",
        PlotCard::Siphon => "siphon",
    }
}

fn feat(f: m::Feat) -> Feat {
    use m::Feat as M;
    match f {
        M::DestroyedACapitalShip => Feat::DestroyedACapitalShip,
        M::BombardedOutTheLastGroundForces => Feat::BombardedOutTheLastGroundForces,
        M::SpaceCannonTookTheLastNonFighters => Feat::SpaceCannonTookTheLastNonFighters,
        M::BarrageTookTheLastFighters => Feat::BarrageTookTheLastFighters,
        M::WonAgainstThePointsLeader => Feat::WonAgainstThePointsLeader,
        M::WonBesideASurvivingFlagship => Feat::WonBesideASurvivingFlagship,
        M::WonAgainstANoteHolder => Feat::WonAgainstANoteHolder,
        M::WonInAnAnomaly => Feat::WonInAnAnomaly,
        M::WonInARivalHome => Feat::WonInARivalHome,
        M::HeldThreeShipsAfterASpaceCombat => Feat::HeldThreeShipsAfterASpaceCombat,
        M::LostAHomePlanet => Feat::LostAHomePlanet,
        M::ElectedByAnAgenda => Feat::ElectedByAnAgenda,
        M::LastToPass => Feat::LastToPass,
    }
}

fn feat_back(f: Feat) -> m::Feat {
    use m::Feat as M;
    match f {
        Feat::DestroyedACapitalShip => M::DestroyedACapitalShip,
        Feat::BombardedOutTheLastGroundForces => M::BombardedOutTheLastGroundForces,
        Feat::SpaceCannonTookTheLastNonFighters => M::SpaceCannonTookTheLastNonFighters,
        Feat::BarrageTookTheLastFighters => M::BarrageTookTheLastFighters,
        Feat::WonAgainstThePointsLeader => M::WonAgainstThePointsLeader,
        Feat::WonBesideASurvivingFlagship => M::WonBesideASurvivingFlagship,
        Feat::WonAgainstANoteHolder => M::WonAgainstANoteHolder,
        Feat::WonInAnAnomaly => M::WonInAnAnomaly,
        Feat::WonInARivalHome => M::WonInARivalHome,
        Feat::HeldThreeShipsAfterASpaceCombat => M::HeldThreeShipsAfterASpaceCombat,
        Feat::LostAHomePlanet => M::LostAHomePlanet,
        Feat::ElectedByAnAgenda => M::ElectedByAnAgenda,
        Feat::LastToPass => M::LastToPass,
    }
}

fn leader_status(s: m::LeaderStatus) -> LeaderStatus {
    match s {
        m::LeaderStatus::Locked => LeaderStatus::Locked,
        m::LeaderStatus::Readied => LeaderStatus::Readied,
        m::LeaderStatus::Exhausted => LeaderStatus::Exhausted,
        m::LeaderStatus::Unlocked => LeaderStatus::Unlocked,
        m::LeaderStatus::Purged => LeaderStatus::Purged,
    }
}

fn leader_status_back(s: LeaderStatus) -> m::LeaderStatus {
    match s {
        LeaderStatus::Locked => m::LeaderStatus::Locked,
        LeaderStatus::Readied => m::LeaderStatus::Readied,
        LeaderStatus::Exhausted => m::LeaderStatus::Exhausted,
        LeaderStatus::Unlocked => m::LeaderStatus::Unlocked,
        LeaderStatus::Purged => m::LeaderStatus::Purged,
    }
}

fn reaction(r: m::ReactionMode) -> ReactionMode {
    match r {
        m::ReactionMode::Always => ReactionMode::Always,
        m::ReactionMode::Never => ReactionMode::Never,
        m::ReactionMode::OwnTurn => ReactionMode::OwnTurn,
    }
}

fn reaction_back(r: ReactionMode) -> m::ReactionMode {
    match r {
        ReactionMode::Always => m::ReactionMode::Always,
        ReactionMode::Never => m::ReactionMode::Never,
        ReactionMode::OwnTurn => m::ReactionMode::OwnTurn,
    }
}

const ROUND_KINDS: [RoundKind; 8] = [
    RoundKind::FighterBonus,
    RoundKind::BunkerInvasion,
    RoundKind::WarMachine,
    RoundKind::BlitzInvasion,
    RoundKind::DisableInvasion,
    RoundKind::SolarFlare,
    RoundKind::LostStar,
    RoundKind::DominusOrb,
];

fn tech_set(texts: impl IntoIterator<Item = impl AsRef<str>>) -> R<Set<Technology, 2>> {
    let mut s = Set::new();
    for t in texts {
        s.insert(id::<Technology>(S, "technology", t.as_ref())?);
    }
    Ok(s)
}

/// Load one player.
pub fn load_player(cx: &Ctx, p: &m::Player) -> R<Player> {
    let seat = cx.seat(S, p.id.as_str())?;
    let faction = id::<Faction>(S, "faction", p.faction.as_str())?;
    let mut o = Player::new(seat, faction);
    o.home_system = p.home_system.as_ref().map(|s| id::<System>(S, "system", s.as_str())).transpose()?;
    o.home_planets = list(S, "planet", p.home_planets.iter().map(PlanetId::as_str))?;
    o.strat = list(S, "strategy card", p.strategy_cards.iter().map(StrategyCardId::as_str))?;
    for c in &p.exhausted_strategy_cards {
        o.strat_exhausted.insert(id::<StrategyCard>(S, "strategy card", c.as_str())?);
    }
    o.econ.tactic_tokens = narrow(S, "tactic_tokens", p.tactic_tokens)?;
    o.econ.fleet_tokens = narrow(S, "fleet_tokens", p.fleet_tokens)?;
    o.econ.strategic_tokens = narrow(S, "strategic_tokens", p.strategic_tokens)?;
    o.econ.trade_goods = narrow(S, "trade_goods", p.trade_goods)?;
    o.econ.commodities = narrow(S, "commodities", p.commodities)?;
    o.econ.victory_points = narrow(S, "victory_points", p.victory_points)?;
    o.econ.passed = p.passed;
    o.econ.public_objectives_forbidden = p.public_objectives_forbidden;
    o.econ.stability = p.stability;
    o.econ.duress_by = p.duress_by.as_ref().map(|d| cx.seat(S, d.as_str())).transpose()?;

    o.tech.owned = tech_set(p.technologies.iter().map(TechnologyId::as_str))?;
    o.tech.exhausted = tech_set(p.exhausted_technologies.iter().map(TechnologyId::as_str))?;
    for (k, v) in &p.assimilated_technologies {
        o.tech.assimilated.insert(id::<Technology>(S, "technology", k)?, id::<Technology>(S, "technology", v.as_str())?);
    }

    let c = &mut o.cards;
    c.hand = list(S, "action card", p.action_cards.iter().map(ActionCardId::as_str))?;
    for (k, v) in &p.reaction_card_modes {
        c.reaction_modes.insert(id::<ActionCard>(S, "action card", k.as_str())?, reaction(*v));
    }
    c.secrets = list(S, "secret objective", p.secret_objectives.iter().map(SecretObjectiveId::as_str))?;
    c.relics = list(S, "relic", p.relics.iter().map(RelicId::as_str))?;
    for r in &p.exhausted_relics {
        c.relics_exhausted.insert(id::<Relic>(S, "relic", r.as_str())?);
    }
    for pl in &p.exhausted_legendary {
        c.legendary_exhausted.insert(id::<Planet>(S, "planet", pl.as_str())?, ());
    }
    let mut frags = FragmentCounts::default();
    for (k, v) in &p.relic_fragments {
        let (kind, _) = FRAGMENT_KEYS.iter().find(|(_, key)| key == k).ok_or_else(|| bad(S, "fragment trait", k))?;
        frags.set(*kind, narrow(S, "fragment count", *v)?);
    }
    c.fragments = frags;
    c.explore = list::<Explore, 4, _>(S, "exploration card", p.exploration_cards.iter())?;
    for text in &p.plots {
        let plot = ti4_model::plots::Plot::decode(text).ok_or_else(|| bad(S, "plot", text))?;
        if plot.encode() != *text {
            return Err(bad(S, "plot form", text));
        }
        let card = match &plot.card {
            None => None,
            Some(name) => Some(plot_card(name).ok_or_else(|| bad(S, "plot card", name))?),
        };
        let tokens: SeatSet = plot.tokens.iter().map(|t| cx.seat(S, t.as_str())).collect::<R<_>>()?;
        c.plots.push(Plot { card, tokens, faceup: plot.faceup });
    }
    for s in &p.plot_objectives {
        c.plot_objectives.insert(id::<SecretObjective>(S, "secret objective", s.as_str())?);
    }
    c.breakthrough = p.breakthrough.as_ref().map(|b| id::<Breakthrough>(S, "breakthrough", b.as_str())).transpose()?;

    for (k, v) in &p.leaders {
        o.leaders.status.insert(id::<Leader>(S, "leader", k.as_str())?, leader_status(*v));
    }

    let t = &mut o.timers;
    t.conduit_activation = opt32(S, "conduit_activation", p.conduit_activation)?;
    o.card_timers.move_bonus_activation = opt32(S, "move_bonus_activation", p.move_bonus_activation)?;
    t.gravity_drive_used_activation = opt32(S, "gravity_drive_used_activation", p.gravity_drive_used_activation)?;
    o.card_timers.anomalies_ignored_activation = opt32(S, "anomalies_ignored_activation", p.anomalies_ignored_activation)?;
    o.card_timers.silence_activation = opt32(S, "silence_activation", p.silence_activation)?;
    o.card_timers.silence_system = p.silence_system.as_ref().map(|s| id::<System>(S, "system", s.as_str())).transpose()?;
    t.spatial_conduit = opt32(S, "spatial_conduit", p.spatial_conduit)?;
    o.card_timers.hack_votes_last_agenda = opt32(S, "hack_votes_last_agenda", p.hack_votes_last_agenda)?;
    o.card_timers.extra_votes_agenda = p.extra_votes_agenda.map(|(agenda, votes)| ExtraVotes { agenda, votes });

    let ct = &mut o.combat_timers;
    o.card_timers.combat_bonus_round = opt32(S, "combat_bonus_round", p.combat_bonus_round)?;
    o.card_timers.rout_round = opt32(S, "rout_round", p.rout_round)?;
    o.card_timers.waylay_barrage_round = opt32(S, "waylay_barrage_round", p.waylay_barrage_round)?;
    o.card_timers.retreat_barred_round = opt32(S, "retreat_barred_round", p.retreat_barred_round)?;
    ct.extra_die_round = opt32(S, "extra_die_round", p.extra_die_round)?;
    ct.extra_die_unit = p.extra_die_unit.as_ref().map(|u| id::<Unit>(S, "unit type", u.as_str())).transpose()?;
    ct.munitions_round = opt32(S, "munitions_round", p.munitions_round)?;
    ct.ground_roll_suppressed_round = opt32(S, "ground_roll_suppressed_round", p.ground_roll_suppressed_round)?;
    ct.sustained_damage_round = opt32(S, "sustained_damage_round", p.sustained_damage_round)?;
    ct.sustained_damage_types = list(S, "unit type", p.sustained_damage_types.iter().map(UnitTypeId::as_str))?;
    o.card_timers.cancel_hits_round = p.cancel_hits_round.map(|(r, h)| Ok((r, narrow::<usize, u32>(S, "cancel_hits", h)?))).transpose()?;

    o.production_timers.fleet_supply_unlimited_until = opt32(S, "fleet_supply_unlimited_until", p.fleet_supply_unlimited_until)?;
    o.production_timers.free_production_use = opt32(S, "free_production_use", p.free_production_use)?;
    o.card_timers.agency_supply_used_turn = opt32(S, "agency_supply_used_turn", p.agency_supply_used_turn)?;
    o.card_timers.fleet_logistics_used_turn = opt32(S, "fleet_logistics_used_turn", p.fleet_logistics_used_turn)?;

    for kind in ROUND_KINDS {
        let list = match kind {
            RoundKind::FighterBonus => &p.fighter_bonus_round,
            RoundKind::BunkerInvasion => &p.bunker_invasion,
            RoundKind::WarMachine => &p.war_machine_use,
            RoundKind::BlitzInvasion => &p.blitz_invasion,
            RoundKind::DisableInvasion => &p.disable_invasion,
            RoundKind::SolarFlare => &p.solar_flare,
            RoundKind::LostStar => &p.lost_star,
            RoundKind::DominusOrb => &p.dominus_orb,
        };
        for v in list {
            o.round_marks.push(kind, *v);
        }
    }

    let x = &mut o.misc;
    x.infantry_returning = narrow(S, "infantry_returning", p.infantry_returning)?;
    x.spec_ops_returning = narrow(S, "spec_ops_returning", p.spec_ops_returning)?;
    x.spec_ops_destroyed = p.spec_ops_destroyed;
    x.spec_ops_card = p.spec_ops_card;
    x.technology_units_returning = list(S, "unit type", p.technology_units_returning.iter().map(UnitTypeId::as_str))?;
    for (owner, unit) in &p.captured_units {
        x.captured_units.push((cx.seat(S, owner.as_str())?, id::<Unit>(S, "unit type", unit.as_str())?));
    }
    for (f, n) in &p.event_feats {
        x.event_feats.push((feat(*f), n.0));
    }
    Ok(o)
}

/// Write one player back as a fresh string `Player`.
pub fn store_player(cx: &Ctx, p: &Player) -> R<m::Player> {
    let mut o = m::Player::new(PlayerId::new(cx.owner(S, p.seat)?));
    o.faction = FactionId::new(txt(p.faction));
    o.home_system = p.home_system.map(|s| SystemId::new(txt(s)));
    o.home_planets = p.home_planets.iter().map(|x| PlanetId::new(txt(*x))).collect();
    o.strategy_cards = p.strat.iter().map(|x| StrategyCardId::new(txt(*x))).collect();
    o.exhausted_strategy_cards = p.strat_exhausted.iter().map(|x| StrategyCardId::new(txt(x))).collect();
    o.tactic_tokens = i32::from(p.econ.tactic_tokens);
    o.fleet_tokens = i32::from(p.econ.fleet_tokens);
    o.strategic_tokens = i32::from(p.econ.strategic_tokens);
    o.trade_goods = i32::from(p.econ.trade_goods);
    o.commodities = i32::from(p.econ.commodities);
    o.victory_points = i32::from(p.econ.victory_points);
    o.passed = p.econ.passed;
    o.public_objectives_forbidden = p.econ.public_objectives_forbidden;
    o.stability = p.econ.stability;
    o.duress_by = p.econ.duress_by.map(|s| cx.owner(S, s).map(PlayerId::new)).transpose()?;

    o.technologies = p.tech.owned.iter().map(|t| TechnologyId::new(txt(t))).collect();
    o.exhausted_technologies = p.tech.exhausted.iter().map(|t| TechnologyId::new(txt(t))).collect();
    o.assimilated_technologies =
        p.tech.assimilated.iter().map(|(k, v)| (txt(*k).to_owned(), TechnologyId::new(txt(*v)))).collect();

    let c = &p.cards;
    o.action_cards = c.hand.iter().map(|x| ActionCardId::new(txt(*x))).collect();
    o.reaction_card_modes = c.reaction_modes.iter().map(|(k, v)| (ActionCardId::new(txt(*k)), reaction_back(*v))).collect();
    o.secret_objectives = c.secrets.iter().map(|x| SecretObjectiveId::new(txt(*x))).collect();
    o.relics = c.relics.iter().map(|x| RelicId::new(txt(*x))).collect();
    o.exhausted_relics = c.relics_exhausted.iter().map(|x| RelicId::new(txt(x))).collect();
    o.exhausted_legendary = c.legendary_exhausted.keys().map(|x| PlanetId::new(txt(x))).collect();
    for (kind, key) in FRAGMENT_KEYS {
        if let Some(n) = c.fragments.get(kind) {
            o.relic_fragments.insert(key.to_owned(), i32::from(n));
        }
    }
    o.exploration_cards = c.explore.iter().map(|x| txt(*x).to_owned()).collect();
    o.plots = c
        .plots
        .iter()
        .map(|pl| {
            let tokens = pl.tokens.iter().map(|s| cx.owner(S, s).map(PlayerId::new)).collect::<R<_>>()?;
            Ok(ti4_model::plots::Plot { card: pl.card.map(|c| plot_card_text(c).to_owned()), tokens, faceup: pl.faceup }.encode())
        })
        .collect::<R<_>>()?;
    o.plot_objectives = c.plot_objectives.iter().map(|x| SecretObjectiveId::new(txt(x))).collect();
    o.breakthrough = c.breakthrough.map(|b| BreakthroughId::new(txt(b)));

    o.leaders = p.leaders.status.iter().map(|(k, v)| (LeaderId::new(txt(*k)), leader_status_back(*v))).collect();

    let t = &p.timers;
    o.conduit_activation = t.conduit_activation.get();
    o.move_bonus_activation = p.card_timers.move_bonus_activation.get();
    o.gravity_drive_used_activation = t.gravity_drive_used_activation.get();
    o.anomalies_ignored_activation = p.card_timers.anomalies_ignored_activation.get();
    o.silence_activation = p.card_timers.silence_activation.get();
    o.silence_system = p.card_timers.silence_system.map(|s| SystemId::new(txt(s)));
    o.spatial_conduit = t.spatial_conduit.get();
    o.hack_votes_last_agenda = p.card_timers.hack_votes_last_agenda.get();
    o.extra_votes_agenda = p.card_timers.extra_votes_agenda.map(|e| (e.agenda, e.votes));

    let ct = &p.combat_timers;
    o.combat_bonus_round = p.card_timers.combat_bonus_round.get();
    o.rout_round = p.card_timers.rout_round.get();
    o.waylay_barrage_round = p.card_timers.waylay_barrage_round.get();
    o.retreat_barred_round = p.card_timers.retreat_barred_round.get();
    o.extra_die_round = ct.extra_die_round.get();
    o.extra_die_unit = ct.extra_die_unit.map(|u| UnitTypeId::new(txt(u)));
    o.munitions_round = ct.munitions_round.get();
    o.ground_roll_suppressed_round = ct.ground_roll_suppressed_round.get();
    o.sustained_damage_round = ct.sustained_damage_round.get();
    o.sustained_damage_types = ct.sustained_damage_types.iter().map(|u| UnitTypeId::new(txt(*u))).collect();
    o.cancel_hits_round = p.card_timers.cancel_hits_round.map(|(r, h)| (r, h as usize));

    o.fleet_supply_unlimited_until = p.production_timers.fleet_supply_unlimited_until.get();
    o.free_production_use = p.production_timers.free_production_use.get();
    o.agency_supply_used_turn = p.card_timers.agency_supply_used_turn.get();
    o.fleet_logistics_used_turn = p.card_timers.fleet_logistics_used_turn.get();

    for kind in ROUND_KINDS {
        let v: Vec<u32> = p.round_marks.of(kind).collect();
        match kind {
            RoundKind::FighterBonus => o.fighter_bonus_round = v,
            RoundKind::BunkerInvasion => o.bunker_invasion = v,
            RoundKind::WarMachine => o.war_machine_use = v,
            RoundKind::BlitzInvasion => o.blitz_invasion = v,
            RoundKind::DisableInvasion => o.disable_invasion = v,
            RoundKind::SolarFlare => o.solar_flare = v,
            RoundKind::LostStar => o.lost_star = v,
            RoundKind::DominusOrb => o.dominus_orb = v,
        }
    }

    let x = &p.misc;
    o.infantry_returning = i32::from(x.infantry_returning);
    o.spec_ops_returning = i32::from(x.spec_ops_returning);
    o.spec_ops_destroyed = x.spec_ops_destroyed;
    o.spec_ops_card = x.spec_ops_card;
    o.technology_units_returning = x.technology_units_returning.iter().map(|u| UnitTypeId::new(txt(*u))).collect();
    o.captured_units = x
        .captured_units
        .iter()
        .map(|(s, u)| Ok((PlayerId::new(cx.owner(S, *s)?), UnitTypeId::new(txt(*u)))))
        .collect::<R<_>>()?;
    o.event_feats = x.event_feats.iter().map(|(f, n)| (feat_back(*f), m::FeatOccurrence(*n))).collect();
    Ok(o)
}

impl SliceCodec for Players {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let list = gs.players.iter().map(|p| load_player(cx, p)).collect::<R<Vec<_>>>()?;
        core.players = CorePlayers::from_vec(list);
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        gs.players = core.players().iter().map(|p| store_player(cx, p)).collect::<R<_>>()?;
        Ok(())
    }
}
