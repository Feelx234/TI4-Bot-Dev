//! String formatters for human-readable actions and options.

/// Formats an engine option ID into a human-readable action description.
#[must_use]
pub fn format_action_id(option_id: &str) -> String {
    let trimmed = option_id.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(card) = trimmed.strip_prefix("strategic|") {
        let name = format_strategy_card_name(card);
        return format!("Play Strategy Card: {name}");
    }

    if trimmed.starts_with("pok") || is_strategy_card_id(trimmed) {
        let name = format_strategy_card_name(trimmed);
        return format!("Strategy Card: {name}");
    }

    if let Some(sys) = trimmed.strip_prefix("tactical|") {
        return format!("Tactical Action (System {sys})");
    }

    if let Some(component) = trimmed.strip_prefix("component|") {
        return format!("Component Action: {}", humanize_id(component));
    }

    match trimmed {
        "pass" => "Pass Turn".to_owned(),
        "generic" => "Confirm Selection".to_owned(),
        "top" => "Place on Top of Deck".to_owned(),
        "bottom" => "Place on Bottom of Deck".to_owned(),
        _ => {
            if let Some(unit) = trimmed.strip_prefix("produce|") {
                format!("Produce {}", humanize_id(unit))
            } else if let Some(unit) = trimmed.strip_prefix("produce_unit|") {
                format!("Produce {}", humanize_id(unit))
            } else {
                humanize_id(trimmed)
            }
        }
    }
}

fn is_strategy_card_id(id: &str) -> bool {
    matches!(
        id.to_lowercase().as_str(),
        "pok1leadership"
            | "pok2diplomacy"
            | "pok3politics"
            | "pok4construction"
            | "pok5trade"
            | "pok6warfare"
            | "pok7technology"
            | "pok8imperial"
            | "leadership"
            | "diplomacy"
            | "politics"
            | "construction"
            | "trade"
            | "warfare"
            | "technology"
            | "imperial"
    )
}

fn format_strategy_card_name(id: &str) -> String {
    match id.to_lowercase().as_str() {
        "pok1leadership" | "leadership" => "1. Leadership".to_owned(),
        "pok2diplomacy" | "diplomacy" => "2. Diplomacy".to_owned(),
        "pok3politics" | "politics" => "3. Politics".to_owned(),
        "pok4construction" | "construction" => "4. Construction".to_owned(),
        "pok5trade" | "trade" => "5. Trade".to_owned(),
        "pok6warfare" | "warfare" => "6. Warfare".to_owned(),
        "pok7technology" | "technology" => "7. Technology".to_owned(),
        "pok8imperial" | "imperial" => "8. Imperial".to_owned(),
        _ => humanize_id(id),
    }
}

fn humanize_id(id: &str) -> String {
    let mut words = Vec::new();
    for word in id.split('_') {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            let capitalized = format!("{}{}", first.to_uppercase(), chars.as_str());
            words.push(capitalized);
        }
    }
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_actions_consistently() {
        assert_eq!(
            format_action_id("pok1leadership"),
            "Strategy Card: 1. Leadership"
        );
        assert_eq!(
            format_action_id("strategic|pok3politics"),
            "Play Strategy Card: 3. Politics"
        );
        assert_eq!(format_action_id("pass"), "Pass Turn");
        assert_eq!(format_action_id("generic"), "Confirm Selection");
        assert_eq!(
            format_action_id("tactical|18"),
            "Tactical Action (System 18)"
        );
        assert_eq!(format_action_id("top"), "Place on Top of Deck");
    }
}
