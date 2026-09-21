import { GENERATED_CONTENT_CATALOG } from './generatedContentManifest.ts';

export interface StrategyCardMeta {
  id: string;
  name: string;
  initiative: number;
  primaryText: string;
  secondaryText: string;
}

export interface ObjectiveMeta {
  id: string;
  name: string;
  phase: string;
  points: number;
  description: string;
}

export interface CardMeta {
  id: string;
  name: string;
  phase?: string;
  description: string;
}

export const STRATEGY_CARDS: Record<string, StrategyCardMeta> = GENERATED_CONTENT_CATALOG.strategyCards;
export const SECRET_OBJECTIVES: Record<string, ObjectiveMeta> = GENERATED_CONTENT_CATALOG.secretObjectives;
export const PUBLIC_OBJECTIVES: Record<string, ObjectiveMeta> = GENERATED_CONTENT_CATALOG.publicObjectives;
export const ACTION_CARDS: Record<string, CardMeta> = GENERATED_CONTENT_CATALOG.actionCards;
export const TECHNOLOGIES: Record<string, CardMeta> = GENERATED_CONTENT_CATALOG.technologies;

function normalizeId(id: string): string {
  return id.toLowerCase().trim();
}

export function humanizeId(id: string): string {
  return id
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (char) => char.toUpperCase());
}

export function getStrategyCardMeta(id: string): StrategyCardMeta {
  return STRATEGY_CARDS[normalizeId(id)] ?? {
    id,
    name: humanizeId(id),
    initiative: 0,
    primaryText: '',
    secondaryText: '',
  };
}

export function getSecretObjectiveMeta(id: string): ObjectiveMeta {
  return SECRET_OBJECTIVES[normalizeId(id)] ?? {
    id,
    name: humanizeId(id),
    phase: 'Secret',
    points: 1,
    description: 'Secret Objective',
  };
}

export function getPublicObjectiveMeta(id: string): ObjectiveMeta {
  return PUBLIC_OBJECTIVES[normalizeId(id)] ?? {
    id,
    name: humanizeId(id),
    phase: 'Public',
    points: 1,
    description: 'Public Objective',
  };
}

export function getActionCardMeta(id: string): CardMeta {
  return ACTION_CARDS[normalizeId(id)] ?? {
    id,
    name: humanizeId(id),
    description: 'Action Card',
  };
}

export function getTechnologyMeta(id: string): CardMeta {
  return TECHNOLOGIES[normalizeId(id)] ?? {
    id,
    name: humanizeId(id),
    description: 'Technology',
  };
}

export function formatActionDescription(optionId: string): string {
  if (!optionId) return '';
  const trimmed = optionId.trim();

  if (trimmed.startsWith('strategic|')) {
    const cardId = trimmed.slice('strategic|'.length);
    const meta = getStrategyCardMeta(cardId);
    return `Play Strategy Card: ${meta.initiative > 0 ? `${meta.initiative}. ` : ''}${meta.name}`;
  }

  if (trimmed.startsWith('pok') || STRATEGY_CARDS[normalizeId(trimmed)]) {
    const meta = getStrategyCardMeta(trimmed);
    if (meta.initiative > 0) {
      return `Strategy Card: ${meta.initiative}. ${meta.name}`;
    }
  }

  if (trimmed.startsWith('tactical|')) {
    return `Tactical Action (System ${trimmed.slice('tactical|'.length)})`;
  }

  if (trimmed.startsWith('component|')) {
    return `Component Action: ${humanizeId(trimmed.slice('component|'.length))}`;
  }

  if (trimmed === 'pass') {
    return 'Pass Turn';
  }

  if (trimmed === 'generic') {
    return 'Confirm Selection';
  }

  if (trimmed === 'top') {
    return 'Place on Top of Deck';
  }

  if (trimmed === 'bottom') {
    return 'Place on Bottom of Deck';
  }

  if (trimmed.startsWith('produce|') || trimmed.startsWith('produce_unit|')) {
    const unit = trimmed.split('|')[1];
    return `Produce ${humanizeId(unit)}`;
  }

  if (trimmed.startsWith('place ')) {
    return humanizeId(trimmed);
  }

  return humanizeId(trimmed);
}
