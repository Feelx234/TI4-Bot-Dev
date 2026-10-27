use ti4_engine_core::generated::{GalacticEvent, System, Wormhole};
use ti4_engine_core::state::{CoreState, MetaFlags, Phase, Slice};
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::{GameState, Phase as P};

use super::{Sources, SliceCodec};
use crate::core_sync::conv::{id, narrow, opt32, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::R;

const S: Slice = Slice::Meta;

pub struct Meta;

fn phase(p: P) -> Phase {
    match p {
        P::Strategy => Phase::Strategy,
        P::Action => Phase::Action,
        P::Status => Phase::Status,
        P::Agenda => Phase::Agenda,
    }
}

fn phase_back(p: Phase) -> P {
    match p {
        Phase::Strategy => P::Strategy,
        Phase::Action => P::Action,
        Phase::Status => P::Status,
        Phase::Agenda => P::Agenda,
    }
}

impl SliceCodec for Meta {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let m = &mut core.meta;
        m.rules_version = gs.rules_version;
        m.round = gs.round;
        m.phase = phase(gs.phase);
        m.speaker = cx.seat(S, gs.speaker.as_str())?;
        m.active = gs.active.as_ref().map(|p| cx.seat(S, p.as_str())).transpose()?;
        m.active_system = gs.active_system.as_ref().map(|s| id::<System>(S, "system", s.as_str())).transpose()?;
        m.finished = gs.finished;
        let mut flags = MetaFlags(0);
        flags.set(MetaFlags::LEGACY_DEVICE_IDS, gs.legacy_device_ids);
        flags.set(MetaFlags::CUSTODIANS_REMOVED, gs.custodians_removed);
        flags.set(MetaFlags::FRACTURE_IN_PLAY, gs.fracture_in_play);
        flags.set(MetaFlags::NEXUS_UNLOCKED, gs.nexus_unlocked);
        flags.set(MetaFlags::THUNDERS_EDGE, gs.thunders_edge);
        m.flags = flags;
        m.strategy_cards_per_player = narrow(S, "strategy_cards_per_player", gs.strategy_cards_per_player)?;
        m.styx_holder = gs.styx_holder.as_ref().map(|p| cx.seat(S, p.as_str())).transpose()?;
        m.thunders_edge_system =
            gs.thunders_edge_system.as_ref().map(|s| id::<System>(S, "system", s.as_str())).transpose()?;
        m.galactic_event = gs.galactic_event.as_deref().map(|e| id::<GalacticEvent>(S, "galactic event", e)).transpose()?;
        m.ion_storm = gs
            .ion_storm
            .as_ref()
            .map(|(s, face)| Ok((id::<System>(S, "system", s.as_str())?, id::<Wormhole>(S, "ion storm face", face)?)))
            .transpose()?;
        m.influence_pays_for_units = gs
            .influence_pays_for_units
            .iter()
            .map(|p| cx.seat(S, p.as_str()))
            .collect::<R<_>>()?;
        m.combat_round_seq = gs.combat_round_seq;
        m.production_seq = gs.production_seq;
        m.activation_seq = gs.activation_seq;
        m.agenda_seq = gs.agenda_seq;
        m.turn_seq = gs.turn_seq;
        m.action_turn = gs.action_turn;
        m.feat_occurrence_seq = gs.feat_occurrence_seq;
        m.combat_draw_round = opt32(S, "combat_draw_round", gs.combat_draw_round)?;
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        let m = core.meta();
        gs.rules_version = m.rules_version;
        gs.round = m.round;
        gs.phase = phase_back(m.phase);
        gs.speaker = PlayerId::new(cx.owner(S, m.speaker)?);
        gs.active = m.active.map(|s| cx.owner(S, s).map(PlayerId::new)).transpose()?;
        gs.active_system = m.active_system.map(|s| SystemId::new(txt(s)));
        gs.finished = m.finished;
        gs.legacy_device_ids = m.flags.has(MetaFlags::LEGACY_DEVICE_IDS);
        gs.custodians_removed = m.flags.has(MetaFlags::CUSTODIANS_REMOVED);
        gs.fracture_in_play = m.flags.has(MetaFlags::FRACTURE_IN_PLAY);
        gs.nexus_unlocked = m.flags.has(MetaFlags::NEXUS_UNLOCKED);
        gs.thunders_edge = m.flags.has(MetaFlags::THUNDERS_EDGE);
        gs.strategy_cards_per_player = usize::from(m.strategy_cards_per_player);
        gs.styx_holder = m.styx_holder.map(|s| cx.owner(S, s).map(PlayerId::new)).transpose()?;
        gs.thunders_edge_system = m.thunders_edge_system.map(|s| SystemId::new(txt(s)));
        gs.galactic_event = m.galactic_event.map(|e| txt(e).to_owned());
        gs.ion_storm = m.ion_storm.map(|(s, f)| (SystemId::new(txt(s)), txt(f).to_owned()));
        gs.influence_pays_for_units =
            m.influence_pays_for_units.iter().map(|s| cx.owner(S, s).map(PlayerId::new)).collect::<R<_>>()?;
        gs.combat_round_seq = m.combat_round_seq;
        gs.production_seq = m.production_seq;
        gs.activation_seq = m.activation_seq;
        gs.agenda_seq = m.agenda_seq;
        gs.turn_seq = m.turn_seq;
        gs.action_turn = m.action_turn;
        gs.feat_occurrence_seq = m.feat_occurrence_seq;
        gs.combat_draw_round = m.combat_draw_round.get();
        Ok(())
    }
}
