use ti4_engine_core::containers::{Cow, SeatSet, SmallList};
use ti4_engine_core::generated::{PublicObjective, SecretObjective};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::state::{CoreState, ObjectiveSet, Slice};
use ti4_model::id::{ObjectiveId, PlayerId};
use ti4_model::state::{FeatOccurrence, GameState};

use super::{Sources, SliceCodec};
use crate::core_sync::conv::{id, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::R;
use crate::generated::IdStr;

const S: Slice = Slice::Objectives;

pub struct Objectives;

impl SliceCodec for Objectives {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let o = &mut core.objectives;
        // A secret objective made public (Classified Document Leaks) sits in `revealed_objectives` as
        // an `ObjectiveId` the public enum cannot hold: such a position is unmapped (see the report).
        o.revealed = gs
            .revealed_objectives
            .iter()
            .map(|x| id::<PublicObjective>(S, "public objective", x.as_str()))
            .collect::<R<SmallList<_, 12>>>()?;
        o.scored = [ObjectiveSet::default(); 16];
        o.scored_present = SeatSet::EMPTY;
        for (p, set) in &gs.scored_objectives {
            let seat = cx.seat(S, p.as_str())?;
            o.scored_present.insert(seat);
            let cell = &mut o.scored[usize::from(seat.0)];
            for obj in set {
                if let Some(x) = PublicObjective::from_id(obj.as_str()) {
                    cell.public.insert(x);
                } else {
                    cell.secret.insert(id::<SecretObjective>(S, "objective", obj.as_str())?);
                }
            }
        }
        let mut feats: Vec<(Seat, u64)> = Vec::with_capacity(gs.scored_feat_occurrences.len());
        for (p, n) in &gs.scored_feat_occurrences {
            feats.push((cx.seat(S, p.as_str())?, n.0));
        }
        o.scored_feats = Cow::new(feats);
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        let o = core.objectives();
        gs.revealed_objectives = o.revealed.iter().map(|x| ObjectiveId::new(txt(*x))).collect();
        gs.scored_objectives.clear();
        for seat in o.scored_present.iter() {
            let cell = &o.scored[usize::from(seat.0)];
            let mut set = std::collections::BTreeSet::new();
            set.extend(cell.public.iter().map(|x| ObjectiveId::new(txt(x))));
            set.extend(cell.secret.iter().map(|x| ObjectiveId::new(txt(x))));
            gs.scored_objectives.insert(PlayerId::new(cx.owner(S, seat)?), set);
        }
        gs.scored_feat_occurrences = o
            .scored_feats
            .iter()
            .map(|(s, n)| Ok((PlayerId::new(cx.owner(S, *s)?), FeatOccurrence(*n))))
            .collect::<R<_>>()?;
        Ok(())
    }
}
