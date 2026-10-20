import React, { useEffect, useId, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { PlayerView, TableView } from "../protocol/types.ts";
import { overlayStack } from "../primitives/index.ts";
import { usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";
import { usePassStatus } from "../presentation/passOrder.tsx";
import { usePublicState } from "../presentation/PublicStateContext.tsx";
import { buildScoreModel, GROUP_TITLES, type ScoreGroup, type ScoreRow, type SeatScore } from "../presentation/scoreBreakdown.ts";
import "./ScoreBreakdownDialog.css";

const sign = (n: number) => (n > 0 ? `+${n}` : String(n));

/** The rows of one group, with the group's subtotal in the heading. */
const Group: React.FC<{ group: ScoreGroup; rows: ScoreRow[] }> = ({ group, rows }) => {
  const subtotal = rows.reduce((sum, row) => sum + row.points, 0);
  return (
    <section className="score-group" data-testid={`score-group-${group}`}>
      <h3 className="score-group__title">
        <span>{GROUP_TITLES[group]}</span>
        <span>{sign(subtotal)}</span>
      </h3>
      <ul className="score-group__rows">
        {rows.map((row) => (
          <li key={row.key} className="score-row" data-testid={`score-row-${row.key}`}>
            <span className="score-row__label">
              {row.label}
              {row.detail && <small className="score-row__detail">{row.detail}</small>}
            </span>
            <strong className="score-row__points">{sign(row.points)}</strong>
          </li>
        ))}
      </ul>
    </section>
  );
};

const SeatBody: React.FC<{ seat: SeatScore; target: number; label: (id: string) => string; isViewer: boolean }> = ({
  seat,
  target,
  label,
  isViewer,
}) => {
  const groups = useMemo(() => {
    const out = new Map<ScoreGroup, ScoreRow[]>();
    for (const row of seat.rows) out.set(row.group, [...(out.get(row.group) ?? []), row]);
    return [...out];
  }, [seat.rows]);
  const sum = seat.rows.reduce((n, row) => n + row.points, 0);
  const pct = Math.min(100, Math.round((seat.total / target) * 100));
  return (
    <>
      <div className="score-summary" data-testid="score-summary">
        <div className="score-summary__head">
          <strong className="score-summary__total" data-testid="score-total">
            {seat.total}
            <small> / {target} VP</small>
          </strong>
          <span className="score-summary__note">
            {seat.total >= target
              ? "Target reached"
              : seat.isLeader
                ? `Leading · ${seat.toWin} to win`
                : `${seat.behind} behind the lead · ${seat.toWin} to win`}
          </span>
        </div>
        <div
          className="score-bar"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={target}
          aria-valuenow={seat.total}
          aria-label="Points toward the win"
        >
          <span className="score-bar__fill" style={{ width: `${pct}%` }} />
        </div>
      </div>
      {groups.length === 0 ? (
        <p className="score-empty">No points yet.</p>
      ) : (
        groups.map(([group, rows]) => <Group key={group} group={group} rows={rows} />)
      )}
      <div className="score-row score-row--total" data-testid="score-sum" data-matches={sum === seat.total ? "true" : "false"}>
        <span className="score-row__label">Total</span>
        <strong className="score-row__points">{seat.total}</strong>
      </div>
      <section className="score-group score-group--open" data-testid="score-open">
        <h3 className="score-group__title">
          <span>Public objectives not scored yet</span>
          <span>{seat.open.length}</span>
        </h3>
        {seat.open.length === 0 ? (
          <p className="score-empty">None revealed, or all scored.</p>
        ) : (
          <ul className="score-group__rows">
            {seat.open.map((o) => (
              <li key={o.id} className="score-row" data-testid={`score-open-${o.id}`}>
                <span className="score-row__label">
                  {o.name}
                  <small className="score-row__detail">
                    {o.points >= 2 ? "Stage II" : "Stage I"}
                    {o.scoredBy.length > 0 ? ` · scored by ${o.scoredBy.map(label).join(", ")}` : " · nobody yet"}
                  </small>
                </span>
                <strong className="score-row__points">{sign(o.points)}</strong>
              </li>
            ))}
          </ul>
        )}
        <p className="score-foot">
          Revealed objectives this seat has not scored; the conditions are not checked here.
          {isViewer && seat.secretsHeld > 0 ? ` You hold ${seat.secretsHeld} unscored secret ${seat.secretsHeld === 1 ? "objective" : "objectives"}.` : ""}
        </p>
      </section>
    </>
  );
};

/**
 * Where each seat's victory points come from: a tab per seat (one tap to switch, sticky on a
 * phone) and, for the chosen seat, its points by source with the total the engine reports, the way
 * to the target and the public objectives still open. A full-height sheet on a phone, a docked
 * panel on desktop. Read-only; public information only.
 */
export const ScoreBreakdownDialog: React.FC<{
  players: readonly PlayerView[];
  table?: TableView;
  viewer?: string;
  initialSeat?: string;
  onClose: () => void;
}> = ({ players, table, viewer, initialSeat, onClose }) => {
  const display = usePlayerIdentity();
  const passStatus = usePassStatus();
  const { score } = usePublicState();
  const [seatId, setSeatId] = useState(initialSeat ?? viewer ?? players[0]?.id ?? "");
  const ref = useRef<HTMLElement | null>(null);
  const titleId = useId();
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current?.focus();
    const unregister = overlayStack.register({ id: "score-breakdown", modal: false, onDismiss: onClose, element: ref.current });
    return () => {
      unregister();
      previous?.focus?.();
    };
  }, [onClose]);
  const label = React.useCallback((id: string) => display(id).label, [display]);
  const model = useMemo(() => buildScoreModel({ players, table, score, label }), [players, table, score, label]);
  const seat = model.seats.find((s) => s.seat === seatId) ?? model.seats[0];
  if (!seat) return null;
  const pass = passStatus(seat.seat);
  const identity = display(seat.seat);
  return createPortal(
    <section
      ref={ref}
      role="dialog"
      aria-modal="false"
      aria-labelledby={titleId}
      tabIndex={-1}
      className="score-dialog"
      data-testid="score-dialog"
    >
      <header className="score-dialog__header">
        <div className="score-dialog__top">
          <h2 id={titleId} className="score-dialog__title">
            Victory points
            <small> first to {model.target} wins</small>
          </h2>
          <button type="button" className="button button--secondary score-dialog__close" data-testid="score-close" onClick={onClose}>
            Close
          </button>
        </div>
        <div className="score-tabs" role="tablist" aria-label="Whose points">
          {model.seats.map((s) => {
            const id = display(s.seat);
            const passed = passStatus(s.seat).passed;
            return (
              <button
                key={s.seat}
                type="button"
                role="tab"
                aria-selected={s.seat === seat.seat}
                data-testid={`score-tab-${s.seat}`}
                data-leader={s.isLeader ? "true" : "false"}
                className="score-tab"
                style={{ borderColor: id.color }}
                onClick={() => setSeatId(s.seat)}
              >
                <span className="score-tab__name">
                  {id.label}
                  {s.seat === viewer && !/^you\b/i.test(id.label) ? " (you)" : ""}
                </span>
                <span className="score-tab__vp">
                  {s.isLeader ? "★ " : ""}
                  {s.total}
                  {passed ? " · passed" : ""}
                </span>
              </button>
            );
          })}
        </div>
        <div className="score-dialog__seat" style={{ borderColor: identity.color }}>
          <strong>{identity.label}</strong>
          <span className="score-dialog__chips">
            {seat.isLeader && <span className="score-chip score-chip--leader" data-testid="score-leader">Leader</span>}
            {pass.passed && (
              <span className="passed-chip" data-testid="score-passed">
                {pass.label}
              </span>
            )}
          </span>
        </div>
      </header>
      <div className="score-dialog__scroll">
        <SeatBody seat={seat} target={model.target} label={label} isViewer={seat.seat === viewer} />
      </div>
    </section>,
    document.body,
  );
};
