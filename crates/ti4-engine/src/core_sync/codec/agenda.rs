use ti4_engine_core::containers::{Log, SmallList, SortedMap};
use ti4_engine_core::generated::Agenda as AgendaId;
use ti4_engine_core::state::{AgendaCastRec, AgendaRec, CoreState, Outcome, Prediction, Slice};
use ti4_model::id::PlayerId;
use ti4_model::state::{AgendaCast, AgendaRecord, GameState};

use super::{Sources, SliceCodec};
use crate::core_sync::conv::{id, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};

const S: Slice = Slice::Agenda;

pub struct Agenda;

/// An outcome text as package X1's typed outcome (no agenda hint: the kinds are tried in the order
/// of `Outcome`'s variants; the text is the same either way). The text must render back as itself;
/// anything else is `Unmapped`.
pub(crate) fn outcome(cx: &Ctx, slice: Slice, text: &str) -> R<Outcome> {
    cx.keys
        .parse_outcome(text, None)
        .filter(|o| cx.keys.render_outcome(*o).is_some_and(|t| t == text))
        .ok_or_else(|| bad(slice, "outcome not typed", text))
}

/// A `agenda_predictions` value: an outcome, `<outcome>|<rider>` or `none|<bar>`.
fn prediction(cx: &Ctx, text: &str) -> R<Prediction> {
    cx.keys
        .parse_prediction(text)
        .filter(|p| cx.keys.render_prediction(*p).is_some_and(|t| t == text))
        .ok_or_else(|| bad(S, "prediction not typed", text))
}

pub(super) fn outcome_text(cx: &Ctx, slice: Slice, o: Outcome) -> R<String> {
    cx.keys.render_outcome(o).ok_or_else(|| bad(slice, "outcome without a text", format!("{o:?}")))
}

fn agenda_name(alias: &str) -> String {
    ti4_content::ContentStore::embedded()
        .get(ti4_model::content_types::ContentType::Agendas, alias)
        .and_then(|r| r.text("name"))
        .unwrap_or_default()
        .to_owned()
}

impl SliceCodec for Agenda {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let mut a = ti4_engine_core::state::AgendaState::default();
        for (alias, choice) in &gs.laws {
            let o = outcome(cx, S, choice)?;
            a.laws.insert(id::<AgendaId>(S, "agenda", alias)?, o);
        }
        a.in_play = gs.agenda_in_play.as_deref().map(|x| id::<AgendaId>(S, "agenda", x)).transpose()?;
        let mut choices: SmallList<Outcome, 8> = SmallList::new();
        for c in &gs.agenda_choices {
            choices.push(outcome(cx, S, c)?);
        }
        a.choices = choices;
        a.veto_replacement = gs.agenda_veto_replacement.as_deref().map(|x| id::<AgendaId>(S, "agenda", x)).transpose()?;
        a.covert = gs.covert_agenda.as_deref().map(|x| id::<AgendaId>(S, "agenda", x)).transpose()?;
        a.elected_override = gs.agenda_elected_override.as_ref().map(|p| cx.seat(S, p.as_str())).transpose()?;
        for (p, o) in &gs.agenda_votes {
            let seat = cx.seat(S, p.as_str())?;
            a.votes.insert(seat, outcome(cx, S, o)?);
        }
        for (p, o) in &gs.agenda_predictions {
            let seat = cx.seat(S, p.as_str())?;
            a.predictions.insert(seat, prediction(cx, o)?);
        }
        let mut log = Log::new();
        for r in &gs.agenda_log {
            let agenda = id::<AgendaId>(S, "agenda", &r.agenda)?;
            if r.name != agenda_name(&r.agenda) {
                return Err(bad(S, "agenda name differs from the corpus", &r.name));
            }
            let mut votes = SortedMap::new();
            for (k, v) in &r.votes {
                votes.insert(outcome(cx, S, k)?, *v);
            }
            log.push(AgendaRec {
                round: r.round,
                agenda,
                outcome: r.outcome.as_deref().map(|o| outcome(cx, S, o)).transpose()?,
                votes,
                speaker_decided: r.speaker_decided,
                law: r.law,
                enacted: r.enacted,
                discarded: r.discarded,
                redirected_to: r.redirected_to.as_deref().map(|o| outcome(cx, S, o)).transpose()?,
                discard_reason: r.discard_reason.as_deref().map(|t| cx.texts.intern(S, t)).transpose()?,
            });
        }
        a.log = log;
        let mut casts = Log::new();
        for c in &gs.agenda_casts {
            casts.push(AgendaCastRec {
                player: cx.seat(S, c.player.as_str())?,
                outcome: c.outcome.as_deref().map(|o| outcome(cx, S, o)).transpose()?,
                votes: c.votes,
            });
        }
        a.casts = casts;
        core.agenda = a;
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        let a = core.agenda();
        gs.laws = a.laws.iter().map(|(k, v)| Ok((txt(*k).to_owned(), outcome_text(cx, S, *v)?))).collect::<R<_>>()?;
        gs.agenda_in_play = a.in_play.map(|x| txt(x).to_owned());
        gs.agenda_choices = a.choices.iter().map(|o| outcome_text(cx, S, *o)).collect::<R<_>>()?;
        gs.agenda_veto_replacement = a.veto_replacement.map(|x| txt(x).to_owned());
        gs.covert_agenda = a.covert.map(|x| txt(x).to_owned());
        gs.agenda_elected_override = a.elected_override.map(|s| cx.owner(S, s).map(PlayerId::new)).transpose()?;
        gs.agenda_votes = a
            .votes
            .iter()
            .map(|(k, v)| Ok((PlayerId::new(cx.owner(S, *k)?), outcome_text(cx, S, *v)?)))
            .collect::<R<_>>()?;
        gs.agenda_predictions = a
            .predictions
            .iter()
            .map(|(k, v)| {
                let text = cx.keys.render_prediction(*v).ok_or_else(|| bad(S, "prediction without a text", format!("{v:?}")))?;
                Ok((PlayerId::new(cx.owner(S, *k)?), text))
            })
            .collect::<R<_>>()?;
        gs.agenda_log = a
            .log
            .iter()
            .map(|r| {
                let alias = txt(r.agenda);
                Ok(AgendaRecord {
                    round: r.round,
                    agenda: alias.to_owned(),
                    name: agenda_name(alias),
                    outcome: r.outcome.map(|o| outcome_text(cx, S, o)).transpose()?,
                    votes: r.votes.iter().map(|(k, v)| Ok((outcome_text(cx, S, *k)?, *v))).collect::<R<_>>()?,
                    speaker_decided: r.speaker_decided,
                    law: r.law,
                    enacted: r.enacted,
                    discarded: r.discarded,
                    redirected_to: r.redirected_to.map(|o| outcome_text(cx, S, o)).transpose()?,
                    discard_reason: r.discard_reason.map(|c| cx.texts.text(S, c).map(str::to_owned)).transpose()?,
                })
            })
            .collect::<R<_>>()?;
        gs.agenda_casts = a
            .casts
            .iter()
            .map(|c| {
                Ok(AgendaCast {
                    player: PlayerId::new(cx.owner(S, c.player)?),
                    outcome: c.outcome.map(|o| outcome_text(cx, S, o)).transpose()?,
                    votes: c.votes,
                })
            })
            .collect::<R<_>>()?;
        Ok(())
    }
}
