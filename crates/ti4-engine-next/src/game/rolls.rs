//! The roll feed: the core's dice ledger as the viewer's `Roll`s.

use std::collections::BTreeSet;

use ti4_engine_core::state::{CoreState, RollPurpose};

use crate::core_sync::Ctx;
use crate::generated::IdStr;
use crate::render::Render;
use crate::wire::dice::{Roll, RollSource};

/// Every die of the ledger, in order. `_have` is the count already shown (the whole ledger is rendered again: a few hundred entries a game).
pub(super) fn rendered(core: &CoreState, cx: &Ctx, render: &Render, _have: usize) -> Vec<Roll> {
    let seat_text = |s: ti4_engine_core::Seat| cx.owners.ids().get(usize::from(s.0)).cloned();
    core.ledger
        .dice
        .iter()
        .map(|r| Roll {
            reason: match r.purpose {
                RollPurpose::SpaceCombat => "space combat".to_owned(),
                RollPurpose::GroundCombat => "ground combat".to_owned(),
                RollPurpose::AntiFighterBarrage => "anti-fighter barrage".to_owned(),
                RollPurpose::SpaceCannon => "space cannon".to_owned(),
                RollPurpose::Bombardment => "bombardment".to_owned(),
                RollPurpose::Reroll => "reroll".to_owned(),
                RollPurpose::Ability(_) => "ability".to_owned(),
                RollPurpose::Other(code) => render.code_text(code),
            },
            faces: r.faces.iter().map(|f| u32::from(*f)).collect(),
            hits_on: r.hits_on.map(u32::from),
            rerolled: (0..64).filter(|i| r.rerolled >> i & 1 == 1).collect::<BTreeSet<usize>>(),
            by: r.owner.and_then(seat_text),
            system: r.scene.map(|s| s.as_str().to_owned()),
            outcome: None,
            source: r.unit.map(|u| RollSource { unit: u.as_str().to_owned(), planet: r.planet.map(|p| p.as_str().to_owned()), system: None }),
        })
        .collect()
}
