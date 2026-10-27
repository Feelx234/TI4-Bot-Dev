use ti4_engine_core::containers::{Log, SmallList, SortedMap};
use ti4_engine_core::generated::{Agenda as AgendaId, Planet, SecretObjective, StrategyCard, System};
use ti4_engine_core::state::{AgendaCastRec, AgendaRec, CoreState, Outcome, Slice};
use ti4_model::id::PlayerId;
use ti4_model::state::{AgendaCast, AgendaRecord, GameState};

use super::{Sources, SliceCodec};
use crate::core_sync::conv::{id, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};
use crate::generated::IdStr;

const S: Slice = Slice::Agenda;

pub struct Agenda;

/// An outcome text as a typed outcome. The closed forms are tried in a fixed order; the text of a
/// typed outcome is its own id, so `render(parse(t)) == t` for every text. Anything else is an
/// interned `Other`.
pub(super) fn outcome(cx: &mut Ctx, slice: Slice, text: &str) -> R<Outcome> {
    Ok(if text == "for" {
        Outcome::For
    } else if text == "against" {
        Outcome::Against
    } else if let Some(seat) = cx.owners.seat(text) {
        Outcome::Player(seat)
    } else if let Some(p) = Planet::from_id(text) {
        Outcome::Planet(p)
    } else if let Some(s) = System::from_id(text) {
        Outcome::System(s)
    } else if let Some(a) = AgendaId::from_id(text) {
        Outcome::Law(a)
    } else if let Some(c) = StrategyCard::from_id(text) {
        Outcome::StrategyCard(c)
    } else if let Some(o) = SecretObjective::from_id(text) {
        Outcome::SecretObjective(o)
    } else {
        Outcome::Other(cx.texts.intern(slice, text)?)
    })
}

pub(super) fn outcome_text(cx: &Ctx, slice: Slice, o: Outcome) -> R<String> {
    Ok(match o {
        Outcome::For => "for".to_owned(),
        Outcome::Against => "against".to_owned(),
        Outcome::Player(s) => cx.owner(slice, s)?.to_owned(),
        Outcome::Planet(x) => txt(x).to_owned(),
        Outcome::System(x) => txt(x).to_owned(),
        Outcome::Law(x) => txt(x).to_owned(),
        Outcome::StrategyCard(x) => txt(x).to_owned(),
        Outcome::SecretObjective(x) => txt(x).to_owned(),
        Outcome::Other(c) => cx.texts.text(slice, c)?.to_owned(),
    })
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
            a.predictions.insert(seat, outcome(cx, S, o)?);
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
            .map(|(k, v)| Ok((PlayerId::new(cx.owner(S, *k)?), outcome_text(cx, S, *v)?)))
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
