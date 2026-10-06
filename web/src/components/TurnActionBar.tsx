import React, { useCallback, useEffect, useRef, useState } from "react";
import { humanizeId } from "../protocol/contentCatalog.ts";
import {
  type BarAction,
  type BarInfo,
  type BarItem,
  type StrategicButton,
  type TradePartnerView,
  type TurnBarModel,
} from "../presentation/turnBar.ts";
import { usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";
import "./TurnActionBar.css";

export interface TurnActionBarProps {
  model: TurnBarModel;
  /** Send the engine option a button stands for. */
  onSubmit: (optionId: string) => Promise<void>;
  lastError?: string | null;
}

type MenuKey = "components" | "cards" | "trade";

const MAX_PIPS = 12;

const Pips: React.FC<{ count: number }> = ({ count }) => (
  <span className="turn-bar__pips" aria-hidden="true">
    {Array.from({ length: Math.min(Math.max(count, 0), MAX_PIPS) }, (_, i) => (
      <i key={i} />
    ))}
  </span>
);

const KEY_HINTS: [string, string][] = [
  ["T", "tactical"],
  ["S", "strategic"],
  ["C", "components"],
  ["A", "action cards"],
  ["D", "trade (deal)"],
  ["P", "pass"],
  ["Enter", "end turn when highlighted"],
  ["Esc", "close menu"],
];

function typingTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el || !el.tagName) return false;
  return (
    el.tagName === "INPUT" ||
    el.tagName === "TEXTAREA" ||
    el.tagName === "SELECT" ||
    el.isContentEditable
  );
}

interface BarButtonProps {
  barKey: string;
  testId: string;
  title: string;
  hint: string;
  reason: string | null;
  enabled: boolean;
  busy: boolean;
  kbd?: string;
  count?: number;
  primary?: boolean;
  glow?: boolean;
  ghost?: boolean;
  expanded?: boolean;
  infoValue?: BarInfo | null;
  onActivate: () => void;
  onInfo: (info: BarInfo | null) => void;
  extra?: string;
}

const BarButton: React.FC<BarButtonProps> = (p) => (
  <button
    type="button"
    className={[
      "turn-bar__btn",
      p.primary && "turn-bar__btn--primary",
      p.glow && "turn-bar__btn--glow",
      p.ghost && "turn-bar__btn--ghost",
      p.extra,
    ]
      .filter(Boolean)
      .join(" ")}
    data-testid={p.testId}
    data-bar-key={p.barKey}
    aria-disabled={!p.enabled || p.busy}
    aria-expanded={p.expanded === undefined ? undefined : p.expanded}
    aria-haspopup={p.expanded === undefined ? undefined : "true"}
    aria-keyshortcuts={p.kbd}
    title={p.enabled ? undefined : (p.reason ?? undefined)}
    onClick={() => {
      if (p.enabled && !p.busy) p.onActivate();
    }}
    onMouseEnter={() => p.infoValue && p.onInfo(p.infoValue)}
    onFocus={() => p.infoValue && p.onInfo(p.infoValue)}
    onMouseLeave={() => p.onInfo(null)}
    onBlur={() => p.onInfo(null)}
  >
    <span className="turn-bar__title">
      <span>{p.title}</span>
      {p.count !== undefined ? (
        <span className="turn-bar__count">{p.count}</span>
      ) : p.kbd ? (
        <kbd>{p.kbd}</kbd>
      ) : null}
    </span>
    <span className="turn-bar__sub">{p.enabled ? p.hint : (p.reason ?? p.hint)}</span>
  </button>
);

/**
 * The persistent action bar for the action phase. One bar that never moves: Tactical, one
 * Strategic button per held strategy card, Components, Action cards, Trade, Pass and End turn.
 * Disabled buttons stay in place and say why; card text appears in a side panel on hover or focus.
 * Letter keys jump to a button (menus open); Enter ends the turn when nothing else has focus.
 */
export const TurnActionBar: React.FC<TurnActionBarProps> = ({ model, onSubmit, lastError }) => {
  const display = usePlayerIdentity();
  const rootRef = useRef<HTMLElement | null>(null);
  const [open, setOpen] = useState<MenuKey | null>(null);
  const [info, setInfo] = useState<BarInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const readOnly = model.mode === "readonly";

  const submit = useCallback(
    async (optionId: string | null) => {
      if (!optionId || busy || readOnly) return;
      setBusy(true);
      setError(null);
      setOpen(null);
      setInfo(null);
      try {
        await onSubmit(optionId);
      } catch (err: unknown) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setBusy(false);
      }
    },
    [busy, onSubmit, readOnly],
  );

  const focusIn = (selector: string) => {
    const el = rootRef.current?.querySelector<HTMLElement>(selector);
    el?.focus();
  };

  const toggleMenu = (key: MenuKey) => setOpen((current) => (current === key ? null : key));

  useEffect(() => {
    if (readOnly) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.ctrlKey || event.metaKey || event.altKey || event.defaultPrevented) return;
      if (typingTarget(event.target)) return;
      // A dialog owns the keyboard; the bar must not act behind it.
      if (document.querySelector('[role="dialog"][aria-modal="true"]')) return;
      const key = event.key;
      if (key === "Escape") {
        if (open) {
          setOpen(null);
          setInfo(null);
          event.preventDefault();
        }
        return;
      }
      if (key === "Enter") {
        const active = document.activeElement as HTMLElement | null;
        const onControl = active && (active.tagName === "BUTTON" || active.tagName === "A");
        if (!onControl && model.end.enabled) {
          event.preventDefault();
          void submit(model.end.optionId);
        }
        return;
      }
      const lower = key.toLowerCase();
      const jump: Record<string, () => void> = {
        t: () => focusIn('[data-bar-key="tactical"]'),
        s: () =>
          focusIn(
            rootRef.current?.querySelector('[data-bar-key="strategic"]:not([aria-disabled="true"])')
              ? '[data-bar-key="strategic"]:not([aria-disabled="true"])'
              : '[data-bar-key="strategic"]',
          ),
        p: () => focusIn('[data-bar-key="pass"]'),
        c: () => {
          if (model.components.enabled) setOpen("components");
          else focusIn('[data-bar-key="components"]');
        },
        a: () => {
          if (model.actionCards.enabled) setOpen("cards");
          else focusIn('[data-bar-key="cards"]');
        },
        d: () => {
          if (model.trade.enabled) setOpen("trade");
          else focusIn('[data-bar-key="trade"]');
        },
      };
      const action = jump[lower];
      if (!action) return;
      event.preventDefault();
      action();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [model, open, readOnly, submit]);

  // The first row of a freshly opened menu takes focus so the arrows work at once.
  useEffect(() => {
    if (!open) return;
    const first = rootRef.current?.querySelector<HTMLElement>(
      `[data-bar-menu="${open}"] [data-bar-row]:not([aria-disabled="true"])`,
    );
    first?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onPointer = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(null);
        setInfo(null);
      }
    };
    document.addEventListener("mousedown", onPointer);
    return () => document.removeEventListener("mousedown", onPointer);
  }, [open]);

  const onMenuKey = (event: React.KeyboardEvent<HTMLElement>) => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    const rows = Array.from(
      event.currentTarget.querySelectorAll<HTMLElement>('[data-bar-row]:not([aria-disabled="true"])'),
    );
    if (rows.length === 0) return;
    const at = rows.indexOf(document.activeElement as HTMLElement);
    const next =
      event.key === "ArrowDown"
        ? (at + 1) % rows.length
        : at <= 0
          ? rows.length - 1
          : at - 1;
    rows[next].focus();
    event.preventDefault();
  };

  const infoHandlers = (value: BarInfo | null) => ({
    onMouseEnter: () => value && setInfo(value),
    onFocus: () => value && setInfo(value),
    onMouseLeave: () => setInfo(null),
    onBlur: () => setInfo(null),
  });

  const heading =
    model.mode === "readonly" && model.waitingFor
      ? `Waiting for ${display(model.waitingFor).label}`
      : model.heading;

  const actionBtn = (a: BarAction, kbd: string, extra?: Partial<BarButtonProps>) => (
    <BarButton
  busy={busy}
  onInfo={setInfo}
      barKey={a.key}
      testId={`turn-bar-${a.key}`}
      title={a.label}
      hint={a.hint}
      reason={a.reason}
      enabled={a.enabled}
      kbd={kbd}
      infoValue={a.info}
      onActivate={() => void submit(a.optionId)}
      {...extra}
    />
  );

  const strategicBtn = (s: StrategicButton, index: number) => (
    <BarButton
  busy={busy}
  onInfo={setInfo}
      key={`${s.cardId}-${index}`}
      barKey="strategic"
      testId={`turn-bar-strategic-${s.cardId || index}`}
      title={s.label}
      hint={s.cardName === s.label ? s.effect : `${s.cardName}: ${s.effect}`}
      reason={s.reason}
      enabled={s.enabled}
      kbd={index === 0 ? "S" : undefined}
      infoValue={s.info}
      extra={s.used ? "turn-bar__btn--used" : undefined}
      onActivate={() => void submit(s.optionId)}
    />
  );

  const row = (item: BarItem) => (
    <button
      key={item.optionId}
      type="button"
      role="menuitem"
      className="turn-bar__row"
      data-bar-row
      data-testid={`turn-bar-item-${item.optionId}`}
      onClick={() => void submit(item.optionId)}
      {...infoHandlers(item.info)}
    >
      <span className="turn-bar__row-label">{item.label}</span>
      {item.detail && <span className="turn-bar__row-detail">{item.detail}</span>}
    </button>
  );

  const partnerName = (p: TradePartnerView) => {
    const seat = p.seat ? display(p.seat) : null;
    return { name: humanizeId(p.faction), color: seat?.color ?? "#94a3b8", who: seat?.label ?? null };
  };

  const holdings = (p: TradePartnerView): string[] => {
    const out: string[] = [];
    if (p.commodities !== null) out.push(`${p.commodities} commodities`);
    if (p.tradeGoods !== null) out.push(`${p.tradeGoods} trade goods`);
    if (p.promissoryNotes !== null)
      out.push(`${p.promissoryNotes} promissory note${p.promissoryNotes === 1 ? "" : "s"}`);
    return out;
  };

  const tokens = model.tokens;
  const errorText = error ?? lastError ?? null;
  const tradeSub =
    model.trade.partners.length > 0
      ? `${model.trade.partners.length} partners · ${model.trade.available} in contact`
      : "";
  const cardWord = model.strategic.length > 1 ? "strategic cards" : "strategic card";

  return (
    <section
      ref={rootRef}
      className="turn-bar"
      data-testid="turn-action-bar"
      data-mode={model.mode}
      aria-label="Turn actions"
    >
      {info && (
        <aside className="turn-bar__info" data-testid="turn-bar-info" aria-live="polite">
          <div className="turn-bar__info-kind">{info.kind}</div>
          <h3>{info.title}</h3>
          {info.text && <p>{info.text}</p>}
          {info.secondary && <p className="turn-bar__info-secondary">{info.secondary}</p>}
          {info.chips.length > 0 && (
            <div className="turn-bar__chips">
              {info.chips.map((chip) => (
                <span key={chip.label} className={`turn-bar__chip turn-bar__chip--${chip.tone}`}>
                  {chip.label}
                </span>
              ))}
            </div>
          )}
        </aside>
      )}

      <div className="turn-bar__head">
        <strong data-testid="turn-bar-heading">{heading}</strong>
        {tokens && (
          <span className="turn-bar__pools" data-testid="turn-bar-pools">
            <span className="turn-bar__pool" data-testid="turn-bar-pool-tactic">
              Tactic <Pips count={tokens.tactic} /> <b>{tokens.tactic}</b>
            </span>
            <span className="turn-bar__pool" data-testid="turn-bar-pool-fleet">
              Fleet <Pips count={tokens.fleet} /> <b>{tokens.fleet}</b>
            </span>
            <span className="turn-bar__pool" data-testid="turn-bar-pool-strategy">
              Strategy <Pips count={tokens.strategy} /> <b>{tokens.strategy}</b>
            </span>
          </span>
        )}
      </div>

      <div className="turn-bar__actions" role="group" aria-label={`Actions: ${cardWord}`}>
        {actionBtn(model.tactical, "T")}
        {model.strategic.length === 0 ? (
          <BarButton
  busy={busy}
  onInfo={setInfo}
            barKey="strategic"
            testId="turn-bar-strategic-none"
            title="Strategic action"
            hint=""
            reason={readOnly ? model.tactical.reason : "No strategy card"}
            enabled={false}
            onActivate={() => undefined}
          />
        ) : (
          model.strategic.map(strategicBtn)
        )}

        <div className="turn-bar__wrap">
          <BarButton
  busy={busy}
  onInfo={setInfo}
            barKey="components"
            testId="turn-bar-components"
            title="Components"
            hint={`${model.components.count} usable`}
            reason={model.components.reason}
            enabled={model.components.enabled}
            kbd="C"
            expanded={open === "components"}
            onActivate={() => toggleMenu("components")}
          />
          {open === "components" && (
            <div
              className="turn-bar__menu"
              role="menu"
              aria-label="Components"
              data-bar-menu="components"
              data-testid="turn-bar-menu-components"
              onKeyDown={onMenuKey}
            >
              {model.components.groups.map((group) => (
                <div key={group.title} role="group" aria-label={group.title}>
                  <h4>{group.title}</h4>
                  {group.items.map(row)}
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="turn-bar__wrap">
          <BarButton
  busy={busy}
  onInfo={setInfo}
            barKey="cards"
            testId="turn-bar-cards"
            title="Action cards"
            hint={`${model.actionCards.count} playable now`}
            reason={model.actionCards.reason}
            enabled={model.actionCards.enabled}
            kbd="A"
            count={model.actionCards.count}
            expanded={open === "cards"}
            onActivate={() => toggleMenu("cards")}
          />
          {open === "cards" && (
            <div
              className="turn-bar__menu"
              role="menu"
              aria-label="Action cards"
              data-bar-menu="cards"
              data-testid="turn-bar-menu-cards"
              onKeyDown={onMenuKey}
            >
              <h4>Action cards with an Action window</h4>
              {model.actionCards.items.map(row)}
            </div>
          )}
        </div>

        <div className="turn-bar__wrap">
          <BarButton
  busy={busy}
  onInfo={setInfo}
            barKey="trade"
            testId="turn-bar-trade"
            title="Trade"
            hint={tradeSub}
            reason={model.trade.reason}
            enabled={model.trade.enabled}
            kbd="D"
            expanded={open === "trade"}
            onActivate={() => toggleMenu("trade")}
          />
          {open === "trade" && (
            <div
              className="turn-bar__menu turn-bar__menu--trade"
              role="dialog"
              aria-label="Open a transaction"
              data-bar-menu="trade"
              data-testid="turn-bar-menu-trade"
              onKeyDown={onMenuKey}
            >
              <h4>Open a transaction with&hellip;</h4>
              {model.trade.partners.map((p) => {
                const who = partnerName(p);
                return (
                  <div
                    key={p.faction}
                    className="turn-bar__partner"
                    data-testid={`turn-bar-partner-${p.faction}`}
                    data-available={p.available}
                  >
                    <div className="turn-bar__partner-name">
                      <i style={{ background: who.color }} aria-hidden="true" />
                      <span>{who.name}</span>
                      {who.who && <span className="turn-bar__partner-who">{who.who}</span>}
                      <span
                        className={`turn-bar__chip turn-bar__chip--${p.available ? "ok" : "bad"}`}
                      >
                        {p.available
                          ? p.inContact === false
                            ? "Can trade"
                            : "In contact"
                          : (p.reason ?? "Not available")}
                      </span>
                    </div>
                    <button
                      type="button"
                      className="turn-bar__open"
                      data-bar-row
                      data-testid={`turn-bar-open-${p.faction}`}
                      aria-disabled={!p.available}
                      aria-label={`Open a transaction with ${who.name}`}
                      onClick={() => {
                        if (p.available) void submit(p.optionId);
                      }}
                    >
                      Open
                    </button>
                    <div className="turn-bar__partner-what">
                      {holdings(p).map((text) => (
                        <span key={text} className="turn-bar__chip">
                          {text}
                        </span>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        <span className="turn-bar__spacer" />
        {actionBtn(model.pass, "P", { ghost: true, extra: "turn-bar__btn--pass" })}
        {actionBtn(model.end, "Enter", {
          primary: model.end.enabled,
          glow: model.end.enabled,
        })}
      </div>

      {errorText && (
        <div className="turn-bar__error" role="alert" data-testid="turn-bar-error">
          {errorText}
        </div>
      )}

      {!readOnly && (
        <div className="turn-bar__hints" aria-label="Keyboard shortcuts">
          {KEY_HINTS.map(([key, text]) => (
            <span key={key}>
              <kbd>{key}</kbd> {text}
            </span>
          ))}
        </div>
      )}
    </section>
  );
};
