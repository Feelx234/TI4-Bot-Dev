//! From `exploration.rs` of the legacy crate: what resolving an exploration card did, what the seat held before and after, and the words
//! of the log row (copies; the rules of exploring stay in the legacy crate).

use ti4_content::ContentStore;
use ti4_model::content_types::ContentType;
use ti4_model::id::PlanetId;

/// What resolving one exploration card did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Explored {
    /// A relic fragment, kept faceup until purged (35.9).
    Fragment { trait_name: String },
    /// An attachment on the planet.
    Attached { card: String },
    /// Drawn, but this engine has no handler for it. Announced, never silently dropped.
    Unresolved { card: String },
    /// An instant card this engine resolved.
    Resolved { card: String },
    /// An attachment drawn from the frontier, which has no planet to attach to.
    Discarded { card: String },
}

/// What a seat holds that an exploration card can change; compared before and after a card to say
/// what it did. Display only.
#[derive(Debug, PartialEq, Eq)]
pub struct Holdings {
    trade_goods: i32,
    commodities: i32,
    command_tokens: i32,
    action_cards: usize,
    secrets: usize,
    relics: usize,
    technologies: usize,
    play_area: usize,
    own_infantry: usize,
    own_mechs: usize,
    exhausted: bool,
}

impl Holdings {
    /// The typed core's reading as the string struct.
        pub fn from_core(h: ti4_engine_core::query::explore::Holdings) -> Self {
        Self {
            trade_goods: h.trade_goods,
            commodities: h.commodities,
            command_tokens: h.command_tokens,
            action_cards: h.action_cards,
            secrets: h.secrets,
            relics: h.relics,
            technologies: h.technologies,
            play_area: h.play_area,
            own_infantry: h.own_infantry,
            own_mechs: h.own_mechs,
            exhausted: h.exhausted,
        }
    }

    /// The signed change of each holding, for the exploration record's `gains`.
    fn gains(&self, after: &Self) -> std::collections::BTreeMap<String, i32> {
        let to_i32 = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);
        [
            ("trade_goods", after.trade_goods - self.trade_goods),
            ("commodities", after.commodities - self.commodities),
            ("command_tokens", after.command_tokens - self.command_tokens),
            (
                "action_cards",
                to_i32(after.action_cards) - to_i32(self.action_cards),
            ),
            (
                "secret_objectives",
                to_i32(after.secrets) - to_i32(self.secrets),
            ),
            ("relics", to_i32(after.relics) - to_i32(self.relics)),
            (
                "technologies",
                to_i32(after.technologies) - to_i32(self.technologies),
            ),
        ]
        .into_iter()
        .filter(|(_, change)| *change != 0)
        .map(|(key, change)| (key.to_owned(), change))
        .collect()
    }

    /// "gained 1 trade good, removed 1 infantry" -- empty when nothing the seat holds changed.
    fn describe_change(
        &self,
        after: &Self,
        planet: Option<&PlanetId>,
        outcome: &Explored,
    ) -> String {
        fn plural(n: i64, one: &str, many: &str) -> String {
            format!("{n} {}", if n == 1 { one } else { many })
        }
        let mut parts: Vec<String> = Vec::new();
        let delta = |a: i64, b: i64| b - a;
        let goods = delta(i64::from(self.trade_goods), i64::from(after.trade_goods));
        if goods > 0 {
            parts.push(format!(
                "gained {}",
                plural(goods, "trade good", "trade goods")
            ));
        } else if goods < 0 {
            parts.push(format!(
                "spent {}",
                plural(-goods, "trade good", "trade goods")
            ));
        }
        let commodities = delta(i64::from(self.commodities), i64::from(after.commodities));
        if commodities > 0 {
            parts.push(format!(
                "gained {}",
                plural(commodities, "commodity", "commodities")
            ));
        } else if commodities < 0 && goods <= 0 {
            // Converting commodities into trade goods is reported by the goods gained.
            parts.push(format!(
                "spent {}",
                plural(-commodities, "commodity", "commodities")
            ));
        }
        let tokens = delta(
            i64::from(self.command_tokens),
            i64::from(after.command_tokens),
        );
        if tokens > 0 {
            parts.push(format!(
                "gained {}",
                plural(tokens, "command token", "command tokens")
            ));
        }
        let cards = delta(self.action_cards as i64, after.action_cards as i64);
        if cards > 0 {
            parts.push(format!(
                "drew {}",
                plural(cards, "action card", "action cards")
            ));
        }
        if after.secrets > self.secrets {
            parts.push("drew a secret objective".to_owned());
        }
        if after.relics > self.relics {
            parts.push("gained a relic".to_owned());
        }
        if after.technologies > self.technologies {
            parts.push("gained a technology".to_owned());
        }
        if after.play_area > self.play_area {
            // Enigmatic Device: kept faceup, and its ACTION is offered from there.
            parts.push("placed it faceup in the play area (ACTION: spend 6 resources to research a technology)".to_owned());
        }
        let infantry = delta(self.own_infantry as i64, after.own_infantry as i64);
        if infantry < 0 {
            parts.push(format!(
                "removed {} from the planet",
                plural(-infantry, "infantry", "infantry")
            ));
        } else if infantry > 0 {
            parts.push(format!(
                "placed {} on the planet",
                plural(infantry, "infantry", "infantry")
            ));
        }
        if after.own_mechs > self.own_mechs {
            parts.push("placed a mech on the planet".to_owned());
        }
        if self.exhausted && !after.exhausted && planet.is_some() {
            parts.push("readied the planet".to_owned());
        }
        match outcome {
            Explored::Fragment { trait_name } => parts.insert(
                0,
                format!("took a {} relic fragment", trait_name.to_ascii_lowercase()),
            ),
            Explored::Unresolved { .. } => parts.insert(0, "could not be resolved".to_owned()),
            Explored::Discarded { .. } => {
                parts.insert(0, "discarded (nothing to attach to)".to_owned());
            }
            Explored::Attached { .. } | Explored::Resolved { .. } => {}
        }
        parts.join(", ")
    }
}

/// The words the log row of a drawn card is closed with (`explore_in`): the outcome, what the card did to the seat, what the seat
/// gained, and what was placed.
pub struct RowWords {
    pub text: String,
    pub result: String,
    pub gains: std::collections::BTreeMap<String, i32>,
    pub placed: Vec<String>,
}

/// [`RowWords`] from what the seat held before and after the card, the outcome, the wormhole and ion storm tokens the card placed
/// (in that order), the card it kept in its play area and the attachment it put on the planet. The exploration of a frontier token
/// by an action card closes its row with the same words (`action_cards_view`).
pub fn row_words(
    before: &Holdings,
    after: &Holdings,
    planet: Option<&PlanetId>,
    outcome: &Explored,
    tokens: Vec<String>,
    play_area: Option<&str>,
    attached: Option<&str>,
) -> RowWords {
    let text = match outcome {
        Explored::Fragment { .. } => "fragment".to_owned(),
        Explored::Attached { .. } => format!("attached:{}", attached.unwrap_or_default()),
        Explored::Unresolved { .. } => "unresolved".to_owned(),
        Explored::Resolved { .. } => "resolved".to_owned(),
        Explored::Discarded { .. } => "discarded".to_owned(),
    };
    let result = before.describe_change(after, planet, outcome);
    let gains = before.gains(after);
    let mut placed = tokens;
    if let Explored::Fragment { trait_name } = outcome {
        placed.push(format!("fragment:{}", trait_name.to_ascii_lowercase()));
    }
    if after.play_area > before.play_area
        && let Some(card) = play_area
    {
        placed.push(format!("play_area:{card}"));
    }
    if let (Explored::Attached { .. }, Some(planet)) = (outcome, planet)
        && let Some(id) = text.strip_prefix("attached:")
    {
        placed.push(format!("attachment:{id}:{planet}"));
    }
    RowWords { text, result, gains, placed }
}

/// The offer of the infantry question as the string engine decorates it: the card as printed, what is lost and
/// exactly what is gained. Display only; the core window's question is decorated by the same function.
pub fn pay_offer(
    content: &ContentStore,
    card: &str,
    planet: &PlanetId,
    system_name: &str,
    reward: &str,
) -> impl FnOnce(crate::wire::choice::Choice) -> crate::wire::choice::Choice {
    let record = content.get(ContentType::Explores, card);
    let name = record.and_then(|r| r.text("name")).unwrap_or(card).to_owned();
    let tag = format!(
        "{} exploration card",
        record.and_then(|r| r.text("type")).unwrap_or("").to_ascii_lowercase()
    );
    let printed = record.and_then(|r| r.text("text")).map(ToOwned::to_owned);
    let (planet, system_name, reward) = (planet.clone(), system_name.to_owned(), reward.to_owned());
    move |choice| {
        choice.offered(
            crate::wire::choice::offer_card(&name, &tag, Some("on exploring this planet"), printed.as_deref()),
            vec![
                crate::wire::choice::offer_fact_planet("Planet", planet.as_str(), &system_name),
                crate::wire::choice::offer_fact("You lose", "1 infantry on this planet"),
                crate::wire::choice::offer_fact("You gain", reward.as_str()),
            ],
            &[
                (
                    "remove_infantry",
                    crate::wire::choice::offer_caption(
                        &format!("Remove 1 infantry and {reward}"),
                        Some("the infantry is returned to your reinforcements"),
                    ),
                ),
                (
                    "decline",
                    crate::wire::choice::offer_caption("Keep the infantry", Some("the card has no effect")),
                ),
            ],
        )
    }
}
