//! The faction component actions of the action menu, rendered (package KM0): the table from `(module, code, arg)` of
//! [`ComponentAction`] (`ti4_engine_core::kernel::hooks::components`) to the id, kind and label the string engine lists them with
//! (`faction_abilities::component_actions`, the modules' `component_actions` / `mapped_component_actions`), byte for byte.

use ti4_engine_core::generated::{Planet, Technology};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::hooks::components as c;
use ti4_engine_core::kernel::hooks::sites::ComponentAction;

use super::Render;
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

/// `faction_abilities::ACTION_KIND`.
const KIND: &str = "component";
/// The kind a few modules (Mentak, the Empyrean) give their actions.
const KIND_ACTION: &str = "component_action";

/// The option of `action`, as the string lists it. An action this table does not know renders as a plain id from its triple (the lists
/// never produce one: a test pins every code).
#[must_use]
pub fn option(r: &Render, action: ComponentAction) -> ChoiceOption {
    let (id, kind, label) = text(r, action).unwrap_or_else(|| (format!("faction|{}|{}|{}", action.module.as_str(), action.code, action.arg), KIND.to_owned(), "faction action".to_owned()));
    ChoiceOption::labelled(id, kind, label)
}

fn seat_text(r: &Render, arg: u16) -> String {
    r.seat_text(Seat(u8::try_from(arg).unwrap_or(u8::MAX))).to_owned()
}

fn text(r: &Render, a: ComponentAction) -> Option<(String, String, String)> {
    let (id, k, label): (String, &str, String) = match a.code {
        c::ORBITAL_DROP => ("faction|orbital_drop".into(), KIND, "Orbital Drop: spend a strategy token to land 2 infantry".into()),
        c::PRODUCTION_BIOMES => ("faction|production_biomes".into(), KIND, "Production Biomes: exhaust and spend a strategy token for 4 trade goods".into()),
        c::TRADE_CONVOYS => ("faction|trade_convoys".into(), KIND, "Trade Convoys: place it faceup in your play area".into()),
        c::NUCLEUS => ("faction|muaat|nucleus".into(), KIND, "The Nucleus: exhaust to use Star Forge without a token".into()),
        c::STAR_FORGE => ("faction|muaat|star_forge".into(), KIND, "Star Forge: spend a strategy token to place 2 fighters or 1 destroyer at a war sun".into()),
        c::INFERNO => ("faction|muaat|inferno".into(), KIND, "The Inferno: spend a strategy token to place 1 cruiser in its system".into()),
        c::FIRES => ("faction|muaat|fires".into(), KIND, "Fires of the Gashlai: return a Muaat fleet token and gain the war sun technology".into()),
        c::WORMHOLE_GENERATOR => ("faction|ghost|wg".into(), KIND, "Wormhole Generator: exhaust to place or move a Creuss wormhole token".into()),
        c::VORTEX => ("faction|cabal|vortex".into(), KIND, "Vortex: exhaust to capture another player's unit type from their reinforcements".into()),
        c::PROMISE_OF_PROTECTION => ("faction|mentak|pop".into(), KIND_ACTION, "Promise of Protection: place it faceup in your play area".into()),
        c::VAULTS => {
            let tech = Technology::ALL.get(usize::from(a.arg))?.as_str();
            (format!("faction|mahact|vaults|{tech}"), KIND, format!("Vaults of the Heir: exhaust and purge {tech} to gain a relic"))
        }
        c::C4R_PLACE => ("faction|nekro|c4r|pds".into(), KIND, "???_ERROR_ERROR_???: place 1 PDS on a planet you control".into()),
        c::C4R_REPAIR => ("faction|nekro|c4r|repair".into(), KIND, "???_ERROR_ERROR_???: repair all of your damaged units".into()),
        c::C4R_DRAW => ("faction|nekro|c4r|draw".into(), KIND, "???_ERROR_ERROR_???: discard 1 action card to draw 1 action card".into()),
        c::LAZAX_GATE_FOLDING => ("faction|winnu|lgf".into(), KIND, "Lazax Gate Folding: exhaust to place 1 infantry on Mecatol Rex".into()),
        c::TERRAFORM => {
            let planet = Planet::ALL.get(usize::from(a.arg))?.as_str();
            (format!("faction|titans|terraform|{planet}"), KIND, format!("Terraform: attach it to {planet} (+1 resource, +1 influence, all traits)"))
        }
        c::FABRICATION_RELIC => ("faction|naaz|fabrication_relic".into(), KIND, "Fabrication: purge 2 relic fragments of the same type to gain 1 relic".into()),
        c::FABRICATION_TOKEN => ("faction|naaz|fabrication_token".into(), KIND, "Fabrication: purge 1 relic fragment to gain 1 command token".into()),
        c::BLACK_MARKET_FORGERY => ("faction|naaz|bmf".into(), KIND, "Black Market Forgery: purge 2 relic fragments of the same type to gain 1 relic".into()),
        c::STALL_TACTICS => ("faction|yssaril|stall_tactics".into(), KIND, "Stall Tactics: discard 1 action card from your hand".into()),
        c::BORROWED_STALL_TACTICS => {
            let owner = seat_text(r, a.arg);
            (format!("faction|yssaril|bt_stall|{owner}"), KIND, format!("Deepgloom Executable: use {owner}'s Stall Tactics (if {owner} allows)"))
        }
        c::MAGEON_IMPLANTS => {
            let target = seat_text(r, a.arg);
            (format!("faction|yssaril|mi|{target}"), KIND, format!("Mageon Implants: exhaust to look at {target}'s action cards and take 1"))
        }
        c::BLOOD_PACT => ("faction|empyrean|blood_pact".into(), KIND_ACTION, "Blood Pact: place it faceup in your play area".into()),
        c::DARK_PACT => ("faction|empyrean|dark_pact".into(), KIND_ACTION, "Dark Pact: place it faceup in your play area".into()),
        _ => return None,
    };
    Some((id, k.to_owned(), label))
}
