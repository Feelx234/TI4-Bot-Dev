import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { PendingChoiceDto } from "../protocol/types.ts";
import {
  describeCommandTokens,
  paymentCheck,
  paymentDraftOf,
  purchaseOffer,
  stepPaymentGoods,
  toggleMapPlanet,
  type CommandTokenView,
  type PaymentCheck,
  type PaymentOverride,
  type TokenStaging,
} from "./commandTokens.ts";
import type { PaymentDraft, PaymentOffer } from "./paymentDraft.ts";

/** What the player has staged on the token panel; nothing in it is sent until confirmed. */
export interface TokenDraft {
  /** null: the engine's starting arrangement, untouched. */
  staging: TokenStaging | null;
  bought: number;
  /** null: Auto-pay plans the payment. */
  override: PaymentOverride | null;
}

export const EMPTY_TOKEN_DRAFT: TokenDraft = { staging: null, bought: 0, override: null };

export interface TokenDraftState {
  draft: TokenDraft;
  update: (change: (draft: TokenDraft) => TokenDraft) => void;
  reset: () => void;
}

/** The staged tokens and purchase, reset whenever the pending decision changes. */
export function useTokenDraftState(nonce: string | null | undefined): TokenDraftState {
  const [draft, setDraft] = useState<TokenDraft>(EMPTY_TOKEN_DRAFT);
  useEffect(() => setDraft(EMPTY_TOKEN_DRAFT), [nonce]);
  const update = useCallback((change: (d: TokenDraft) => TokenDraft) => setDraft(change), []);
  const reset = useCallback(() => setDraft(EMPTY_TOKEN_DRAFT), []);
  return useMemo(() => ({ draft, update, reset }), [draft, update, reset]);
}

/** The purchase payment as the map shows it: planets with their worth, what is staged, the account. */
export interface TokenMapPayment {
  view: CommandTokenView;
  offer: PaymentOffer;
  draft: PaymentDraft;
  check: PaymentCheck;
}

export interface CommandTokenDraftApi extends TokenDraftState {
  view: CommandTokenView | null;
  /** Non-null while tokens are bought, so there is a bill the map can pay. */
  mapPayment: TokenMapPayment | null;
  togglePlanet: (planetId: string) => void;
  stepTradeGoods: (delta: number) => void;
}

export function useCommandTokenDraft(choice: PendingChoiceDto | null | undefined): CommandTokenDraftApi {
  const state = useTokenDraftState(choice?.nonce);
  const { draft, update } = state;
  const view = useMemo(() => (choice ? describeCommandTokens(choice, true) : null), [choice]);
  const mapPayment = useMemo<TokenMapPayment | null>(() => {
    if (!view?.purchase || draft.bought <= 0) return null;
    const offer = purchaseOffer(view, draft.bought);
    if (!offer) return null;
    return {
      view,
      offer,
      draft: paymentDraftOf(view, draft.bought, draft.override),
      check: paymentCheck(view, draft.bought, draft.override),
    };
  }, [view, draft.bought, draft.override]);
  const togglePlanet = useCallback(
    (planetId: string) => {
      if (!view) return;
      update((d) => ({ ...d, override: toggleMapPlanet(view, d.bought, d.override, planetId) }));
    },
    [view, update],
  );
  const stepTradeGoods = useCallback(
    (delta: number) => {
      if (!view) return;
      update((d) => ({ ...d, override: stepPaymentGoods(view, d.bought, d.override, delta) }));
    },
    [view, update],
  );
  return useMemo(
    () => ({ ...state, view, mapPayment, togglePlanet, stepTradeGoods }),
    [state, view, mapPayment, togglePlanet, stepTradeGoods],
  );
}

const CommandTokenDraftContext = createContext<CommandTokenDraftApi | null>(null);

export const CommandTokenDraftProvider: React.FC<{
  value: CommandTokenDraftApi;
  children: React.ReactNode;
}> = ({ value, children }) => (
  <CommandTokenDraftContext.Provider value={value}>{children}</CommandTokenDraftContext.Provider>
);

/** The shared token draft (panel, map and bar), or null outside a provider. */
export const useSharedTokenDraft = (): CommandTokenDraftApi | null => useContext(CommandTokenDraftContext);
