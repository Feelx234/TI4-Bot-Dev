import React from "react";
import {
  Staged,
  StagedDeal,
  StagedSide,
  SideLimits,
  isStagedEmpty,
  nearestDeals,
  sideLines,
  stagedToDealId,
  stagingLimits,
  whyNoDeal,
} from "../presentation/tradeStaging.ts";
import { TradeItemDisplay } from "./TradeItemDisplay.tsx";

export interface Holdings {
  tradeGoods: number;
  commodities: number;
}

const Stepper: React.FC<{
  label: string;
  testId: string;
  value: number;
  max: number;
  onChange: (n: number) => void;
}> = ({ label, testId, value, max, onChange }) => (
  <div className="trade-stepper" role="group" aria-label={label} data-testid={testId}>
    <span className="trade-stepper__label">{label}</span>
    <div className="trade-stepper__controls">
      <button
        type="button"
        className="button button--secondary trade-stepper__btn"
        aria-label={`Decrease ${label}`}
        data-testid={`${testId}-dec`}
        disabled={value <= 0}
        onClick={() => onChange(value - 1)}
      >
        −
      </button>
      <output className="trade-stepper__value" aria-live="polite" data-testid={`${testId}-value`}>
        {value}
      </output>
      <button
        type="button"
        className="button button--secondary trade-stepper__btn"
        aria-label={`Increase ${label}`}
        data-testid={`${testId}-inc`}
        disabled={value >= max}
        onClick={() => onChange(value + 1)}
      >
        +
      </button>
      <span className="trade-stepper__hint">listed up to {max}</span>
    </div>
  </div>
);

const ItemToggle: React.FC<{
  name: string;
  label: string;
  card: boolean;
  type: "offer" | "receive";
  pressed: boolean;
  testId: string;
  onToggle: () => void;
}> = ({ name, label, card, type, pressed, testId, onToggle }) => (
  <button
    type="button"
    className="trade-toggle"
    aria-pressed={pressed}
    aria-label={`${pressed ? "Remove" : "Add"} ${label}`}
    data-testid={testId}
    data-staged={pressed}
    onClick={onToggle}
  >
    <TradeItemDisplay label={name} type={type} isCard={card} />
    <span className="trade-toggle__state" aria-hidden="true">
      {pressed ? "Staged" : "Add"}
    </span>
  </button>
);

interface ColumnProps {
  title: string;
  subtitle?: string;
  testId: string;
  type: "offer" | "receive";
  side: StagedSide;
  limits: SideLimits;
  held?: Holdings | null;
  heldLabel?: string;
  onChange: (side: StagedSide) => void;
}

const Column: React.FC<ColumnProps> = ({
  title,
  subtitle,
  testId,
  type,
  side,
  limits,
  held,
  heldLabel,
  onChange,
}) => {
  const set = (patch: Partial<StagedSide>) => onChange({ ...side, ...patch });
  const key = testId.replace("stage-", "");
  // Whatever is staged stays visible even if this list does not offer it, so a pre-filled
  // counter-offer never hides the part that makes it unavailable.
  const withStaged = (listed: string[], staged: (string | null)[]) => [
    ...listed,
    ...staged.filter((x): x is string => !!x && !listed.includes(x)),
  ];
  const notes = withStaged(limits.notes, [side.note]);
  const cards = withStaged(limits.actionCards, [side.actionCard]);
  const secrets = withStaged(limits.secrets, [side.secret]);
  const fragments = withStaged(limits.fragments, side.fragments);
  const hasSupport = limits.support || side.support;
  const maxTg = Math.max(limits.maxTradeGoods, side.tradeGoods);
  const maxCm = Math.max(limits.maxCommodities, side.commodities);
  // The seat's own purse caps what it can give; the partner's is only a hint, the list is the limit.
  const capTg = type === "offer" && held ? Math.max(Math.min(maxTg, held.tradeGoods), side.tradeGoods) : maxTg;
  const capCm = type === "offer" && held ? Math.max(Math.min(maxCm, held.commodities), side.commodities) : maxCm;
  const nothingListed =
    maxTg === 0 &&
    maxCm === 0 &&
    !notes.length &&
    !cards.length &&
    !secrets.length &&
    !fragments.length &&
    !hasSupport;
  return (
    <section className={`trade-column trade-column--${type}`} data-testid={testId} aria-label={title}>
      <h4 className="trade-section__title">{title}</h4>
      {subtitle && <p className="trade-column__subtitle">{subtitle}</p>}
      {held && (
        <p className="trade-column__held" data-testid={`${testId}-held`}>
          {heldLabel}: {held.tradeGoods} trade goods, {held.commodities} commodities
        </p>
      )}
      {nothingListed && (
        <p className="trade-column__none">Nothing on this side is available in the listed deals.</p>
      )}
      {maxTg > 0 && (
        <Stepper
          label={`Trade goods (${key})`}
          testId={`${testId}-tg`}
          value={side.tradeGoods}
          max={capTg}
          onChange={(n) => set({ tradeGoods: n })}
        />
      )}
      {maxCm > 0 && (
        <Stepper
          label={`Commodities (${key})`}
          testId={`${testId}-cm`}
          value={side.commodities}
          max={capCm}
          onChange={(n) => set({ commodities: n })}
        />
      )}
      {hasSupport && (
        <ItemToggle
          name="Promissory Note: support_for_throne"
          label="Support for the Throne"
          card
          type={type}
          pressed={side.support}
          testId={`${testId}-support`}
          onToggle={() => set({ support: !side.support })}
        />
      )}
      {notes.map((note) => (
        <ItemToggle
          key={`n-${note}`}
          name={`Promissory Note: ${note}`}
          label={`the note ${note}`}
          card
          type={type}
          pressed={side.note === note}
          testId={`${testId}-note-${note}`}
          onToggle={() => set({ note: side.note === note ? null : note })}
        />
      ))}
      {cards.map((card) => (
        <ItemToggle
          key={`a-${card}`}
          name={`Action Card: ${card}`}
          label={`the action card ${card}`}
          card
          type={type}
          pressed={side.actionCard === card}
          testId={`${testId}-ac-${card}`}
          onToggle={() => set({ actionCard: side.actionCard === card ? null : card })}
        />
      ))}
      {secrets.map((secret) => (
        <ItemToggle
          key={`s-${secret}`}
          name={`Secret Objective: ${secret}`}
          label={`the secret objective ${secret}`}
          card
          type={type}
          pressed={side.secret === secret}
          testId={`${testId}-so-${secret}`}
          onToggle={() => set({ secret: side.secret === secret ? null : secret })}
        />
      ))}
      {fragments.map((trait) => (
        <ItemToggle
          key={`f-${trait}`}
          name={`Fragment (${trait})`}
          label={`a ${trait} relic fragment`}
          card={false}
          type={type}
          pressed={side.fragments.includes(trait)}
          testId={`${testId}-fr-${trait}`}
          onToggle={() =>
            set({ fragments: side.fragments.includes(trait) ? [] : [trait] })
          }
        />
      ))}
    </section>
  );
};

export interface TradeStagingDeskProps {
  deals: StagedDeal[];
  staged: Staged;
  onChange: (s: Staged) => void;
  partnerLabel: string;
  partnerFaction?: string | null;
  myHoldings?: Holdings | null;
  partnerHoldings?: Holdings | null;
  /** Shown above the columns, e.g. that a counter-offer was pre-filled. */
  banner?: React.ReactNode;
}

export const TradeStagingDesk: React.FC<TradeStagingDeskProps> = ({
  deals,
  staged,
  onChange,
  partnerLabel,
  partnerFaction,
  myHoldings,
  partnerHoldings,
  banner,
}) => {
  const limits = React.useMemo(() => stagingLimits(deals), [deals]);
  const matchId = stagedToDealId(staged, deals);
  const match = matchId ? (deals.find((d) => d.id === matchId) ?? null) : null;
  const suggestions = React.useMemo(
    () => (match || isStagedEmpty(staged) ? [] : nearestDeals(staged, deals, 3)),
    [match, staged, deals],
  );
  const partnerName = partnerFaction ? `${partnerLabel} (${partnerFaction})` : partnerLabel;

  return (
    <div className="trade-desk" data-testid="trade-staging-desk">
      {banner}
      <div className="trade-desk__columns">
        <Column
          title="You give"
          subtitle="Staged from what you hold"
          testId="stage-give"
          type="offer"
          side={staged.give}
          limits={limits.give}
          held={myHoldings}
          heldLabel="You hold"
          onChange={(give) => onChange({ ...staged, give })}
        />
        <Column
          title="You receive"
          subtitle={`From ${partnerName}`}
          testId="stage-receive"
          type="receive"
          side={staged.receive}
          limits={limits.receive}
          held={partnerHoldings}
          heldLabel="They hold"
          onChange={(receive) => onChange({ ...staged, receive })}
        />
      </div>

      <div className="trade-desk__status" role="status" aria-live="polite" data-testid="stage-status">
        {isStagedEmpty(staged) ? (
          <p data-testid="stage-status-empty">
            Stage what you give and what you want in return. The Propose button lights up when the
            combination is one the server lists.
          </p>
        ) : match ? (
          <div data-testid="stage-status-valid" className="trade-desk__valid">
            <strong>Ready to propose.</strong> {match.deal.label}
            {match.deal.net !== undefined && (
              <span
                className={`trade-summary__value trade-summary__value--${match.deal.net >= 0 ? "positive" : "negative"}`}
              >
                <span className="trade-summary__label">Net value (you):</span>{" "}
                <span className="trade-summary__amount">
                  {match.deal.net >= 0 ? `+${match.deal.net}` : match.deal.net}
                </span>
              </span>
            )}
          </div>
        ) : (
          <div data-testid="stage-status-invalid" className="trade-desk__invalid">
            <p>
              <strong>This combination isn&apos;t available right now.</strong>
            </p>
            <ul>
              {whyNoDeal(staged, deals).map((reason) => (
                <li key={reason}>{reason}</li>
              ))}
            </ul>
            {suggestions.length > 0 && (
              <>
                <p className="trade-desk__suggest-title">Nearest available deals:</p>
                <ul className="trade-desk__suggestions">
                  {suggestions.map((s) => (
                    <li key={s.id}>
                      <button
                        type="button"
                        className="button button--secondary trade-desk__suggestion"
                        data-testid={`stage-suggest-${s.id}`}
                        onClick={() => onChange(s.staged)}
                      >
                        {s.deal.label}
                      </button>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
};

/** The partner's offer in the same two columns, read-only (answer screen). */
export const TradeOfferColumns: React.FC<{
  staged: Staged;
  proposerLabel: string;
  net?: number;
}> = ({ staged, proposerLabel, net }) => {
  const side = (s: StagedSide, type: "offer" | "receive") => {
    const items: React.ReactNode[] = [];
    if (s.tradeGoods)
      items.push(
        <TradeItemDisplay key="tg" label={`${s.tradeGoods} trade goods`} type={type} />,
      );
    if (s.commodities)
      items.push(
        <TradeItemDisplay key="cm" label={`${s.commodities} commodities`} type={type} />,
      );
    if (s.fragments.length)
      items.push(
        <TradeItemDisplay key="fr" label={`${s.fragments.length} relic fragments`} type={type} />,
      );
    if (s.note)
      items.push(<TradeItemDisplay key="n" label={`Promissory Note: ${s.note}`} type={type} isCard />);
    if (s.actionCard)
      items.push(
        <TradeItemDisplay key="a" label={`Action Card: ${s.actionCard}`} type={type} isCard />,
      );
    if (s.secret)
      items.push(
        <TradeItemDisplay key="s" label={`Secret Objective: ${s.secret}`} type={type} isCard />,
      );
    if (!items.length) items.push(<p key="none" className="trade-column__none">Nothing.</p>);
    return items;
  };
  const give = sideLines(staged.give);
  const receive = sideLines(staged.receive);
  return (
    <div className="trade-desk trade-desk--answer" data-testid="trade-offer-columns">
      <div className="trade-desk__columns">
        <section
          className="trade-column trade-column--offer"
          data-testid="offer-give"
          aria-label="You give"
        >
          <h4 className="trade-section__title">You give</h4>
          <p className="trade-column__subtitle">What {proposerLabel} asks of you</p>
          {side(staged.give, "offer")}
        </section>
        <section
          className="trade-column trade-column--receive"
          data-testid="offer-receive"
          aria-label="You receive"
        >
          <h4 className="trade-section__title">You receive</h4>
          <p className="trade-column__subtitle">What {proposerLabel} gives you</p>
          {side(staged.receive, "receive")}
        </section>
      </div>
      <div className="trade-desk__status" data-testid="offer-net">
        <p>
          You give {give.length ? give.join(", ") : "nothing"}; you receive{" "}
          {receive.length ? receive.join(", ") : "nothing"}.
        </p>
        {net !== undefined && (
          <span
            className={`trade-summary__value trade-summary__value--${net >= 0 ? "positive" : "negative"}`}
          >
            <span className="trade-summary__label">Net value (you):</span>{" "}
            <span className="trade-summary__amount">{net >= 0 ? `+${net}` : net}</span>
          </span>
        )}
      </div>
    </div>
  );
};
