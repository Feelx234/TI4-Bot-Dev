import type { ChoiceWorkflowKind } from '../presentation/choiceModel.ts';
import type { DecisionTargetDto, PendingChoiceDto } from '../protocol/types.ts';

// Illustrative UI inputs, not captured engine states or legal-game fixtures.
const actor = 'gallery_seat';
const option = (id: string, label: string, kind?: string, payload?: Record<string, unknown>) =>
  ({ id, label, kind, payload });
const finish = (id = 'decline', label = 'Finish') => option(id, label, 'decline');
const system = { System: '18' } as const;

export interface GalleryCase {
  title: string;
  note: string;
  choice: PendingChoiceDto;
  workflow: ChoiceWorkflowKind;
  /** A scenario, not an additional workflow kind. */
  fallback?: string;
}

const cases = {
  system_activation: { subtype: 'activate_system', options: [option('18', 'Activate system 18', 'activate', { system: '18' })], note: 'Board target and generic choice modal (no dedicated activation renderer).' },
  tactical_movement: { subtype: 'movement_step', target: system, options: [option('move|24|0', 'Move cruiser from 24', 'move', { origin: '24', unit: 'cruiser', capacity: 0 }), finish('done_moving', 'Done moving')], note: 'Staged movement and explicit finish.' },
  tactical_cargo: { subtype: 'load_cargo', target: system, options: [option('load|infantry', 'Load infantry', 'load', { unit: 'infantry', source: 'jord', capacity_remaining: 2 }), finish('done_loading', 'Done loading')], note: 'Cargo tray; board counts are illustrative.' },
  tactical_invasion: { subtype: 'commit_ground_forces', target: system, options: [option('land|infantry', 'Land infantry on Jord', 'land', { planet: 'jord', unit: 'infantry' }), finish('done_landing', 'Done landing')], note: 'Classified as invasion; currently uses generic modal fallback.' },
  payment: { subtype: 'pay_resources', outstanding: [{ kind: 'resources', amount: 2, paid: 0 }], options: [option('exhaust|jord', 'Exhaust Jord', 'pay', { worth: 2, owed: 2, kind: 'resources', planet_name: 'Jord' }), finish()], note: 'Payment drawer with a planet payment.' },
  production: { subtype: 'produce_unit', target: system, outstanding: [{ kind: 'production_capacity', amount: 3, paid: 0 }], options: [option('build|infantry|1', 'Produce infantry', 'produce', { unit: 'infantry', cost: 1, count: 1, available_resources: 3 }), finish('done_producing', 'Done producing')], note: 'Production builder with capacity and resource preview.' },
  combat_sustain: { subtype: 'sustain_damage', outstanding: [{ amount: 1 }], options: [option('sustain|dreadnought', 'Sustain dreadnought', 'sustain', { unit: 'dreadnought' }), finish()], note: 'Sustain or pass.' },
  combat_casualty: { subtype: 'assign_casualty', outstanding: [{ amount: 1 }], options: [option('destroy|fighter', 'Destroy fighter', 'casualty', { unit: 'fighter' })], note: 'Assign a hit.' },
  combat_retreat: { subtype: 'retreat_to', options: [option('retreat|18', 'Retreat to 18', 'retreat', { system: '18' }), finish()], note: 'Retreat destination or pass.' },
  agenda_vote_outcome: { subtype: 'cast_vote', options: [option('FOR', 'For', 'outcome'), option('AGAINST', 'Against', 'outcome')], note: 'Choose an outcome.' },
  agenda_vote_planets: { subtype: 'vote_exhaust_planet', options: [option('exhaust|jord', 'Exhaust Jord for 2 votes', 'exhaust', { votes: 2 }), finish()], note: 'Exhaust a planet or finish voting.' },
  transaction_propose: { subtype: 'propose_transaction', target: { Player: 'other_seat' }, options: [option('offer|1', 'Offer one trade good', 'offer', { net: -1, their_net: 1 }), finish()], note: 'Propose terms to another seat.' },
  transaction_answer: { subtype: 'answer_transaction', target: { Player: 'other_seat' }, options: [option('accept', 'Accept offer', 'accept'), finish('refuse', 'Refuse offer')], note: 'Accept or refuse incoming terms.' },
  action_card_reaction: { subtype: 'play_reaction_after_ACTION_CARD_PLAYED', options: [option('sabotage', 'Play Sabotage', 'action_card'), finish('decline', 'Pass')], note: 'Reaction window with pass.' },
  objective_scoring: { subtype: 'score_objective', options: [option('objective|1', 'Score objective', 'score'), finish()], note: 'Currently uses generic choice modal.' },
  generic_selection: { subtype: 'draft_strategy_card', options: [option('leadership', 'Leadership'), option('politics', 'Politics')], note: 'Known engine subtype routed through generic choice modal.' },
} satisfies Record<ChoiceWorkflowKind, {
  subtype: string;
  options: PendingChoiceDto['options'];
  note: string;
  target?: DecisionTargetDto;
  outstanding?: NonNullable<PendingChoiceDto['context']>['outstanding'];
}>;

export const galleryCases: GalleryCase[] = (Object.keys(cases) as ChoiceWorkflowKind[]).map((workflow) => {
  const entry = cases[workflow];
  return {
    workflow, title: workflow.replaceAll('_', ' '), note: entry.note,
    choice: { actor, nonce: `gallery-${workflow}`, prompt: `Preview: ${workflow.replaceAll('_', ' ')}`,
      context: { subtype: entry.subtype, ...('target' in entry ? { target: entry.target } : {}),
        ...('outstanding' in entry ? { outstanding: entry.outstanding } : {}) }, options: entry.options },
  };
});

export const fallbackCases: GalleryCase[] = [
  { workflow: 'generic_selection', title: 'Unknown subtype', fallback: 'Unknown subtype → generic modal', note: 'Unknown engine subtypes fall back to the generic single-choice modal.',
    choice: { actor, nonce: 'gallery-unknown', prompt: 'Unknown subtype', context: { subtype: 'gallery_unrecognized' }, options: [option('offered_option', 'Offered option')] } },
  { workflow: 'generic_selection', title: 'Missing context', fallback: 'No context → generic modal', note: 'A choice with no context still renders offered IDs.',
    choice: { actor, nonce: 'gallery-no-context', prompt: 'No context', options: [option('offered_option', 'Offered option')] } },
  { workflow: 'generic_selection', title: 'Bounded multi-select', fallback: 'Unknown subtype + constraint → generic multi-select', note: 'The UI stages choices; a mock acknowledgement does not establish a next decision.',
    choice: { actor, nonce: 'gallery-multi', prompt: 'Select two', context: { subtype: 'gallery_bounded', outstanding: [{ min_selection: 2, max_selection: 2 }] }, options: [option('a', 'Option A'), option('b', 'Option B'), option('c', 'Option C')] } },
  { workflow: 'tactical_movement', title: 'Empty movement', fallback: 'Explicit finish, no movable units', note: 'Done moving is the only offered option.',
    choice: { actor, nonce: 'gallery-empty-move', prompt: 'No units can move', context: { subtype: 'movement_step', target: system }, options: [finish('done_moving', 'Done moving')] } },
  { workflow: 'tactical_movement', title: 'Missing movement finish', fallback: 'Missing explicit finish → visible error', note: 'No arbitrary option is submitted as a finish.',
    choice: { actor, nonce: 'gallery-missing-finish', prompt: 'No finish offered', context: { subtype: 'movement_step', target: system }, options: [option('unrelated', 'Unrelated option', 'other')] } },
  { workflow: 'generic_selection', title: 'No options', fallback: 'Empty generic choice → disabled submit', note: 'Illustrative invalid/unactionable choice; investigate server state if encountered live.',
    choice: { actor, nonce: 'gallery-no-options', prompt: 'No options offered', context: { subtype: 'gallery_empty' }, options: [] } },
];
