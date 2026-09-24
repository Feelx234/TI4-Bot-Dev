import React, { useState } from 'react';
import { GameShell } from '../components/GameShell.tsx';
import { deriveChoiceRendererModel } from '../presentation/choiceModel.ts';
import { fallbackCases, galleryCases, type GalleryCase } from './decisionGalleryCases.ts';
import './DecisionGallery.css';

/** Local, synthetic presentation only: submissions never contact a game server. */
export const DecisionGallery: React.FC = () => {
  const [selected, setSelected] = useState<GalleryCase | null>(null);
  const [lastSubmitted, setLastSubmitted] = useState<string | null>(null);
  const [rejectNext, setRejectNext] = useState(false);
  const [viewer, setViewer] = useState<'actor' | 'other'>('actor');
  const [nonce, setNonce] = useState(0);
  const [selectedOption, setSelectedOption] = useState<string>();

  const select = (item: GalleryCase | null) => {
    setSelected(item);
    setLastSubmitted(null);
    setRejectNext(false);
    setViewer('actor');
    setSelectedOption(undefined);
    setNonce((previous) => previous + 1);
  };

  const choice = selected ? { ...selected.choice, nonce: `${selected.choice.nonce}-${nonce}` } : null;
  const viewerSeat = viewer === 'actor' ? selected?.choice.actor : 'other_seat';
  const classified = deriveChoiceRendererModel(choice, viewerSeat ?? null)?.workflow;

  return <div className="decision-gallery" data-testid="decision-gallery">
    <header className="decision-gallery__header panel">
      <h1>Decision gallery <small>Development only · synthetic previews</small></h1>
      <p>No server is connected. Submissions are recorded locally; they do not acknowledge an action or advance an engine decision.</p>
      {selected && <div className="workflow-row">
        <button type="button" className="button button--secondary" onClick={() => select(null)}>All decisions</button>
        <button type="button" className="button button--secondary" onClick={() => { setNonce((n) => n + 1); setLastSubmitted(null); }}>Reset preview</button>
        <label><input type="checkbox" checked={viewer === 'other'} onChange={(event) => { setViewer(event.target.checked ? 'other' : 'actor'); setNonce((n) => n + 1); }} /> View as another seat</label>
        <label><input type="checkbox" checked={rejectNext} onChange={(event) => setRejectNext(event.target.checked)} /> Reject next submission</label>
      </div>}
    </header>
    {!selected ? <main className="decision-gallery__index">
      <h2>Workflow kinds ({galleryCases.length})</h2>
      <div className="decision-gallery__grid">{galleryCases.map((item) =>
        <button type="button" key={item.workflow} className="workflow-card decision-gallery__case" onClick={() => select(item)}>
          <strong>{item.title}</strong><span>{item.choice.context?.subtype}</span><span>{item.note}</span>
        </button>)}</div>
      <h2>Fallbacks and boundary states ({fallbackCases.length})</h2>
      <div className="decision-gallery__grid">{fallbackCases.map((item) =>
        <button type="button" key={item.choice.nonce} className="workflow-card decision-gallery__case" onClick={() => select(item)}>
          <strong>{item.title}</strong><span>{item.fallback}</span><span>{item.note}</span>
        </button>)}</div>
    </main> : <main className="decision-gallery__preview">
      <section className="panel decision-gallery__details" aria-label="Preview details">
        <h2>{selected.title}</h2>
        <p>{selected.note}</p>
        <p>Subtype: <code>{choice?.context?.subtype ?? '(none)'}</code> · Classified: <code>{classified ?? '(hidden from other seat)'}</code></p>
        {selected.fallback && <p>Fallback: {selected.fallback}</p>}
        <p>Offered IDs: {choice?.options.length ? choice.options.map((o) => <code key={o.id}>{o.id} </code>) : '(none)'}</p>
        <p role="status">{lastSubmitted ? `Local submission: ${lastSubmitted} (no engine transition)` : 'No local submission yet.'}</p>
        <p>Minimize the decision (or press Escape) to inspect these controls and switch previews.</p>
      </section>
      <GameShell key={`${selected.choice.nonce}-${viewer}`} header={<span>Gallery board preview</span>}
        board={<div className="panel">Illustrative board area · no game state</div>} playerSheet={null} events={[]}
        choice={choice} viewerSeat={viewerSeat} selectedOptionId={selectedOption} onSelectOption={setSelectedOption}
        onSubmitChoice={async (id) => {
          if (!choice?.options.some((o) => o.id === id)) throw new Error('Option is not offered in this preview.');
          if (rejectNext) { setRejectNext(false); throw new Error('Simulated rejection: try again or reset the preview.'); }
          setLastSubmitted(id);
          // Do not synthesize a new nonce: a local callback is not an authoritative transition.
        }} />
    </main>}
  </div>;
};
