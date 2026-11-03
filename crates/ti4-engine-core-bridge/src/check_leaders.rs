//! Equivalence of the typed leader reads (package B7) with the string engine: for every
//! `(position variant, seat)` the same question asked of the string implementation (the `String`
//! backend) and of one [`LeaderView`] per variant (the shape the flipped engine uses), all compared.
//! The variants perturb a golden position where the engine's own play rarely goes: every leader
//! unlocked or locked, enough goods, technologies and scored objectives for the commander and hero
//! checks, a fleet that meets the Letnev and L1Z1X commanders, exhausted planets, a durable grant.

use ti4_content::ContentStore;
use ti4_engine_legacy::dual::{Area, AreaMask, Backend, with_mask};
use ti4_engine_legacy::generated::IdStr;
use ti4_engine_legacy::leaders as st;
use ti4_engine_legacy::faction_abilities as fa;
use ti4_engine_legacy::leaders_view::{LeaderView, READS_ABILITIES, READS_ABILITY, READS_ACTIONS, READS_STATUS, READS_UNLOCK};
use ti4_engine_core::generated::{Ability, Leader, LeaderType, PublicObjective, StrategyCardKind, Technology};
use ti4_model::id::{LeaderId, ObjectiveId, PlayerId, TechnologyId, UnitTypeId};
use ti4_model::state::{Feat, FeatOccurrence, GameState, LeaderStatus};
use ti4_model::units::Unit;

use crate::workload::Position;

#[derive(Debug, Default, Clone)]
pub struct LeaderReport {
    pub positions: usize,
    pub variants: usize,
    pub seats: usize,
    pub answers: usize,
    pub unmapped: usize,
    pub unlock_hits: usize,
    pub mismatches: Vec<String>,
}

impl LeaderReport {
    pub fn add(&mut self, o: &Self) {
        self.positions += o.positions;
        self.variants += o.variants;
        self.seats += o.seats;
        self.answers += o.answers;
        self.unmapped += o.unmapped;
        self.unlock_hits += o.unlock_hits;
        self.mismatches.extend(o.mismatches.iter().cloned());
    }

    fn same<T: PartialEq + std::fmt::Debug>(&mut self, what: &str, string: &T, typed: &Option<T>) {
        self.answers += 1;
        match typed {
            Some(t) if t == string => {}
            other => {
                if self.mismatches.len() < 50 {
                    self.mismatches.push(format!("{what}: string {string:?} vs typed {other:?}"));
                }
            }
        }
    }
}

/// Static data: the kind and the printed window of every leader against the content store.
pub fn check_leaders_content(report: &mut LeaderReport) {
    let content = ContentStore::embedded();
    check_ability_ids(report);
    for l in Leader::ALL {
        let id = LeaderId::new(l.as_str());
        let string = with_mask(AreaMask::STRING, || st::kind_of(content, &id));
        let typed = Some(ti4_engine_core::rules::leaders::kind(l).as_str().to_owned());
        report.same(&format!("kind_of {}", l.as_str()), &string, &Some(typed));
        let string = with_mask(AreaMask::STRING, || st::uses_the_action(content, &id));
        report.same(
            &format!("uses_the_action {}", l.as_str()),
            &string,
            &Some(ti4_engine_core::rules::leaders::uses_the_action(l)),
        );
    }
}

fn unit(kind: &str, owner: &PlayerId) -> Unit {
    Unit::new(UnitTypeId::new(kind), owner.clone())
}

/// The variants of one position: a label and the perturbed state.
pub(crate) fn variants(gs: &GameState) -> Vec<(&'static str, GameState)> {
    let mut out = vec![("as is", gs.clone())];
    let all = |status: LeaderStatus| {
        let mut g = gs.clone();
        for seat in &mut g.players {
            for s in seat.leaders.values_mut() {
                *s = status;
            }
        }
        g
    };
    out.push(("all unlocked", all(LeaderStatus::Unlocked)));
    out.push(("all readied", all(LeaderStatus::Readied)));
    // Locked leaders with every commander condition within reach.
    let mut g = all(LeaderStatus::Locked);
    let ids: Vec<PlayerId> = g.players.iter().map(|p| p.id.clone()).collect();
    for (i, id) in ids.iter().enumerate() {
        for obj in PublicObjective::ALL.iter().take(3 + i % 2) {
            g.scored_objectives.entry(id.clone()).or_default().insert(ObjectiveId::new(obj.as_str()));
        }
        let home = g.players[i].home_system.clone();
        if let Some(seat) = g.player_mut(id) {
            seat.trade_goods = 10 + i32::try_from(i % 2).unwrap_or(0);
            for t in Technology::ALL.iter().take(8) {
                seat.technologies.insert(TechnologyId::new(t.as_str()));
            }
            seat.event_feats.push((Feat::LastToPass, FeatOccurrence(1)));
        }
        if let Some(home) = home {
            let sys = g.board.entry(home).or_default();
            for _ in 0..4 {
                sys.units.push(unit("dreadnought", id));
            }
            for _ in 0..5 {
                sys.units.push(unit("cruiser", id));
            }
        }
    }
    out.push(("conditions met", g.clone()));
    // The same with the planets exhausted and a durable grant of a commander ability.
    let mut g2 = g;
    let exhausted: Vec<_> = g2.players.iter().flat_map(|p| g2.controlled_planets(&p.id).into_iter().map(|(_, q)| q.clone()).collect::<Vec<_>>()).collect();
    for q in exhausted {
        g2.exhausted_planets.insert(q);
    }
    if let Some(first) = ids.first() {
        g2.faction_marks.insert(format!("commander_ability:{first}:xxchacommander"), "granted".to_owned());
        if let Some(seat) = g2.player_mut(first) {
            seat.commodities = 0;
        }
    }
    // Hyper Metabolism and a foreign Cybernetic Enhancements for the status-token count.
    let factions: Vec<String> = g2.players.iter().map(|p| p.faction.to_string()).collect();
    for (i, id) in ids.iter().enumerate() {
        if let Some(seat) = g2.player_mut(id) {
            seat.technologies.insert(TechnologyId::new("hm"));
        }
        if let Some(other) = factions.get((i + 1) % factions.len().max(1)) {
            g2.promissory_notes.insert(format!("ce:{other}"), id.clone());
        }
    }
    out.push(("planets exhausted", g2));
    out
}

/// The ids the ability constants of the core name must be the ones the string engine tests.
pub fn check_ability_ids(report: &mut LeaderReport) {
    for (a, id) in [
        (Ability::Fragile, "fragile"),
        (Ability::Unrelenting, "unrelenting"),
        (Ability::Armada, "armada"),
        (Ability::Versatile, "versatile"),
        (Ability::Analytical, "analytical"),
        (Ability::Brilliant, "brilliant"),
        (Ability::Arbiters, "arbiters"),
        (Ability::GuildShips, "guild_ships"),
        (Ability::MastersOfTrade, "master_of_trade"),
    ] {
        report.same(&format!("ability id {id}"), &id.to_owned(), &Some(a.as_str().to_owned()));
    }
    for (t, id) in [(Technology::HyperMetabolism, "hm")] {
        report.same(&format!("technology id {id}"), &id.to_owned(), &Some(t.as_str().to_owned()));
    }
    report.same(
        "promissory id ce",
        &"ce".to_owned(),
        &Some(ti4_engine_core::generated::PromissoryNote::CyberneticEnhancements.as_str().to_owned()),
    );
}

const KINDS: [StrategyCardKind; 8] = StrategyCardKind::ALL;

fn string_kind(kind: StrategyCardKind) -> ti4_content::StrategyCardKind {
    use ti4_content::StrategyCardKind as S;
    match kind {
        StrategyCardKind::Leadership => S::Leadership,
        StrategyCardKind::Diplomacy => S::Diplomacy,
        StrategyCardKind::Politics => S::Politics,
        StrategyCardKind::Construction => S::Construction,
        StrategyCardKind::Trade => S::Trade,
        StrategyCardKind::Warfare => S::Warfare,
        StrategyCardKind::Technology => S::Technology,
        StrategyCardKind::Imperial => S::Imperial,
    }
}

/// Compare every leader read on one position and its variants.
pub fn check_leaders_position(p: &Position, report: &mut LeaderReport) {
    let content = ContentStore::embedded();
    report.positions += 1;
    for (name, g) in variants(&p.state) {
        report.variants += 1;
        let label = format!("{} [{name}]", p.label);
        let galaxy = Some(&p.galaxy);
        let view = |reads| LeaderView::of(&g, Some(content), p.sources, galaxy, reads);
        let (Some(unlock), Some(actions), Some(status), Some(ability)) =
            (view(READS_UNLOCK), view(READS_ACTIONS), view(READS_STATUS), view(READS_ABILITY))
        else {
            report.unmapped += 1;
            continue;
        };
        let Some(abil) = view(READS_ABILITIES) else {
            report.unmapped += 1;
            continue;
        };
        let string = AreaMask::STRING.with(Area::LeadersAbilities, Backend::String);
        with_mask(string, || {
            for seat in &g.players {
                report.seats += 1;
                let pid = &seat.id;
                let at = |what: &str| format!("{label} {pid} {what}");
                report.same(&at("usable"), &st::usable(&g, content, pid), &status.usable(pid));
                for kind in LeaderType::ALL {
                    report.same(&at("of_kind"), &st::of_kind(&g, content, pid, kind.as_str()), &status.of_kind(pid, kind.as_str()));
                }
                report.same(&at("combines"), &st::combines_planet_values(&g, pid), &status.combines_planet_values(pid));
                let unlockable = st::unlockable(&g, content, p.sources, galaxy, pid);
                report.unlock_hits += unlockable.len();
                report.same(&at("unlockable"), &unlockable, &unlock.unlockable(pid));
                report.same(&at("action_candidates"), &st::action_candidates(&g, content, pid), &actions.action_candidates(pid));
                // Faction abilities.
                report.same(&at("abilities"), &fa::of_player(&g, content, pid), &abil.abilities_of(pid));
                for a in Ability::ALL {
                    report.same(&at(&format!("has {}", a.as_str())), &fa::has(&g, content, pid, a.as_str()), &abil.has_ability(pid, a.as_str()));
                }
                for ctx in ["space", "ground"] {
                    report.same(&at("combat_modifier"), &fa::combat_modifier(&g, content, pid, ctx), &abil.combat_modifier(pid, ctx));
                }
                report.same(&at("fleet_supply"), &fa::fleet_supply(&g, content, pid, 7), &abil.fleet_supply(pid, 7));
                report.same(&at("armada_bonus"), &fa::armada_bonus(&g, content, pid), &abil.armada_bonus(pid));
                report.same(&at("status_tokens"), &fa::status_tokens(&g, content, pid, 2), &abil.status_tokens(pid, 2));
                report.same(&at("trades_action_cards"), &fa::trades_action_cards(&g, content, pid), &abil.trades_action_cards(pid));
                report.same(&at("ignores_neighbours"), &fa::ignores_neighbours(&g, content, pid), &abil.ignores_neighbours(pid));
                for t in Technology::ALL {
                    report.same(
                        &at(&format!("waived {}", t.as_str())),
                        &fa::waived_prerequisites(&g, content, p.sources, pid, t.as_str()),
                        &abil.waived_prerequisites(pid, t.as_str()),
                    );
                }
                for card in std::iter::once(None).chain(KINDS.iter().map(|k| Some(string_kind(*k)))) {
                    report.same(&at("substitutes_primary"), &fa::substitutes_primary(&g, content, pid, card), &abil.substitutes_primary(pid, card));
                    report.same(&at("secondary_is_free"), &fa::secondary_is_free(&g, content, pid, card), &abil.secondary_is_free(pid, card));
                }
                for l in Leader::ALL {
                    let id = LeaderId::new(l.as_str());
                    report.same(&at(&format!("status {}", l.as_str())), &st::status(&g, pid, &id), &status.status(pid, &id));
                    report.same(
                        &at(&format!("commander_unlocked {}", l.as_str())),
                        &st::commander_unlocked(&g, content, p.sources, galaxy, pid, &id),
                        &unlock.commander_unlocked(pid, &id),
                    );
                    if ti4_engine_core::rules::leaders::kind(l) == LeaderType::Commander {
                        report.same(
                            &at(&format!("has_commander_ability {}", l.as_str())),
                            &ti4_engine_legacy::promissory::has_commander_ability(&g, pid, l.as_str()),
                            &ability.has_commander_ability(pid, l.as_str()),
                        );
                    }
                }
            }
        });
    }
}
