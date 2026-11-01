//! From `transactions.rs` of the legacy crate (copies).

/// The constants live in `crate::kinds` (lane B); this module re-exports them.
pub use crate::kinds::transactions::{ANSWER_KIND, FREE_FORM_PREFIX, OFFER_KIND, OPEN_KIND};

use ti4_content::ContentStore;
use ti4_model::content_types::DEFAULT;
use std::collections::BTreeSet;

use ti4_model::id::{ActionCardId, PlanetId, PlayerId, SecretObjectiveId};


/// The id prefix of a free-form deal: the proposer names both sides themselves instead of picking
/// one of the listed shapes (94.1: a transaction is any exchange of trade goods, commodities,
/// promissory notes and relic fragments, in any amounts the two seats hold).


/// What a planet card is worth to hold: its resources and influence.
pub fn planet_value(content: &ContentStore, planet: &PlanetId) -> f64 {
    ti4_content::galaxy::planet(content, planet.as_str(), DEFAULT).map_or(2.0, |record| {
        #[allow(clippy::cast_precision_loss)]
        let value = (record.resources() + record.influence()) as f64;
        value.max(1.0)
    })
}


/// The free-form id of a deal: `x;` then `g…` fields for what the proposer gives and `r…` fields
/// for what they receive. The inverse of [`offer_from`] for such ids.
#[must_use]
pub fn free_form_id(given: &Terms, received: &Terms) -> String {
    let mut id = FREE_FORM_PREFIX.to_owned();
    push_fields(&mut id, 'g', given);
    push_fields(&mut id, 'r', received);
    id
}

/// The key a relic fragment is held under: its trait upper-cased, as exploration stores it
/// (`CULTURAL`), with the diplomacy asset's `unknown` meaning the frontier deck's `FRONTIER`.
///
/// Diplomacy transfers name fragments in lower case and the legacy window in the stored case, and
/// comparing them exactly made every diplomacy deal with a fragment "cannot pay what they offered".
#[must_use]
pub fn fragment_key(trait_name: &str) -> String {
    let upper = trait_name.to_ascii_uppercase();
    if upper == "UNKNOWN" {
        "FRONTIER".to_owned()
    } else {
        upper
    }
}

/// What one side of a deal hands over.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Terms {
    pub trade_goods: i32,
    pub commodities: i32,
    /// Relic fragments by trait, one entry per fragment.
    pub fragments: Vec<String>,
    /// A promissory note, by id.
    ///
    /// A note is a loan rather than a sale — every one of them says "then, return this card" —
    /// which is why what it costs to part with is not what it is worth to receive.
    pub promissory: Option<String>,
    /// An action card, by alias. 94.3 forbids exchanging them; Hacan's Arbiters is the
    /// exception, so naming one here is legal only for a table where somebody has that ability.
    pub action_card: Option<ActionCardId>,
    /// An unscored secret objective, by alias. Black Market Dealings is the one card that lets
    /// these change hands at the table; like an action card its value is entirely situational,
    /// so it prices flat in both directions.
    pub secret: Option<SecretObjectiveId>,
    /// A planet card (rules version 17), tradable only through the Hacan mech Pride of Kenara.
    pub planet: Option<PlanetTerm>,
}

/// A planet card handed over with the Pride of Kenara: "This planet's card may be traded as part of
/// a transaction; if you do, move all of your units from this planet to another planet you control."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetTerm {
    /// The planet whose card changes hands.
    pub planet: PlanetId,
    /// The giver's other planet its units move to.
    pub moves_to: PlanetId,
}

/// A proposed deal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub proposer: PlayerId,
    pub partner: PlayerId,
    pub given: Terms,
    pub received: Terms,
}

/// A side's terms as the fields of a free-form id, `{side}t=2;{side}c=1;{side}n=note;...`.
pub fn push_fields(out: &mut String, side: char, terms: &Terms) {
    use std::fmt::Write as _;
    if terms.trade_goods > 0 {
        let _ = write!(out, "{side}t={};", terms.trade_goods);
    }
    if terms.commodities > 0 {
        let _ = write!(out, "{side}c={};", terms.commodities);
    }
    if let Some(note) = &terms.promissory {
        let _ = write!(out, "{side}n={note};");
    }
    if let Some(card) = &terms.action_card {
        let _ = write!(out, "{side}a={card};");
    }
    if let Some(secret) = &terms.secret {
        let _ = write!(out, "{side}s={secret};");
    }
    if !terms.fragments.is_empty() {
        let _ = write!(out, "{side}f={};", terms.fragments.join(","));
    }
    if let Some(term) = &terms.planet {
        let _ = write!(out, "{side}p={}>{};", term.planet, term.moves_to);
    }
}

/// [`offer_from`] knowing the faction names of the two chairs instead of the state (the typed renderer reads a
/// deal back from its id this way).
#[must_use]
pub fn offer_from_with(
    faction: &dyn Fn(&PlayerId) -> String,
    id: &str,
    proposer: &PlayerId,
    partner: &PlayerId,
) -> Option<Offer> {
    let goods = |n: i32| Terms {
        trade_goods: n,
        ..Terms::default()
    };
    let commodities = |n: i32| Terms {
        commodities: n,
        ..Terms::default()
    };
    let deal = |given: Terms, received: Terms| {
        Some(Offer {
            proposer: proposer.clone(),
            partner: partner.clone(),
            given,
            received,
        })
    };

    if id.starts_with(FREE_FORM_PREFIX) {
        return free_form_offer(id, proposer, partner);
    }
    if id == "ss" {
        let from = faction(proposer);
        let to = faction(partner);
        return deal(
            Terms {
                promissory: Some(super::promissory::support(&from)),
                ..Terms::default()
            },
            Terms {
                promissory: Some(super::promissory::support(&to)),
                ..Terms::default()
            },
        );
    }
    if let Some(rest) = id.strip_prefix("ac") {
        // `ac{card}:{price}` — card aliases never carry colons, so this splits cleanly. An
        // unpriced form parses to no deal rather than inventing a price; the oracle would raise.
        let (card, price) = rest.split_once(':')?;
        return deal(
            Terms {
                action_card: Some(ActionCardId::new(card)),
                ..Terms::default()
            },
            goods(price.parse().ok()?),
        );
    }
    if let Some(rest) = id.strip_prefix("pn") {
        // `pn{note}:{price}` — the note id itself carries a colon (alias:faction), so the price
        // is whatever follows its *last* one. An unpriced legacy form parses to no deal rather
        // than inventing a price; the oracle would raise on it.
        let (note, price) = rest.rsplit_once(':')?;
        return deal(
            Terms {
                promissory: Some(note.to_owned()),
                ..Terms::default()
            },
            goods(price.parse().ok()?),
        );
    }
    if let Some(rest) = id.strip_prefix("kp").or_else(|| id.strip_prefix("kq")) {
        // `kp{planet}>{to}:{price}` (mine) or `kq{planet}>{to}:{price}` (theirs): a Pride of
        // Kenara planet card for trade goods.
        let (cards, price) = rest.rsplit_once(':')?;
        let (planet, to) = cards.split_once('>')?;
        let term = Terms {
            planet: Some(PlanetTerm {
                planet: PlanetId::new(planet),
                moves_to: PlanetId::new(to),
            }),
            ..Terms::default()
        };
        let price = goods(price.parse().ok()?);
        return if id.starts_with("kp") {
            deal(term, price)
        } else {
            deal(price, term)
        };
    }
    // The asks that take something from the partner: `np`, `cp`, `pc`, `nn`, `cn`. Delegated rather
    // than inline because a shape this file cannot read has to fall through to the commodity and
    // goods branches untouched, and `offer_from` was already at the edge of being readable.
    if let Some(ask) = ask_terms(id, proposer, partner) {
        return Some(ask);
    }
    if let Some(rest) = id.strip_prefix("so") {
        // `so{secret}:{price}` — Black Market Dealings puts an unscored secret objective on
        // the table. An alias never carries a colon, so the split is clean; an unpriced form
        // parses to no deal rather than inventing a price.
        let (secret, price) = rest.split_once(':')?;
        return deal(
            Terms {
                secret: Some(SecretObjectiveId::new(secret)),
                ..Terms::default()
            },
            goods(price.parse().ok()?),
        );
    }
    if let Some(rest) = id.strip_prefix("fr") {
        // `fr{trait}:{price}` — a Black Market relic fragment, one per entry in the terms.
        let (trait_name, price) = rest.split_once(':')?;
        return deal(
            Terms {
                fragments: vec![trait_name.to_owned()],
                ..Terms::default()
            },
            goods(price.parse().ok()?),
        );
    }
    if let Some(rest) = id.strip_prefix("cc") {
        let many = rest.parse().ok()?;
        return deal(commodities(many), commodities(many));
    }
    if let Some(rest) = id.strip_prefix("ct") {
        let (give, want) = pair(rest)?;
        return deal(commodities(give), goods(want));
    }
    if let Some(rest) = id.strip_prefix("tc") {
        let (give, want) = pair(rest)?;
        return deal(goods(give), commodities(want));
    }
    if let Some(rest) = id.strip_prefix('c') {
        let (give, _) = pair(rest)?;
        return deal(commodities(give), Terms::default());
    }
    let (give, want) = pair(id)?;
    deal(goods(give), goods(want))
}

/// The asks that take something *from* the partner, as generated by [`partner_assets`].
///
/// `None` for every id this is not, so a caller tries it in the middle of a prefix chain and falls
/// through. Note ids contain `:` themselves (`ra:jolnar`), so a price is whatever follows the
/// *last* colon; the two-asset shapes are split on `>` because neither `:` nor `|` can divide a
/// pair of note ids.
pub fn ask_terms(id: &str, proposer: &PlayerId, partner: &PlayerId) -> Option<Offer> {
    let goods = |n: i32| Terms {
        trade_goods: n,
        ..Terms::default()
    };
    let commodities = |n: i32| Terms {
        commodities: n,
        ..Terms::default()
    };
    let deal = |given: Terms, received: Terms| {
        Some(Offer {
            proposer: proposer.clone(),
            partner: partner.clone(),
            given,
            received,
        })
    };

    if let Some(rest) = id.strip_prefix("np") {
        // `np{note}:{price}` — note ids carry a colon, so the price is after the last one.
        let (note, price) = rest.rsplit_once(':')?;
        return deal(
            goods(price.parse().ok()?),
            Terms {
                promissory: Some(note.to_owned()),
                ..Terms::default()
            },
        );
    }
    if let Some(rest) = id.strip_prefix("cp") {
        let (note, price) = rest.rsplit_once(':')?;
        return deal(
            commodities(price.parse().ok()?),
            Terms {
                promissory: Some(note.to_owned()),
                ..Terms::default()
            },
        );
    }
    if let Some(rest) = id.strip_prefix("pc") {
        // `pc{mine}:{price}` — my note, for their commodities.
        let (note, price) = rest.rsplit_once(':')?;
        return deal(
            Terms {
                promissory: Some(note.to_owned()),
                ..Terms::default()
            },
            commodities(price.parse().ok()?),
        );
    }
    if let Some(rest) = id.strip_prefix("nn") {
        // `nn{mine}>{theirs}`. Neither half can be a `|` (option-token separator) and a `:` cannot
        // divide them, because both halves are note ids.
        let (owed, want) = rest.split_once('>')?;
        return deal(
            Terms {
                promissory: Some(owed.to_owned()),
                ..Terms::default()
            },
            Terms {
                promissory: Some(want.to_owned()),
                ..Terms::default()
            },
        );
    }
    if let Some(rest) = id.strip_prefix("cn") {
        let (card, note) = rest.split_once('>')?;
        return deal(
            Terms {
                action_card: Some(ActionCardId::new(card)),
                ..Terms::default()
            },
            Terms {
                promissory: Some(note.to_owned()),
                ..Terms::default()
            },
        );
    }
    None
}

/// Split a `"{a}:{b}"` pair of numbers.
pub fn pair(text: &str) -> Option<(i32, i32)> {
    let (left, right) = text.split_once(':')?;
    Some((left.parse().ok()?, right.parse().ok()?))
}

/// Read a free-form id. `None` for anything malformed (unknown or repeated field, a count that is
/// not a plain non-negative number) rather than a guess; legality is `why_illegal`'s business.
pub fn free_form_offer(id: &str, proposer: &PlayerId, partner: &PlayerId) -> Option<Offer> {
    let mut given = Terms::default();
    let mut received = Terms::default();
    let mut seen = BTreeSet::new();
    for field in id.strip_prefix(FREE_FORM_PREFIX)?.split(';') {
        if field.is_empty() {
            continue;
        }
        let (key, value) = field.split_once('=')?;
        if !seen.insert(key.to_owned()) || value.is_empty() {
            return None;
        }
        let mut chars = key.chars();
        let (side, kind) = (chars.next()?, chars.next()?);
        if chars.next().is_some() {
            return None;
        }
        let terms = match side {
            'g' => &mut given,
            'r' => &mut received,
            _ => return None,
        };
        let count = || {
            // Plain digits only: "+2" and "-2" parse as numbers but are not amounts.
            (value.len() <= 4 && value.bytes().all(|b| b.is_ascii_digit()))
                .then(|| value.parse::<i32>().ok())
                .flatten()
        };
        match kind {
            't' => terms.trade_goods = count()?,
            'c' => terms.commodities = count()?,
            'n' => terms.promissory = Some(value.to_owned()),
            'a' => terms.action_card = Some(ActionCardId::new(value)),
            's' => terms.secret = Some(SecretObjectiveId::new(value)),
            'f' => terms.fragments = value.split(',').map(str::to_owned).collect(),
            'p' => {
                let (planet, to) = value.split_once('>')?;
                terms.planet = Some(PlanetTerm {
                    planet: PlanetId::new(planet),
                    moves_to: PlanetId::new(to),
                });
            }
            _ => return None,
        }
    }
    Some(Offer {
        proposer: proposer.clone(),
        partner: partner.clone(),
        given,
        received,
    })
}

impl Terms {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.trade_goods == 0
            && self.commodities == 0
            && self.fragments.is_empty()
            && self.promissory.is_none()
            && self.action_card.is_none()
            && self.secret.is_none()
            && self.planet.is_none()
    }

    /// How these terms read in a prompt — the oracle's `Terms.describe`, same parts, order and
    /// words. An empty side reads as "nothing", which is how a gift stays readable.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.trade_goods > 0 {
            parts.push(format!("{} trade goods", self.trade_goods));
        }
        if self.commodities > 0 {
            parts.push(format!("{} commodities", self.commodities));
        }
        if !self.fragments.is_empty() {
            parts.push(format!("{} relic fragments", self.fragments.len()));
        }
        if let Some(note) = &self.promissory {
            parts.push(note.clone());
        }
        if let Some(card) = &self.action_card {
            parts.push(format!("the action card {card}"));
        }
        if let Some(secret) = &self.secret {
            parts.push(format!("the secret objective {secret}"));
        }
        if let Some(term) = &self.planet {
            parts.push(format!(
                "the planet card {} (its units move to {})",
                term.planet, term.moves_to
            ));
        }
        if parts.is_empty() {
            "nothing".to_owned()
        } else {
            parts.join(", ")
        }
    }
}

impl Offer {
    /// [`Self::describe`] with the speaker's faction name given (the renderer has no state).
    #[must_use]
    pub fn describe_as(&self, speaker: &str) -> String {
        format!("{speaker} gives {} for {}", self.given.describe(), self.received.describe())
    }
}
