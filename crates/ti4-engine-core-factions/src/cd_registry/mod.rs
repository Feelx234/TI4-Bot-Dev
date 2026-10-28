//! Package CD, the registry line (docs/ENGINE_CORE_FACTIONS_PLAN.md). Owns every file under this directory;
//! nobody else edits it. One sub-directory per faction, one file per tier.

use crate::{Id, Modules};

pub mod naalu;
pub mod naaz;
pub mod sardakk;
pub mod titans;
pub mod yin;
pub mod yssaril;

/// The modules this package provides, with all their tiers.
pub const MODULES: Modules = &[
    (Id::Naalu, naalu::register),
    (Id::Naaz, naaz::register),
    (Id::Sardakk, sardakk::register),
    (Id::Titans, titans::register),
    (Id::Yin, yin::register),
    (Id::Yssaril, yssaril::register),
];

/// The movement slots only (package A2's hooks), for `movement_table`.
pub const MOVEMENT: Modules = &[
    (Id::Yssaril, yssaril::register_movement),
];

pub(crate) use ti4_engine_core::rules::leaders::has_commander_ability_in as has_commander_ability;

#[cfg(test)]
pub(crate) use crate::common::testkit;

#[cfg(test)]
mod tests {
    use ti4_engine_core::generated::{Breakthrough, Faction, Leader, Planet, PromissoryNote, System, Technology, Unit};
    use ti4_engine_core::hooks::{CombatCtx, CombatUnit, Waiver};
    use ti4_engine_core::ids::{Seat, system_at};
    use ti4_engine_core::kernel::{Cx, FirstOption};
    use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Word};
    use ti4_engine_core::state::{LeaderStatus, NoteKey, Phase, Piece};

    use super::testkit::table;
    use super::{naalu, naaz, sardakk, titans, yin, yssaril};

    const NAAZ: Faction = Faction::TheNaazRokhaAlliance;
    const NAALU: Faction = Faction::TheNaaluCollective;
    const SARDAKK: Faction = Faction::SardakkNorr;
    const TITANS: Faction = Faction::TheTitansOfUl;
    const YIN: Faction = Faction::TheYinBrotherhood;
    const YSSARIL: Faction = Faction::TheYssarilTribes;
    const SOL: Faction = Faction::TheFederationOfSol;

    fn unit(owner: u8, kind: Unit, system: System, planet: Option<Planet>, ctx: CombatCtx) -> CombatUnit {
        CombatUnit { owner: Seat(owner), system: Some(system), planet, unit: kind, ctx }
    }

    #[test]
    fn naaz_supercharge_stamps_one_kind_of_combat_in_one_round() {
        let (mut s, k) = table(&[NAAZ, SOL]);
        let sys = system_at(30);
        s.meta_mut().combat_round_seq = 3;
        let space = unit(0, Unit::CruiserI, sys, None, CombatCtx::Space);
        assert_eq!(naaz::query::unit_roll_modifier(&s, &k, space), 0);
        s.marks_mut().tail.edit().insert(
            MarkKey::NaazSupercharge(Seat(0)),
            MarkVal::Rec(vec![MarkVal::Atom(Atom::Word(Word::Space)), MarkVal::Atom(Atom::Num(3))]),
        );
        assert_eq!(naaz::query::unit_roll_modifier(&s, &k, space), 1);
        assert_eq!(naaz::query::unit_roll_modifier(&s, &k, CombatUnit { ctx: CombatCtx::Ground, ..space }), 0, "the other kind");
        assert_eq!(naaz::query::unit_roll_modifier(&s, &k, CombatUnit { owner: Seat(1), ..space }), 0, "another seat");
        s.meta_mut().combat_round_seq = 4;
        assert_eq!(naaz::query::unit_roll_modifier(&s, &k, space), 0, "another round");
    }

    #[test]
    fn naaz_mechs_roll_an_extra_die_beside_the_flagship() {
        let (mut s, k) = table(&[NAAZ, SOL]);
        let sys = system_at(30);
        let mech = unit(0, Unit::Eidolon, sys, None, CombatCtx::Space);
        assert_eq!(naaz::query::unit_dice(&s, &k, mech, 1), 1, "no flagship");
        s.board_mut().sys_mut(sys).units.push(Piece::new(Unit::ViszElVir, Seat(0)));
        assert_eq!(naaz::query::unit_dice(&s, &k, mech, 1), 2);
        assert_eq!(naaz::query::unit_dice(&s, &k, CombatUnit { unit: Unit::CruiserI, ..mech }, 1), 1, "not a mech");
        assert_eq!(naaz::query::unit_dice(&s, &k, CombatUnit { system: None, ..mech }, 1), 1);
    }

    #[test]
    fn the_eidolon_maximum_forbids_mechs_and_is_immune() {
        let (mut s, k) = table(&[NAAZ, SOL]);
        use ti4_engine_core::generated::UnitBaseType::{Mech, Spacedock};
        assert!(!naaz::query::cannot_produce(&s, &k, Seat(0), Mech, Spacedock));
        s.board_mut().sys_mut(system_at(30)).units.push(Piece::new(Unit::EidolonMaximum, Seat(0)));
        assert!(naaz::query::cannot_produce(&s, &k, Seat(0), Mech, Spacedock));
        assert!(!naaz::query::cannot_produce(&s, &k, Seat(0), ti4_engine_core::generated::UnitBaseType::Infantry, Spacedock));
        assert!(!naaz::query::cannot_produce(&s, &k, Seat(1), Mech, Spacedock));
        assert!(naaz::query::effect_placement_forbidden(&s, &k, Seat(0), Unit::Eidolon));
        assert_eq!(naaz::query::captured_unit_return_form(&s, &k, Seat(0), Unit::EidolonMaximum), Unit::Eidolon);
        assert_eq!(naaz::query::captured_unit_return_form(&s, &k, Seat(0), Unit::CruiserI), Unit::CruiserI);
        assert!(naaz::query::ability_hit_immune(&s, &k, unit(0, Unit::EidolonMaximum, system_at(30), None, CombatCtx::Space)));
    }

    #[test]
    fn distant_suns_needs_a_mech_on_the_planet_and_pre_fab_readies_it() {
        let (mut s, k) = table(&[NAAZ, SOL]);
        let planet = Planet::Abaddon;
        assert!(!naaz::query::explore_extra_draw(&s, &k, Seat(0), planet));
        s.board_mut().planet_mut(planet).units.push(Piece::new(Unit::Eidolon, Seat(0)));
        assert!(naaz::query::explore_extra_draw(&s, &k, Seat(0), planet));
        assert!(!naaz::query::explore_extra_draw(&s, &k, Seat(1), planet));
        s.players_mut().edit(0).tech.owned.insert(Technology::PreFabArcologies);
        s.board_mut().planet_mut(planet).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
        let mut ask = FirstOption;
        naaz::observe::explored(&mut Cx::new(&mut s, &k, &mut ask), Seat(0), planet);
        assert!(!ti4_engine_core::query::board::exhausted(&s, planet));
    }

    #[test]
    fn naalu_hooks() {
        let (mut s, k) = table(&[NAALU, SOL]);
        let sys = system_at(30);
        s.meta_mut().rules_version = 17;
        s.board_mut().planet_mut(Planet::Abaddon).units.extend([Piece::new(Unit::Iconoclast, Seat(0)), Piece::new(Unit::InfantryI, Seat(1))]);
        let mech = unit(0, Unit::Iconoclast, sys, Some(Planet::Abaddon), CombatCtx::Ground);
        assert_eq!(naalu::query::unit_roll_modifier(&s, &k, mech), 0, "no fragment");
        s.players_mut().edit(1).cards.fragments.set(ti4_engine_core::generated::ExploreType::Cultural, 1);
        assert_eq!(naalu::query::unit_roll_modifier(&s, &k, mech), 2);
        s.meta_mut().rules_version = 16;
        assert_eq!(naalu::query::unit_roll_modifier(&s, &k, mech), 0, "below rules version 17");
        assert!(naalu::query::fighter_fleet_weight_halves(&s, &k, Seat(0), Unit::HybridCrystalFighterII));
        assert!(!naalu::query::fighter_fleet_weight_halves(&s, &k, Seat(1), Unit::HybridCrystalFighterII));
        assert!(!naalu::query::fighter_fleet_weight_halves(&s, &k, Seat(0), Unit::FighterII));
        // The Matriarch lets its owner's fighters join the invasion.
        s.board_mut().sys_mut(sys).units.extend([Piece::new(Unit::FighterI, Seat(0)), Piece::new(Unit::CarrierI, Seat(0))]);
        assert!(naalu::query::matriarch_fighters(&s, &k, Seat(0), sys).is_empty());
        s.board_mut().sys_mut(sys).units.push(Piece::new(Unit::Matriarch, Seat(0)));
        assert_eq!(naalu::query::matriarch_fighters(&s, &k, Seat(0), sys).as_slice(), &[Piece::new(Unit::FighterI, Seat(0))]);
        assert!(naalu::query::matriarch_temporary_ground_force(&s, &k, Seat(0), sys, Planet::Abaddon, Piece::new(Unit::FighterI, Seat(0))));
        assert!(!naalu::query::matriarch_temporary_ground_force(&s, &k, Seat(0), sys, Planet::Abaddon, Piece::new(Unit::CarrierI, Seat(0))));
    }

    #[test]
    fn mindsieve_offers_one_waiver_per_hidden_note_and_telepathic_goes_first() {
        let (mut s, k) = table(&[NAALU, SOL]);
        let ceasefire = NoteKey { note: PromissoryNote::Ceasefire, faction: SOL };
        s.cards_mut().promissory.edit().insert(ceasefire, Seat(0));
        assert!(naalu::query::secondary_waivers(&s, &k, Seat(0), Seat(1), None).is_empty(), "no breakthrough");
        s.players_mut().edit(0).cards.breakthrough = Some(Breakthrough::Mindsieve);
        let offered = naalu::query::secondary_waivers(&s, &k, Seat(0), Seat(1), None);
        assert_eq!(offered.as_slice(), &[Waiver { module: NAALU, code: naalu::query::mindsieve_code(PromissoryNote::Ceasefire, SOL) }]);
        assert!(naalu::query::secondary_waivers(&s, &k, Seat(0), Seat(0), None).is_empty(), "not on your own card");
        s.cards_mut().promissory_faceup.edit().insert(ceasefire, ());
        assert!(naalu::query::secondary_waivers(&s, &k, Seat(0), Seat(1), None).is_empty(), "a faceup note is not in hand");
        let mut ask = FirstOption;
        naalu::observe::strategy_phase_ended(&mut Cx::new(&mut s, &k, &mut ask));
        assert_eq!(s.strategy().initiative_overrides.get(&Seat(0)), Some(&0));
        assert_eq!(s.strategy().initiative_overrides.get(&Seat(1)), None);
    }

    #[test]
    fn sardakk_roll_shifts_and_the_commander() {
        let (mut s, k) = table(&[SARDAKK, SOL]);
        let sys = system_at(30);
        let cruiser = unit(0, Unit::CruiserI, sys, None, CombatCtx::Space);
        assert_eq!(sardakk::query::unit_roll_modifier(&s, &k, cruiser), 0);
        s.board_mut().sys_mut(sys).units.push(Piece::new(Unit::CmorranNorr, Seat(0)));
        assert_eq!(sardakk::query::unit_roll_modifier(&s, &k, cruiser), 1);
        assert_eq!(sardakk::query::unit_roll_modifier(&s, &k, unit(0, Unit::CmorranNorr, sys, None, CombatCtx::Space)), 0, "other ships only");
        // Tekklar Legion: +1 for the holder, -1 for the N'orr.
        let ground = |owner| unit(owner, Unit::InfantryI, sys, Some(Planet::Abaddon), CombatCtx::Ground);
        s.marks_mut().tail.edit().insert(
            MarkKey::SardakkTekklarCombat(),
            MarkVal::Rec(vec![
                MarkVal::Atom(Atom::System(sys)),
                MarkVal::Atom(Atom::Planet(Planet::Abaddon)),
                MarkVal::Atom(Atom::Seat(Seat(1))),
                MarkVal::Atom(Atom::Seat(Seat(0))),
            ]),
        );
        assert_eq!(sardakk::query::unit_roll_modifier(&s, &k, ground(1)), 1);
        assert_eq!(sardakk::query::unit_roll_modifier(&s, &k, ground(0)), -1);
        s.players_mut().edit(1).tech.owned.insert(Technology::ValkyrieParticleWeave);
        assert_eq!(sardakk::query::ground_rolls_extra_hits(&s, &k, Seat(1), sys, Planet::Abaddon, 0, 1), 1);
        assert_eq!(sardakk::query::ground_rolls_extra_hits(&s, &k, Seat(1), sys, Planet::Abaddon, 0, 0), 0);
        assert_eq!(sardakk::query::ground_rolls_extra_hits(&s, &k, Seat(0), sys, Planet::Abaddon, 0, 1), 0);
        assert_eq!(sardakk::query::commander_unlocked(&s, &k, Seat(0), Leader::Tro), None);
        assert_eq!(sardakk::query::commander_unlocked(&s, &k, Seat(0), Leader::GhomSekkus), Some(false));
    }

    #[test]
    fn coalescence_forces_the_marked_planet_and_slumberstate_draws_in_the_status_phase() {
        let (mut s, k) = table(&[TITANS, SOL]);
        let sys = ti4_engine_core::query::board::home_system(&s, Planet::Abaddon).unwrap_or_else(|| system_at(30));
        s.meta_mut().activation_seq = 5;
        s.marks_mut().tail.edit().insert(
            MarkKey::TitansCoalescence(Planet::Abaddon),
            MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(5)), MarkVal::Atom(Atom::System(sys))]),
        );
        assert!(titans::query::forced_combat_planets(&s, &k, Seat(0), sys).is_empty(), "no units there");
        s.board_mut().planet_mut(Planet::Abaddon).units.push(Piece::new(Unit::InfantryI, Seat(0)));
        assert_eq!(titans::query::forced_combat_planets(&s, &k, Seat(0), sys).as_slice(), &[Planet::Abaddon]);
        assert!(titans::query::forced_combat_planets(&s, &k, Seat(1), sys).is_empty());
        s.meta_mut().activation_seq = 6;
        assert!(titans::query::forced_combat_planets(&s, &k, Seat(0), sys).is_empty(), "another activation");
        assert_eq!(titans::query::slumberstate_draw_bonus(&s, &k, Seat(0), 1), 0, "not the status phase");
        s.meta_mut().phase = Phase::Status;
        assert_eq!(titans::query::slumberstate_draw_bonus(&s, &k, Seat(0), 1), 0, "no coexistence");
    }

    #[test]
    fn brother_omar_gives_a_green_prerequisite_and_a_waiver_paid_in_infantry() {
        let (mut s, k) = table(&[YIN, SOL]);
        let planet = Planet::Abaddon;
        assert!(yin::query::extra_prerequisite_colours(&s, &k, Seat(0)).is_empty());
        s.players_mut().edit(0).leaders.status.insert(Leader::BrotherOmar, LeaderStatus::Unlocked);
        let colours = yin::query::extra_prerequisite_colours(&s, &k, Seat(0));
        assert_eq!(colours.len(), 1);
        assert_eq!(yin::query::commander_unlocked(&s, &k, Seat(0), Leader::BrotherOmar), Some(false));
        let tech = Technology::ALL[0];
        assert!(yin::query::research_waiver_offer(&s, &k, Seat(0), tech).is_none(), "nobody else owns it");
        s.players_mut().edit(1).tech.owned.insert(tech);
        assert!(yin::query::research_waiver_offer(&s, &k, Seat(0), tech).is_none(), "no infantry to return");
        s.board_mut().planet_mut(planet).units.extend([Piece::new(Unit::InfantryI, Seat(0)), Piece::new(Unit::InfantryI, Seat(0))]);
        let offer = yin::query::research_waiver_offer(&s, &k, Seat(0), tech).expect("offered");
        assert_eq!(offer.payments.len(), 1);
        let mut ask = FirstOption;
        assert!(yin::offer::research_waiver_paid(&mut Cx::new(&mut s, &k, &mut ask), Seat(0), tech, offer.payments.as_slice()[0]));
        assert_eq!(ti4_engine_core::query::board::planet_units(&s, planet).len(), 1);
        assert!(!yin::offer::research_waiver_paid(&mut Cx::new(&mut s, &k, &mut ask), Seat(0), tech, Waiver { module: SOL, code: 1 }));
    }

    #[test]
    fn yssaril_crafty_scheming_and_the_deepgloom_marks() {
        let (mut s, k) = table(&[YSSARIL, SOL]);
        assert_eq!(yssaril::query::action_card_limit(&s, &k, Seat(0), 7), usize::MAX);
        assert_eq!(yssaril::query::action_card_limit(&s, &k, Seat(1), 7), 7);
        assert_eq!(yssaril::query::action_card_draw_bonus(&s, &k, Seat(0), 1), 1);
        assert_eq!(yssaril::query::action_card_draw_bonus(&s, &k, Seat(1), 1), 0);
        s.marks_mut().tail.edit().insert(MarkKey::PrivateBorrowedScheming(Seat(1)), MarkVal::Atom(Atom::Seat(Seat(0))));
        assert_eq!(yssaril::query::action_card_draw_bonus(&s, &k, Seat(1), 1), 1, "an allowed borrower");
        assert!(!yssaril::query::deepgloom_reach(&s, &k, Seat(0), Seat(1)));
        s.marks_mut().tail.edit().insert(MarkKey::YssarilBt(Word::Negotiating, Seat(1), Seat(0)), MarkVal::Atom(Atom::Word(Word::True)));
        assert!(yssaril::query::deepgloom_reach(&s, &k, Seat(0), Seat(1)));
        assert!(yssaril::query::deepgloom_reach(&s, &k, Seat(1), Seat(0)));
        s.meta_mut().phase = Phase::Action;
        s.meta_mut().turn_seq = 9;
        assert!(!yssaril::query::transaction_limit_exempt(&s, &k, Seat(0), Seat(1)));
        s.marks_mut().tail.edit().insert(MarkKey::YssarilBt(Word::Exempt, Seat(0), Seat(1)), MarkVal::Atom(Atom::Num(9)));
        assert!(yssaril::query::transaction_limit_exempt(&s, &k, Seat(1), Seat(0)));
        s.meta_mut().turn_seq = 10;
        assert!(!yssaril::query::transaction_limit_exempt(&s, &k, Seat(1), Seat(0)), "a stale turn");
        // Transparasteel Plating: a passed player cannot play action cards on its holder's turn.
        s.meta_mut().active = Some(Seat(0));
        s.players_mut().edit(0).tech.owned.insert(Technology::TransparasteelPlating);
        s.players_mut().edit(1).econ.passed = true;
        assert!(yssaril::query::action_cards_forbidden(&s, Seat(1)));
        assert!(!yssaril::query::action_cards_forbidden(&s, Seat(0)));
        assert_eq!(yssaril::query::commander_unlocked(&s, &k, Seat(0), Leader::SoAta), Some(false));
    }

    #[test]
    fn every_cd_module_registers_and_the_registrars_run_in_dispatch_order() {
        let mut t = ti4_engine_core::hooks::HookTable::new();
        for (_, register) in super::MODULES {
            register(&mut t);
        }
        assert!(t.registered() > 0);
        assert_eq!(super::MODULES.len(), 6);
        assert!(super::MODULES.windows(2).all(|w| w[0].0 < w[1].0), "dispatch order");
    }
}

/// The timing abilities of the package (the reaction windows), in the string registration order of the factions.
pub fn register_abilities(t: &mut ti4_engine_core::kernel::ability::AbilityTable) {
    yin::timing::register_abilities(t);
}
