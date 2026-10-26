//! The generated core enums and tables against the content store, through the generated adapter.
//!
//! The adapter output lives in `crates/ti4-engine/src/generated/` but `ti4-engine` does not compile
//! it yet (it does not depend on this placeholder crate), so it is included here.
//!
//! Per category: the ids of the corpus and of the enum agree in both directions; integer order is
//! byte order of the id for every pair; id -> variant -> id round-trips for every record; every
//! gameplay column and every display field agrees with what `ContentStore` (and its typed views)
//! read from the same record.

#[path = "../../ti4-engine/src/generated/mod.rs"]
#[allow(dead_code, unreachable_pub)]
mod adapter;

use std::collections::BTreeSet;
use std::fmt::Debug;

use adapter::{IdRef, IdStr};
use ti4_content::{ContentStore, Record};
use ti4_engine_core::generated::*;
use ti4_model::content_types::ContentType;

fn store() -> &'static ContentStore {
    ContentStore::embedded()
}

/// Ids both ways, order for every pair, round trip for every record.
fn ids_agree<T: IdStr + Ord + Debug>(category: ContentType) {
    let corpus: BTreeSet<&str> = store()
        .records(category)
        .iter()
        .filter_map(Record::id)
        .collect();
    let generated: BTreeSet<&str> = T::IDS.iter().copied().collect();
    assert_eq!(
        generated.len(),
        T::IDS.len(),
        "{category:?}: duplicate generated ids"
    );
    let missing: Vec<_> = corpus.difference(&generated).collect();
    let extra: Vec<_> = generated.difference(&corpus).collect();
    assert!(
        missing.is_empty(),
        "{category:?}: ids not in the enum: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "{category:?}: enum ids not in the corpus: {extra:?}"
    );
    order_and_round_trip::<T>(&format!("{category:?}"));
    for id in corpus {
        let variant = T::from_id(id).unwrap_or_else(|| panic!("{category:?}: {id:?} not found"));
        assert_eq!(variant.as_str(), id);
    }
}

fn order_and_round_trip<T: IdStr + Ord + Debug>(what: &str) {
    let n = T::IDS.len();
    assert!(T::nth(n).is_none(), "{what}: nth(COUNT) must be None");
    for i in 0..n {
        let a = T::nth(i).unwrap();
        assert_eq!(a.index(), i);
        assert_eq!(T::from_id(a.as_str()), Some(a), "{what}: {a:?}");
        for j in 0..n {
            let b = T::nth(j).unwrap();
            assert_eq!(
                a.cmp(&b),
                T::IDS[i].as_bytes().cmp(T::IDS[j].as_bytes()),
                "{what}: integer order of {a:?} and {b:?} is not the byte order of their ids"
            );
        }
    }
    assert!(T::from_id("no such id").is_none());
}

/// A display field compared with the record it came from, through the `Record` accessors.
trait SameAs {
    fn same_as(&self, record: &Record, field: &str) -> bool;
}
impl SameAs for &str {
    fn same_as(&self, record: &Record, field: &str) -> bool {
        record.text(field) == Some(*self)
    }
}
impl SameAs for Option<&str> {
    fn same_as(&self, record: &Record, field: &str) -> bool {
        record.text(field) == *self
    }
}
impl SameAs for &[&str] {
    fn same_as(&self, record: &Record, field: &str) -> bool {
        record.strings(field) == *self
    }
}
impl SameAs for bool {
    fn same_as(&self, record: &Record, field: &str) -> bool {
        record.flag(field) == *self
    }
}

macro_rules! text_agrees {
    ($record:expr, $text:expr, $($json:literal => $field:ident),+ $(,)?) => {
        $(assert!(
            $text.$field.same_as($record, $json),
            "{:?}: display field `{}` = {:?} differs from the record",
            $record.id(), $json, $text.$field
        );)+
    };
}

/// Every record of a category with its variant.
fn each<T: IdStr>(category: ContentType) -> impl Iterator<Item = (&'static Record, T)> {
    store()
        .records(category)
        .iter()
        .map(|r| (r, T::from_id(r.id().unwrap()).unwrap()))
}

fn source_of(record: &Record, source: Source) {
    assert_eq!(
        Some(source.as_str()),
        record.source().map(|s| s.to_string()).as_deref(),
        "{:?}",
        record.id()
    );
}

fn opt_str<T: IdStr>(value: Option<T>) -> Option<&'static str> {
    value.map(IdStr::as_str)
}

fn strs<T: IdStr>(values: &[T]) -> Vec<&'static str> {
    values.iter().map(|v| v.as_str()).collect()
}

fn int(record: &Record, field: &str) -> Option<i64> {
    record.int(field)
}

// ------------------------------------------------------------------ ids, order, round trip

#[test]
fn card_ids_agree_with_the_corpus() {
    ids_agree::<Ability>(ContentType::Abilities);
    ids_agree::<ActionCard>(ContentType::ActionCards);
    ids_agree::<Agenda>(ContentType::Agendas);
    ids_agree::<Attachment>(ContentType::Attachments);
    ids_agree::<Breakthrough>(ContentType::Breakthroughs);
    ids_agree::<Explore>(ContentType::Explores);
    ids_agree::<GalacticEvent>(ContentType::GalacticEvents);
    ids_agree::<GenericCard>(ContentType::GenericCards);
    ids_agree::<Leader>(ContentType::Leaders);
    ids_agree::<PromissoryNote>(ContentType::PromissoryNotes);
    ids_agree::<PublicObjective>(ContentType::PublicObjectives);
    ids_agree::<Relic>(ContentType::Relics);
    ids_agree::<SecretObjective>(ContentType::SecretObjectives);
    ids_agree::<StrategyCard>(ContentType::StrategyCards);
    ids_agree::<Technology>(ContentType::Technologies);
    ids_agree::<Token>(ContentType::Tokens);
}

#[test]
fn value_enums_are_sorted_and_round_trip() {
    order_and_round_trip::<Source>("Source");
    order_and_round_trip::<FactionTag>("FactionTag");
    order_and_round_trip::<TechType>("TechType");
    order_and_round_trip::<AgendaType>("AgendaType");
    order_and_round_trip::<AgendaElects>("AgendaElects");
    order_and_round_trip::<LeaderType>("LeaderType");
    order_and_round_trip::<LeaderWindow>("LeaderWindow");
    order_and_round_trip::<ObjectivePhase>("ObjectivePhase");
    order_and_round_trip::<ExploreType>("ExploreType");
    order_and_round_trip::<ExploreResolution>("ExploreResolution");
    order_and_round_trip::<StrategyCardKind>("StrategyCardKind");
    order_and_round_trip::<PlanetTrait>("PlanetTrait");
    order_and_round_trip::<Wormhole>("Wormhole");
}

#[test]
fn ids_repeated_across_categories_stay_apart() {
    // `crisis` is an agenda and an action card, `ps` a technology and (bare) a note alias.
    assert_eq!(Agenda::from_id("crisis").unwrap().as_str(), "crisis");
    assert_eq!(ActionCard::from_id("crisis").unwrap().as_str(), "crisis");
    assert_eq!(Technology::from_id("ps").unwrap().as_str(), "ps");
    assert!(PromissoryNote::from_id("ps").is_none());
    assert_eq!(Relic::from_id("nanoforge").unwrap().as_str(), "nanoforge");
    assert_eq!(
        Attachment::from_id("nanoforge").unwrap().as_str(),
        "nanoforge"
    );
}

#[test]
fn an_id_outside_the_enum_takes_the_dynamic_path() {
    let known = IdRef::<Technology>::parse("gd");
    assert_eq!(known.known(), Technology::from_id("gd"));
    assert_eq!(known.as_str(), "gd");
    let dynamic = IdRef::<PromissoryNote>::parse("support:sol");
    assert_eq!(dynamic, IdRef::Dynamic("support:sol"));
    assert_eq!(dynamic.as_str(), "support:sol");
    assert!(dynamic.known().is_none());
}

#[test]
fn the_tables_are_indexed_by_the_variant() {
    assert_eq!(ACTION_CARD_DATA.len(), ActionCard::COUNT);
    assert_eq!(ActionCard::ALL.len(), ActionCard::COUNT);
    for (i, card) in ActionCard::ALL.iter().enumerate() {
        assert_eq!(card.index(), i);
        assert_eq!(ActionCard::from_index(i), Some(*card));
        assert!(std::ptr::eq(card.data(), &ACTION_CARD_DATA[i]));
    }
    assert_eq!(ActionCard::from_index(ActionCard::COUNT), None);
    assert_eq!(std::mem::size_of::<ActionCard>(), 2);
    assert_eq!(std::mem::size_of::<Agenda>(), 1);
}

// ------------------------------------------------------------------ cards: data and text

#[test]
fn abilities_agree() {
    for (r, v) in each::<Ability>(ContentType::Abilities) {
        source_of(r, v.data().source);
        let t = adapter::abilities::text(v);
        text_agrees!(r, t, "name" => name, "faction" => faction, "window" => window,
            "windowEffect" => window_effect, "permanentEffect" => permanent_effect);
    }
}

#[test]
fn action_cards_agree() {
    for (r, v) in each::<ActionCard>(ContentType::ActionCards) {
        let d = v.data();
        source_of(r, d.source);
        assert_eq!(
            d.component_action,
            r.text("window").unwrap().trim() == "Action"
        );
        let t = adapter::action_cards::text(v);
        text_agrees!(r, t, "name" => name, "window" => window, "phase" => phase,
            "text" => text, "automationID" => automation_id);
    }
    // Copies are separate variants with the same text.
    let sabotage: Vec<_> = (1..=4)
        .map(|n| ActionCard::from_id(&format!("sabo{n}")).unwrap())
        .collect();
    assert!(
        sabotage
            .iter()
            .all(|c| adapter::action_cards::text(*c).name == "Sabotage")
    );
    assert_eq!(sabotage[0], ActionCard::Sabotage1);
}

#[test]
fn agendas_agree() {
    for (r, v) in each::<Agenda>(ContentType::Agendas) {
        let d = v.data();
        source_of(r, d.source);
        assert_eq!(Some(d.kind.as_str()), r.text("type"));
        // `vote::outcomes`.
        let target = r.text("target").unwrap();
        let head = target.split('(').next().unwrap().trim();
        match d.elects {
            AgendaElects::ForAgainst => assert!(!head.starts_with("Elect"), "{target}"),
            elects => assert_eq!(head, format!("Elect {}", elects.as_str())),
        }
        let t = adapter::agendas::text(v);
        text_agrees!(r, t, "name" => name, "target" => target, "text1" => text1,
            "text2" => text2, "mapText" => map_text);
    }
    assert_eq!(
        Agenda::CovertLegislation.data().elects,
        AgendaElects::ForAgainst
    );
    assert_eq!(
        Agenda::GalacticCrisisPact.data().elects,
        AgendaElects::StrategyCard
    );
}

#[test]
fn attachments_agree() {
    for (r, v) in each::<Attachment>(ContentType::Attachments) {
        let d = v.data();
        source_of(r, d.source);
        assert_eq!(d.resources.map(i64::from), int(r, "resourcesModifier"));
        assert_eq!(d.influence.map(i64::from), int(r, "influenceModifier"));
        let specialties: Vec<String> = r
            .strings("techSpeciality")
            .iter()
            .map(|s| s.to_ascii_uppercase())
            .collect();
        assert_eq!(strs(d.specialties), specialties);
        let traits: Vec<String> = r
            .strings("planetTypes")
            .iter()
            .map(|s| s.to_ascii_uppercase())
            .collect();
        assert_eq!(strs(d.traits), traits);
        assert_eq!(
            d.space_cannon.map(|c| i64::from(c.hits_on)),
            int(r, "spaceCannonHitsOn")
        );
        assert_eq!(
            d.space_cannon.map(|c| i64::from(c.dice)),
            int(r, "spaceCannonDieCount")
        );
        let t = adapter::attachments::text(v);
        text_agrees!(r, t, "name" => name, "isFakeAttachment" => is_fake_attachment);
    }
}

#[test]
fn breakthroughs_agree() {
    for (r, v) in each::<Breakthrough>(ContentType::Breakthroughs) {
        let d = v.data();
        source_of(r, d.source);
        assert_eq!(Some(d.faction.as_str()), r.text("faction"));
        let synergy: Vec<&str> = r
            .strings("synergy")
            .into_iter()
            .filter(|s| *s != "NONE")
            .collect();
        assert_eq!(strs(d.synergy), synergy);
        let t = adapter::breakthroughs::text(v);
        text_agrees!(r, t, "name" => name, "text" => text);
    }
}

#[test]
fn explores_agree() {
    for (r, v) in each::<Explore>(ContentType::Explores) {
        let d = v.data();
        source_of(r, d.source);
        assert_eq!(Some(d.deck.as_str()), r.text("type"));
        assert_eq!(Some(d.resolution.as_str()), r.text("resolution"));
        let target = opt_str(d.attachment).or(opt_str(d.token));
        assert_eq!(target, r.text("attachmentId"));
        assert!(d.attachment.is_none() || d.token.is_none());
        let t = adapter::explores::text(v);
        text_agrees!(r, t, "name" => name, "text" => text, "flavorText" => flavor_text);
    }
}

#[test]
fn galactic_events_and_generic_cards_agree() {
    for (r, v) in each::<GalacticEvent>(ContentType::GalacticEvents) {
        source_of(r, v.data().source);
        text_agrees!(r, adapter::galactic_events::text(v), "name" => name, "text" => text);
    }
    for (r, v) in each::<GenericCard>(ContentType::GenericCards) {
        source_of(r, v.data().source);
        text_agrees!(r, adapter::genericcards::text(v), "name" => name, "text" => text,
            "cardType" => card_type);
    }
}

#[test]
fn leaders_agree() {
    for (r, v) in each::<Leader>(ContentType::Leaders) {
        let d = v.data();
        source_of(r, d.source);
        assert_eq!(Some(d.faction.as_str()), r.text("faction"));
        assert_eq!(Some(d.kind.as_str()), r.text("type"));
        assert_eq!(opt_str(d.replaces), r.text("homebrewReplacesID"));
        // `leaders::uses_the_action` and `leaders::is_action_window`.
        let window = r.text("abilityWindow").unwrap();
        let printed = window.trim().trim_end_matches(':').trim_end();
        let uses_the_action = window.trim().starts_with("ACTION");
        let action_window = printed.starts_with("ACTION")
            || printed.eq_ignore_ascii_case("during the action phase");
        assert_eq!(
            d.window == LeaderWindow::Action,
            uses_the_action,
            "{window}"
        );
        assert_eq!(d.window != LeaderWindow::Other, action_window, "{window}");
        let t = adapter::leaders::text(v);
        text_agrees!(r, t, "name" => name, "title" => title, "abilityName" => ability_name,
            "abilityWindow" => ability_window, "abilityText" => ability_text,
            "unlockCondition" => unlock_condition);
    }
}

#[test]
fn promissory_notes_agree() {
    for (r, v) in each::<PromissoryNote>(ContentType::PromissoryNotes) {
        let d = v.data();
        source_of(r, d.source);
        assert_eq!(opt_str(d.faction), r.text("faction"));
        assert_eq!(d.play_area, r.flag("playArea"));
        text_agrees!(r, adapter::promissory_notes::text(v), "name" => name, "text" => text);
    }
}

#[test]
fn objectives_agree() {
    for (r, v) in each::<PublicObjective>(ContentType::PublicObjectives) {
        let d = v.data();
        source_of(r, d.source);
        assert_eq!(Some(i64::from(d.points)), r.int("points"));
        text_agrees!(r, adapter::public_objectives::text(v), "name" => name, "text" => text,
            "phase" => phase);
    }
    for (r, v) in each::<SecretObjective>(ContentType::SecretObjectives) {
        let d = v.data();
        source_of(r, d.source);
        assert_eq!(Some(i64::from(d.points)), r.int("points"));
        assert_eq!(Some(d.phase.as_str()), r.text("phase"));
        text_agrees!(r, adapter::secret_objectives::text(v), "name" => name, "text" => text);
    }
}

#[test]
fn relics_agree() {
    for (r, v) in each::<Relic>(ContentType::Relics) {
        let d = v.data();
        source_of(r, d.source);
        assert_eq!(d.fake, r.flag("isFakeRelic"));
        text_agrees!(r, adapter::relics::text(v), "name" => name, "text" => text);
    }
}

#[test]
fn strategy_cards_agree() {
    for (r, v) in each::<StrategyCard>(ContentType::StrategyCards) {
        let d = v.data();
        source_of(r, d.source);
        assert_eq!(Some(d.kind.as_str()), r.text("name"));
        assert_eq!(Some(i64::from(d.initiative)), r.int("initiative"));
        text_agrees!(r, adapter::strategy_cards::text(v), "name" => name,
            "primaryTexts" => primary_texts, "secondaryTexts" => secondary_texts);
    }
    assert_eq!(
        StrategyCard::DiplomacyBase.data().kind,
        StrategyCard::DiplomacyCodex.data().kind
    );
}

#[test]
fn technologies_agree() {
    for (r, v) in each::<Technology>(ContentType::Technologies) {
        let d = v.data();
        source_of(r, d.source);
        let types: Vec<&str> = r
            .strings("types")
            .into_iter()
            .filter(|s| *s != "NONE")
            .collect();
        assert_eq!(strs(d.types), types);
        let req = r
            .text("requirements")
            .filter(|s| !matches!(*s, "null" | "None"))
            .unwrap_or("");
        let count = |c: char| u8::try_from(req.chars().filter(|x| *x == c).count()).unwrap();
        let p = d.prerequisites;
        assert_eq!(
            (p.blue, p.green, p.red, p.yellow),
            (count('B'), count('G'), count('R'), count('Y'))
        );
        assert_eq!(usize::from(p.blue + p.green + p.red + p.yellow), req.len());
        assert_eq!(opt_str(d.faction), r.text("faction"));
        assert_eq!(opt_str(d.base_upgrade), r.text("baseUpgrade"));
        let blocked = r
            .text("text")
            .is_some_and(|t| t.to_ascii_lowercase().contains("cannot be researched"));
        assert_eq!(d.researchable, !blocked);
        text_agrees!(r, adapter::technologies::text(v), "name" => name, "text" => text);
    }
    let unresearchable: Vec<_> = Technology::ALL
        .iter()
        .filter(|t| !t.data().researchable)
        .map(|t| t.as_str())
        .collect();
    assert_eq!(unresearchable, ["parasite-obs", "planesplitter-obs"]);
}

#[test]
fn tokens_agree() {
    for (r, v) in each::<Token>(ContentType::Tokens) {
        let d = v.data();
        source_of(r, d.source);
        assert_eq!(strs(d.wormholes), r.strings("wormholes"));
        assert_eq!(d.space, r.text("spaceOrPlanet") == Some("space"));
        text_agrees!(r, adapter::tokens::text(v), "aliasList" => alias_list,
            "tokenPlanetName" => token_planet_name);
    }
}

// ------------------------------------------------------------------ factions, planets, systems, units

#[test]
fn other_ids_agree_with_the_corpus() {
    ids_agree::<Faction>(ContentType::Factions);
    ids_agree::<Planet>(ContentType::Planets);
    ids_agree::<System>(ContentType::Systems);
    ids_agree::<Unit>(ContentType::Units);
    order_and_round_trip::<TileBack>("TileBack");
    order_and_round_trip::<UnitBaseType>("UnitBaseType");
    // The typed catalogues of ti4-content see the same ids.
    let full = ti4_model::content_types::FULL;
    let planets: Vec<&str> = ti4_content::all_planets(store(), full)
        .into_keys()
        .collect();
    assert_eq!(planets, Planet::IDS);
    let systems: Vec<&str> = ti4_content::all_systems(store(), full)
        .into_keys()
        .collect();
    assert_eq!(systems, System::IDS);
    let units: Vec<&str> = ti4_content::unit_catalogue(store(), full)
        .keys()
        .copied()
        .collect();
    assert_eq!(units, Unit::IDS);
    let factions: Vec<&str> = ti4_content::factions::catalogue(store(), full)
        .into_keys()
        .collect();
    assert_eq!(factions, Faction::IDS);
}

#[test]
fn factions_agree() {
    for (r, v) in each::<Faction>(ContentType::Factions) {
        let d = v.data();
        source_of(r, d.source);
        let f = ti4_content::Faction::new(r);
        assert_eq!(opt_str(d.home_system), f.home_system());
        assert_eq!(strs(d.home_planets), f.home_planets());
        assert_eq!(i32::from(d.commodities), f.commodities());
        assert_eq!(strs(d.starting_tech), f.starting_tech());
        assert_eq!(strs(d.starting_tech_options), f.starting_tech_options());
        assert_eq!(d.starting_tech_amount, f.starting_tech_amount());
        assert_eq!(strs(d.faction_tech), f.faction_tech());
        assert_eq!(strs(d.abilities), f.abilities());
        assert_eq!(strs(d.leaders), f.leaders());
        assert_eq!(strs(d.promissory_notes), f.promissory_notes());
        assert_eq!(strs(d.units), r.strings("units"));
        let t = adapter::factions::text(v);
        assert_eq!(Some(t.faction_name), f.name());
        assert_eq!(t.starting_fleet, f.starting_fleet());
        text_agrees!(r, t, "factionName" => faction_name, "shortName" => short_name,
            "shortTag" => short_tag, "complexity" => complexity, "startingFleet" => starting_fleet);
    }
}

#[test]
fn faction_tags_name_their_factions() {
    for tag in FactionTag::ALL {
        match Faction::from_id(tag.as_str()) {
            Some(faction) => assert_eq!(tag.factions(), [faction]),
            None => assert!(
                tag.factions()
                    .iter()
                    .all(|f| f.as_str().starts_with(tag.as_str()))
            ),
        }
    }
    let keleres: Vec<&str> = strs(FactionTag::Keleres.factions());
    assert_eq!(keleres, ["keleresa", "keleresm", "keleresx"]);
    assert!(
        FactionTag::Ghemina.factions().is_empty(),
        "a homebrew leftover: no faction record"
    );
}

#[test]
fn planets_agree() {
    let full = ti4_model::content_types::FULL;
    for (r, v) in each::<Planet>(ContentType::Planets) {
        let d = v.data();
        source_of(r, d.source);
        let p = ti4_content::Planet::new(r);
        assert_eq!(i64::from(d.resources), p.resources());
        assert_eq!(i64::from(d.influence), p.influence());
        let traits: Vec<String> = p
            .planet_types()
            .iter()
            .map(|t| t.to_ascii_uppercase())
            .collect();
        assert_eq!(strs(d.traits), traits);
        let specialties: Vec<&str> = p
            .tech_specialties()
            .into_iter()
            .filter(|s| *s != "NONE")
            .collect();
        assert_eq!(strs(d.specialties), specialties);
        assert_eq!(d.legendary, p.is_legendary());
        assert_eq!(opt_str(d.system), p.system_id());
        assert_eq!(opt_str(d.homeworld_of), p.homeworld_of());
        assert_eq!(
            d.space_cannon.map(|c| i64::from(c.hits_on)),
            int(r, "spaceCannonHitsOn")
        );
        assert_eq!(
            ti4_content::galaxy::planet(store(), v.as_str(), full).map(|p| p.id()),
            Some(v.as_str())
        );
        let t = adapter::planets::text(v);
        assert_eq!(Some(t.name), p.name());
        text_agrees!(r, t, "name" => name, "shortName" => short_name,
            "legendaryAbilityName" => legendary_ability_name,
            "legendaryAbilityText" => legendary_ability_text, "flavourText" => flavour_text,
            "aliases" => aliases);
    }
}

#[test]
fn systems_agree() {
    let full = ti4_model::content_types::FULL;
    let homes = ti4_content::galaxy::home_systems(store(), full);
    for (r, v) in each::<System>(ContentType::Systems) {
        let d = v.data();
        source_of(r, d.source);
        let s = ti4_content::System::new(r);
        assert_eq!(strs(d.planets), s.planets());
        let wormholes: BTreeSet<&str> = strs(d.wormholes).into_iter().collect();
        assert_eq!(wormholes, s.wormholes());
        let a = d.anomalies;
        assert_eq!(a.asteroid_field, s.is_asteroid_field());
        assert_eq!(a.gravity_rift, s.is_gravity_rift());
        assert_eq!(a.nebula, s.is_nebula());
        assert_eq!(a.supernova, s.is_supernova());
        assert_eq!(a.scar, s.is_scar());
        assert_eq!(a.any(), s.is_anomaly());
        assert_eq!(d.hyperlane, s.is_hyperlane());
        assert_eq!(opt_str(d.tile_back), r.text("tileBack"));
        assert_eq!(
            d.home_system,
            ti4_content::galaxy::is_home_system(store(), v.as_str(), full)
        );
        assert_eq!(d.home_system, homes.contains(v.as_str()));
        let t = adapter::systems::text(v);
        assert_eq!(Some(t.name), s.name());
        text_agrees!(r, t, "name" => name, "aliases" => aliases);
    }
}

#[test]
fn units_agree() {
    let full = ti4_model::content_types::FULL;
    let catalogue = ti4_content::unit_catalogue(store(), full);
    for (r, v) in each::<Unit>(ContentType::Units) {
        let d = v.data();
        source_of(r, d.source);
        let u = catalogue[v.as_str()];
        assert_eq!(d.base_type.as_str(), u.base_type());
        assert_eq!(opt_str(d.faction), u.faction());
        assert!((f64::from(d.cost_halves.unwrap_or(0)) / 2.0 - u.cost()).abs() < 1e-9);
        assert_eq!(i64::from(d.movement.unwrap_or(0)), u.move_value());
        assert_eq!(i64::from(d.capacity.unwrap_or(0)), u.capacity());
        assert_eq!(i64::from(d.capacity_used.unwrap_or(0)), u.capacity_cost());
        let roll = |r: Option<Roll>| r.map(|r| i64::from(r.hits_on));
        let dice = |r: Option<Roll>, default: i64| r.map_or(default, |r| i64::from(r.dice));
        assert_eq!(roll(d.combat), u.combat_hits_on());
        assert_eq!(dice(d.combat, u.combat_dice()), u.combat_dice());
        assert_eq!(roll(d.anti_fighter_barrage), u.afb_hits_on());
        assert_eq!(dice(d.anti_fighter_barrage, u.afb_dice()), u.afb_dice());
        assert_eq!(roll(d.bombardment), u.bombard_hits_on());
        assert_eq!(dice(d.bombardment, u.bombard_dice()), u.bombard_dice());
        assert_eq!(roll(d.space_cannon), u.space_cannon_hits_on());
        assert_eq!(
            dice(d.space_cannon, u.space_cannon_dice()),
            u.space_cannon_dice()
        );
        assert_eq!(d.production.is_some(), u.has_production());
        for resources in [0, 3] {
            let generated = d.production.map_or(0, |p| {
                i64::from(p.value)
                    + if p.plus_planet_resources {
                        resources
                    } else {
                        0
                    }
            });
            assert_eq!(generated, u.production(resources), "{v:?}");
        }
        assert_eq!(d.ship, u.is_ship());
        assert_eq!(d.structure, u.is_structure());
        assert_eq!(d.structure && d.space_only, u.is_space_only_structure());
        assert_eq!(d.ground_force, r.flag("isGroundForce"));
        assert_eq!(d.sustain_damage, u.sustain_damage());
        assert_eq!(d.planetary_shield, u.planetary_shield());
        assert_eq!(d.can_be_direct_hit, u.can_be_direct_hit());
        assert_eq!(d.deep_space_cannon, r.flag("deepSpaceCannon"));
        assert_eq!(
            d.disables_planetary_shield,
            r.flag("disablesPlanetaryShield")
        );
        assert_eq!(opt_str(d.upgrades_to), u.upgrades_to());
        assert_eq!(opt_str(d.upgrades_from), u.upgrades_from());
        match d.required_technology {
            Requirement::None => assert_eq!(u.required_technology(), None),
            Requirement::Known(t) => assert_eq!(u.required_technology(), Some(t.as_str())),
            Requirement::NotInCorpus => {
                let id = u.required_technology().unwrap();
                assert!(store().get(ContentType::Technologies, id).is_none());
            }
        }
        assert_eq!(opt_str(d.replaces), r.text("homebrewReplacesID"));
        let t = adapter::units::text(v);
        assert_eq!(t.async_id, u.async_id());
        assert_eq!(t.ability, u.ability_text());
        text_agrees!(r, t, "name" => name, "subtitle" => subtitle, "ability" => ability,
            "asyncId" => async_id);
    }
}
