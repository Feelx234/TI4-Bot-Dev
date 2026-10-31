//! The typed action menu and closing menu against the string's (package D1b2; `Game::{turn_options, closing_options}`): the
//! core assembles the menu from the options of its sources (`driver::menu::{action_menu, closing_choice}`, typed
//! `choice::turn::Opt`s) and `render/turn.rs` renders it; the rendering is compared with the `Choice` the string built (prompt,
//! option ids, kinds and labels, context, hashes: `compare_choice`). The options of the sources that are not ported are LENT:
//! they are typed as `Opt::Lent(code)` and the string's own option stands in the rendering (D1b2-bail-04), so what the
//! comparison checks is the order and filtering, the native options (pass, tactical, strategic, end turn) and the question.
//! The display `details` (`with_turn_menu`) are added after this point and are not compared here.

use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::choice::Choice;
use crate::turn_write::{ClosingEntry, MenuParts};

/// `Game::turn_options`: `parts` the options each source gave, `failed` the component actions that failed this turn, `string`
/// the question the string built.
pub fn check_action_menu(state: &GameState, sources: SourceSet, parts: &MenuParts, thunders_edge: bool, failed: &[String], string: &Choice) {
    real::check_action_menu(state, sources, parts, thunders_edge, failed, string);
}

/// `Game::closing_options`: `turn` the options of the action menu with their free and pass flags, `expedition` the Thunder's
/// Edge options appended, `second` whether a second action is allowed.
pub fn check_closing_menu(state: &GameState, sources: SourceSet, player: &PlayerId, second: bool, turn: &[ClosingEntry], expedition: &[String], string: &Choice) {
    real::check_closing_menu(state, sources, player, second, turn, expedition, string);
}

#[cfg(not(feature = "typed-core"))]
mod real {
    use super::{Choice, ClosingEntry, GameState, MenuParts, PlayerId, SourceSet};

    pub fn check_action_menu(_: &GameState, _: SourceSet, _: &MenuParts, _: bool, _: &[String], _: &Choice) {}
    pub fn check_closing_menu(_: &GameState, _: SourceSet, _: &PlayerId, _: bool, _: &[ClosingEntry], _: &[String], _: &Choice) {}
}

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::choice::turn::Opt as TOpt;
    use ti4_engine_core::driver::menu::{self, Entry, Source};
    use ti4_engine_core::driver::menu_sources;
    use ti4_engine_core::generated::{Leader, StrategyCard};
    use ti4_engine_core::state::Code;

    use super::{Choice, ClosingEntry, GameState, MenuParts, PlayerId, SourceSet};
    use crate::dual::{Area, Backend, backend, compare_choice, note_bail};
    use crate::generated::IdStr;
    use crate::render::{Render, Typed};

    const MENU: &str = "turn::menu.choice";
    const CLOSING: &str = "turn::closing.choice";

    /// The options the string listed that the core types as `Lent`, in the order met: `Code(i)` is `ids[i]`.
    #[derive(Default)]
    struct Lent {
        ids: Vec<String>,
    }

    impl Lent {
        fn typed(&mut self, state: &GameState, active: &PlayerId, id: &str) -> TOpt {
            match id {
                "pass" => return TOpt::Pass,
                crate::game::TACTICAL_ACTION_ID => return TOpt::Tactical,
                crate::game::END_TURN_ID => return TOpt::EndTurn,
                crate::strategy::STRATEGIC_ACTION_ID => {
                    // The lone unused card keeps the bare id.
                    let unused = state.player(active).map(|p| p.unused_strategy_cards()).unwrap_or_default();
                    if let [card] = unused.as_slice()
                        && let Some(card) = <StrategyCard as IdStr>::from_id(card.as_str())
                    {
                        return TOpt::Strategic { card, bare: true };
                    }
                }
                _ => {
                    if let Some(card) = id.strip_prefix("strategic|").and_then(<StrategyCard as IdStr>::from_id) {
                        return TOpt::Strategic { card, bare: false };
                    }
                    // The leaders' component actions are the core's own options (`Family::parse` of the renderer reads the same ids).
                    if let Some(rest) = id.strip_prefix("component|leader|") {
                        let typed = match rest.strip_prefix("yssarilagent|") {
                            Some(agent) => <Leader as IdStr>::from_id(agent).map(TOpt::Borrowed),
                            None => <Leader as IdStr>::from_id(rest).map(TOpt::Leader),
                        };
                        if let Some(typed) = typed {
                            return typed;
                        }
                    }
                }
            }
            let at = self.ids.iter().position(|known| known == id).unwrap_or_else(|| {
                self.ids.push(id.to_owned());
                self.ids.len() - 1
            });
            TOpt::Lent(Code(u16::try_from(at).unwrap_or(u16::MAX)))
        }
    }

    /// Render the core question and put the string's own option where the core lent one.
    fn render(state: &GameState, string: &Choice, core: ti4_engine_core::choice::Choice, lent: &Lent) -> Option<Choice> {
        let (_, r): (_, Render) = crate::dual_window::tables(state)?;
        let mut typed = Typed::new(&r, core, crate::render::generic::core_phase(state.phase), state.round);
        for (at, opt) in typed.choice.opts.as_slice().iter().enumerate() {
            if let ti4_engine_core::choice::Opt::Turn(TOpt::Lent(code)) = opt {
                let id = lent.ids.get(usize::from(code.0))?;
                *typed.rendered.options.get_mut(at)? = string.options.iter().find(|o| &o.id == id)?.clone();
            }
        }
        Some(typed.rendered)
    }

    pub fn check_action_menu(state: &GameState, sources: SourceSet, parts: &MenuParts, thunders_edge: bool, failed: &[String], string: &Choice) {
        if backend(Area::Driver) != Backend::Dual {
            return;
        }
        let Some(active) = state.active.clone() else { return };
        let Some((mut cx, r)) = crate::dual_window::tables(state) else {
            note_bail(MENU, "the position is outside the tables");
            return;
        };
        let Some(seat) = r.seat(active.as_str()) else {
            note_bail(MENU, "the active seat is not in the tables");
            return;
        };
        let mut lent = Lent::default();
        // The strategic action and the leaders are listed by the core from the position; the other sources are the string's ids.
        let native = crate::menu_sources_view::native(state, &mut cx, sources, seat);
        if native.is_none() {
            note_bail(MENU, "the position is not one the core lists the menu sources of");
        }
        let typed_parts: Vec<(Source, Vec<TOpt>)> = parts
            .iter()
            .filter_map(|(i, ids)| {
                let source = *menu::ORDER.get(usize::from(*i))?;
                let options = match (&native, source) {
                    (Some(native), Source::Strategic) => native.strategic.clone(),
                    (Some(native), Source::Leaders) => native.leaders.clone(),
                    _ => ids.iter().map(|id| lent.typed(state, &active, id)).collect(),
                };
                Some((source, options))
            })
            .collect();
        let typed_failed: Vec<TOpt> = failed.iter().map(|id| lent.typed(state, &active, id)).collect();
        let core = menu::action_menu(seat, &typed_parts, thunders_edge, &typed_failed);
        let Some(rendered) = render(state, string, core, &lent) else {
            note_bail(MENU, "an option the string does not list");
            return;
        };
        crate::dual::note_window(MENU, true);
        compare_choice(Area::Driver, MENU, Some(string), Some(&rendered));
    }

    pub fn check_closing_menu(state: &GameState, _sources: SourceSet, player: &PlayerId, second: bool, turn: &[ClosingEntry], expedition: &[String], string: &Choice) {
        if backend(Area::Driver) != Backend::Dual {
            return;
        }
        let Some((_, r)) = crate::dual_window::tables(state) else {
            note_bail(CLOSING, "the position is outside the tables");
            return;
        };
        let Some(seat) = r.seat(player.as_str()) else {
            note_bail(CLOSING, "the seat is not in the tables");
            return;
        };
        let mut lent = Lent::default();
        // Whether a leader's option is free (printed "During the action phase:") is the core's to say; every other flag is the string's.
        let entries: Vec<Entry<TOpt>> = turn
            .iter()
            .map(|(id, free, pass)| {
                let option = lent.typed(state, player, id);
                Entry { option, free: menu_sources::leader_is_free(option).unwrap_or(*free), pass: *pass }
            })
            .collect();
        let exp: Vec<TOpt> = expedition.iter().map(|id| lent.typed(state, player, id)).collect();
        let options = menu::closing_menu(TOpt::EndTurn, &entries, second, &exp);
        let core = menu::closing_choice(seat, options, second);
        let Some(rendered) = render(state, string, core, &lent) else {
            note_bail(CLOSING, "an option the string does not list");
            return;
        };
        crate::dual::note_window(CLOSING, true);
        compare_choice(Area::Driver, CLOSING, Some(string), Some(&rendered));
    }
}
