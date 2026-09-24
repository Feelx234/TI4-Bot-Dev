import React, { useEffect, useState, useMemo, useRef } from "react";
import { GameLogEntry } from "../hooks/useGameSession.ts";
import { BoardView, HistoryStatus, PendingChoiceDto, PlayerView } from "../protocol/types.ts";
import { EventLog } from "./EventLog.tsx";
import { PendingChoiceModal } from "./PendingChoiceModal.tsx";
import { PaymentDrawer } from "./PaymentDrawer.tsx";
import { TacticalMovementOverlay } from "./TacticalMovementOverlay.tsx";
import { CombatResolutionModal } from "./CombatResolutionModal.tsx";
import { TradeDeskModal } from "./TradeDeskModal.tsx";
import { AgendaBallotModal } from "./AgendaBallotModal.tsx";
import { ReactionStatusBar } from "./ReactionStatusBar.tsx";
import { ProductionBuilderDrawer } from "./ProductionBuilderDrawer.tsx";
import { CargoLoadingTray } from "./CargoLoadingTray.tsx";
import { InvasionLandingTray } from "./InvasionLandingTray.tsx";
import { SystemActivationBar } from "./SystemActivationBar.tsx";
import { deriveChoiceRendererModel, ChoiceRendererModel } from "../presentation/choiceModel.ts";
import { Dialog, overlayStack } from "../primitives/index.ts";
import { useParticipantText } from "../presentation/PlayerIdentity.tsx";

export interface GameShellProps {
  header: React.ReactNode;
  board: React.ReactNode;
  playerSheet: React.ReactNode;
  detail?: React.ReactNode;
  events: GameLogEntry[];
  history?: HistoryStatus;
  onChangeHistory?: (action: "undo" | "redo" | { eventId: string }, steps?: number) => void;
  historyBusy?: boolean;
  choice: PendingChoiceDto | null;
  onSubmitChoice: (optionId: string) => Promise<void>;
  lastError?: string | null;
  selectedOptionId?: string;
  selectedSystemId?: string | null;
  onSelectOption?: (optionId: string) => void;
  viewerSeat?: string | null;
  players?: Record<string, PlayerView> | PlayerView[];
  boardView?: BoardView;
  productionQueue?: readonly string[];
  productionError?: string | null;
  onQueueProduction?: (units: string[]) => void;
}

export interface ChoiceRendererDispatcherProps {
  choice: PendingChoiceDto | null;
  model?: ChoiceRendererModel | null;
  viewerSeat?: string | null;
  onSubmit: (optionId: string) => Promise<void>;
  lastError?: string | null;
  selectedOptionId?: string;
  selectedSystemId?: string | null;
  onSelectOption?: (optionId: string) => void;
  isMinimized: boolean;
  onMinimizedChange: (minimized: boolean) => void;
  players?: Record<string, PlayerView>;
  boardView?: BoardView;
  productionQueue?: readonly string[];
  productionError?: string | null;
  onQueueProduction?: (units: string[]) => void;
}

type WorkflowRenderer = (
  props: Omit<ChoiceRendererDispatcherProps, "model"> & {
    choice: PendingChoiceDto;
    model: ChoiceRendererModel | null;
  },
) => React.ReactNode;

const workflowRenderers = new Map<ChoiceRendererModel["workflow"], WorkflowRenderer>([
  [
    "payment",
    ({
      choice,
      model,
      viewerSeat,
      players,
      onSubmit,
      isMinimized,
      onMinimizedChange,
      lastError,
      selectedOptionId,
    }) => (
      <PaymentDrawer
        choice={choice}
        model={model}
        viewerSeat={viewerSeat}
        player={players?.[choice.actor] ?? null}
        onSubmit={onSubmit}
        isOpen={!isMinimized}
        onClose={() => onMinimizedChange(true)}
        lastError={lastError}
        selectedOptionId={selectedOptionId}
      />
    ),
  ],
  [
    "tactical_movement",
    ({
      choice,
      model,
      viewerSeat,
      players,
      onSubmit,
      isMinimized,
      onMinimizedChange,
      lastError,
    }) => (
      <TacticalMovementOverlay
        choice={choice}
        model={model}
        viewerSeat={viewerSeat}
        activeSystemId={
          model?.selectionMode.mode === "tactical_move"
            ? model.selectionMode.activeSystem
            : choice.context?.target && "System" in choice.context.target
              ? choice.context.target.System
              : null
        }
        player={players?.[choice.actor] ?? null}
        onSubmit={onSubmit}
        isOpen={!isMinimized}
        onClose={() => onMinimizedChange(true)}
        lastError={lastError}
      />
    ),
  ],
  [
    "tactical_cargo",
    ({ choice, boardView, onSubmit, isMinimized, onMinimizedChange, lastError }) => (
      <CargoLoadingTray
        choice={choice}
        board={boardView}
        onSubmit={onSubmit}
        isOpen={!isMinimized}
        onClose={() => onMinimizedChange(true)}
        lastError={lastError}
      />
    ),
  ],
  ["combat_sustain", renderCombat],
  ["combat_casualty", renderCombat],
  ["combat_retreat", renderCombat],
  ["transaction_propose", renderTrade],
  ["transaction_answer", renderTrade],
  ["agenda_vote_outcome", renderAgenda],
  ["agenda_vote_planets", renderAgenda],
  [
    "action_card_reaction",
    ({ choice, model, viewerSeat, onSubmit, isMinimized, onMinimizedChange, lastError }) => (
      <ReactionStatusBar
        choice={choice}
        model={model}
        viewerSeat={viewerSeat}
        onSubmit={onSubmit}
        isOpen={!isMinimized}
        onClose={() => onMinimizedChange(true)}
        lastError={lastError}
      />
    ),
  ],
  [
    "production",
    ({
      choice,
      model,
      viewerSeat,
      onSubmit,
      isMinimized,
      onMinimizedChange,
      lastError,
      productionQueue,
      productionError,
      onQueueProduction,
    }) => (
      <ProductionBuilderDrawer
        choice={choice}
        model={model}
        viewerSeat={viewerSeat}
        onSubmit={onSubmit}
        isOpen={!isMinimized}
        onClose={() => onMinimizedChange(true)}
        lastError={productionError || lastError}
        queuedUnits={productionQueue}
        onQueueProduction={onQueueProduction}
      />
    ),
  ],
  ["generic_selection", renderGeneric],
  ["objective_scoring", renderGeneric],
  ["strategy_card_draft", renderGeneric],
  [
    "system_activation",
    ({
      choice,
      model,
      viewerSeat,
      selectedOptionId,
      selectedSystemId,
      onSelectOption,
      onSubmit,
      boardView,
      lastError,
    }) => (
      <SystemActivationBar
        choice={choice}
        model={model}
        viewerSeat={viewerSeat}
        selectedOptionId={selectedOptionId}
        selectedSystemId={selectedSystemId}
        onSelectOption={onSelectOption}
        onSubmit={onSubmit}
        boardView={boardView}
        lastError={lastError}
      />
    ),
  ],
  [
    "tactical_invasion",
    ({
      choice,
      boardView,
      viewerSeat,
      selectedOptionId,
      onSubmit,
      isMinimized,
      onMinimizedChange,
      lastError,
    }) =>
      !isMinimized && (
        <InvasionLandingTray
          choice={choice}
          board={boardView}
          viewerSeat={viewerSeat}
          selectedOptionId={selectedOptionId}
          onSubmit={onSubmit}
          onClose={() => onMinimizedChange(true)}
          lastError={lastError}
        />
      ),
  ],
]);

function renderCombat({
  choice,
  model,
  viewerSeat,
  onSubmit,
  isMinimized,
  onMinimizedChange,
  lastError,
}: Parameters<WorkflowRenderer>[0]) {
  return (
    <CombatResolutionModal
      choice={choice}
      model={model}
      viewerSeat={viewerSeat}
      onSubmit={onSubmit}
      isOpen={!isMinimized}
      onClose={() => onMinimizedChange(true)}
      lastError={lastError}
    />
  );
}

function renderTrade({
  choice,
  model,
  viewerSeat,
  onSubmit,
  isMinimized,
  onMinimizedChange,
  lastError,
}: Parameters<WorkflowRenderer>[0]) {
  return (
    <TradeDeskModal
      choice={choice}
      model={model}
      viewerSeat={viewerSeat}
      onSubmit={onSubmit}
      isOpen={!isMinimized}
      onClose={() => onMinimizedChange(true)}
      lastError={lastError}
    />
  );
}

function renderAgenda({
  choice,
  model,
  viewerSeat,
  onSubmit,
  isMinimized,
  onMinimizedChange,
  lastError,
  selectedOptionId,
}: Parameters<WorkflowRenderer>[0]) {
  return (
    <AgendaBallotModal
      choice={choice}
      model={model}
      viewerSeat={viewerSeat}
      onSubmit={onSubmit}
      isOpen={!isMinimized}
      onClose={() => onMinimizedChange(true)}
      lastError={lastError}
      selectedOptionId={selectedOptionId}
    />
  );
}

function renderGeneric({
  choice,
  model,
  onSubmit,
  lastError,
  isMinimized,
  onMinimizedChange,
  selectedOptionId,
  onSelectOption,
}: Parameters<WorkflowRenderer>[0]) {
  return (
    <PendingChoiceModal
      choice={choice}
      model={model}
      onSubmit={onSubmit}
      lastError={lastError}
      isMinimized={isMinimized}
      onMinimizedChange={onMinimizedChange}
      selectedOptionId={selectedOptionId}
      onSelectOption={onSelectOption}
    />
  );
}

export const ChoiceRendererDispatcher: React.FC<ChoiceRendererDispatcherProps> = ({
  choice,
  model: propModel,
  viewerSeat,
  onSubmit,
  lastError,
  selectedOptionId,
  selectedSystemId,
  onSelectOption,
  isMinimized,
  onMinimizedChange,
  players,
  boardView,
  productionQueue,
  productionError,
  onQueueProduction,
}) => {
  const present = useParticipantText();
  const derivedModel = useMemo(() => {
    return choice ? deriveChoiceRendererModel(choice, viewerSeat ?? null) : null;
  }, [choice, viewerSeat]);

  if (!choice || (viewerSeat !== undefined && choice.actor !== viewerSeat)) return null;

  // A view-only copy: IDs, payloads and the original pending choice stay intact.
  const visibleChoice = {
    ...choice,
    prompt: present(choice.prompt),
    options: choice.options.map((option) => ({
      ...option,
      label: present(option.label),
      description: option.description == null ? option.description : present(option.description),
    })),
  };

  const model = propModel ?? derivedModel;
  const workflow = model?.workflow ?? "generic_selection";
  const renderer = workflowRenderers.get(workflow) ?? workflowRenderers.get("generic_selection")!;
  const wrappedWorkflow =
    workflow === "payment" ||
    workflow === "tactical_movement" ||
    workflow === "tactical_cargo" ||
    workflow === "action_card_reaction";
  const content = renderer({
    choice: visibleChoice,
    model,
    viewerSeat,
    onSubmit,
    lastError: lastError ? present(lastError) : lastError,
    selectedOptionId,
    selectedSystemId,
    onSelectOption,
    isMinimized,
    onMinimizedChange,
    players,
    boardView,
    productionQueue,
    productionError,
    onQueueProduction,
  });

  return (
    <>
      {/* Minimized Decision Pill for dedicated drawers/modals */}
      {isMinimized &&
        ![
          "generic_selection",
          "objective_scoring",
          "strategy_card_draft",
          "system_activation",
        ].includes(workflow) && (
          <div className="choice-banner choice-minimized-pill" data-testid="choice-minimized-pill">
            <span className="choice-minimized-pill__prompt">{visibleChoice.prompt}</span>
            <button
              type="button"
              data-testid="resume-decision-btn"
              onClick={() => onMinimizedChange(false)}
              className="button button--primary button--sm"
            >
              Resume decision
            </button>
          </div>
        )}

      {wrappedWorkflow ? (
        <Dialog.Root open={!isMinimized} onOpenChange={(open) => onMinimizedChange(!open)}>
          <Dialog.Content
            keepMounted
            className="decision-modal choice-workflow-dialog"
            data-testid="decision-modal"
          >
            <div className="decision-modal__panel panel">
              <Dialog.Title as="h2" className="visually-hidden">
                {visibleChoice.prompt}
              </Dialog.Title>
              {content}
            </div>
          </Dialog.Content>
        </Dialog.Root>
      ) : (
        content
      )}
    </>
  );
};

export const GameShell: React.FC<GameShellProps> = ({
  header,
  board,
  playerSheet,
  detail,
  events,
  history,
  onChangeHistory,
  historyBusy,
  choice,
  onSubmitChoice,
  lastError,
  selectedOptionId,
  selectedSystemId,
  onSelectOption,
  viewerSeat,
  players,
  boardView,
}) => {
  const [openDrawer, setOpenDrawer] = useState<"events" | "players" | null>(null);
  const [isChoiceMinimized, setIsChoiceMinimized] = useState(false);
  const [productionQueue, setProductionQueue] = useState<{
    actor: string;
    system: string;
    units: string[];
  } | null>(null);
  const [productionError, setProductionError] = useState<string | null>(null);
  const submittedNonce = useRef<string | null>(null);
  const productionSubmitting = useRef(false);

  // A build may open payment and placement decisions before the next production offer.
  // Keep the queue above the workflow renderer and resume only on a fresh legal offer.
  useEffect(() => {
    if (
      !productionQueue?.units.length ||
      !choice ||
      productionSubmitting.current ||
      choice.nonce === submittedNonce.current
    )
      return;
    if (choice.context?.subtype !== "produce_unit") {
      if (choice.context?.subtype !== "pay_resources" && choice.context?.subtype !== "place_unit") {
        setProductionError("Production ended; remaining staged builds were not submitted.");
        setProductionQueue(null);
      }
      return;
    }
    const system =
      choice.context.target && "System" in choice.context.target
        ? choice.context.target.System
        : "";
    if (choice.actor !== productionQueue.actor || system !== productionQueue.system) {
      setProductionError("Production changed; remaining staged builds were not submitted.");
      setProductionQueue(null);
      return;
    }
    const unit = productionQueue.units[0];
    const matching = choice.options.filter(
      (candidate) =>
        candidate.kind !== "decline" && (candidate.payload?.unit === unit || candidate.id === unit),
    );
    if (matching.length !== 1) {
      setProductionError(
        `${unit} is no longer uniquely offered; remaining staged builds were not submitted.`,
      );
      setProductionQueue(null);
      return;
    }
    const option = matching[0];
    submittedNonce.current = choice.nonce;
    productionSubmitting.current = true;
    void onSubmitChoice(option.id)
      .then(() => {
        productionSubmitting.current = false;
        setProductionQueue((current) =>
          current && current.actor === choice.actor && current.system === system
            ? { ...current, units: current.units.slice(1) }
            : current,
        );
      })
      .catch((error: unknown) => {
        productionSubmitting.current = false;
        setProductionError(error instanceof Error ? error.message : String(error));
        setProductionQueue(null);
        submittedNonce.current = null;
      });
  }, [choice, productionQueue, onSubmitChoice]);

  const playersMap = React.useMemo<Record<string, PlayerView>>(() => {
    if (!players) return {};
    if (Array.isArray(players)) {
      return Object.fromEntries(players.map((p) => [p.id, p]));
    }
    return players;
  }, [players]);

  useEffect(() => {
    setIsChoiceMinimized(false);
  }, [choice?.nonce]);

  // Register open drawer in overlayStack for Escape dismissal
  useEffect(() => {
    if (!openDrawer) return;
    const unregister = overlayStack.register({
      id: `drawer-${openDrawer}`,
      modal: false,
      onDismiss: () => setOpenDrawer(null),
    });
    return unregister;
  }, [openDrawer]);

  return (
    <div data-testid="game-container" className="app-shell">
      <div className="app-shell__header">{header}</div>
      <div className="app-shell__content">
        <main className="app-shell__board">{board}</main>
        <aside
          id="player-sheet-drawer"
          data-testid="player-sheet-drawer"
          className={`app-shell__player-sheet${openDrawer === "players" ? " app-shell__drawer--open" : ""}`}
        >
          {playerSheet}
        </aside>
      </div>

      {detail}

      <div className="app-shell__mobile-actions" aria-label="Game panels">
        <button
          type="button"
          data-testid="player-sheet-toggle"
          className="button button--secondary"
          aria-expanded={openDrawer === "players"}
          aria-controls="player-sheet-drawer"
          onClick={() => setOpenDrawer((drawer) => (drawer === "players" ? null : "players"))}
        >
          Players
        </button>
        <button
          type="button"
          data-testid="event-log-mobile-toggle"
          className="button button--secondary"
          aria-expanded={openDrawer === "events"}
          aria-controls="event-log-drawer"
          onClick={() => setOpenDrawer((drawer) => (drawer === "events" ? null : "events"))}
        >
          Events
        </button>
      </div>

      <section
        id="event-log-drawer"
        className={`app-shell__event-log${openDrawer === "events" ? " app-shell__drawer--open" : ""}`}
        aria-label="Event log"
      >
        {onChangeHistory && (
          <div style={{ display: "flex", gap: 8, padding: "4px 12px" }}>
            <button
              type="button"
              className="button button--secondary button--sm"
              disabled={historyBusy || !history?.cursor}
              onClick={() => onChangeHistory("undo", 1)}
            >
              Undo
            </button>
            <button
              type="button"
              className="button button--secondary button--sm"
              disabled={historyBusy || !history?.redo_count}
              onClick={() => onChangeHistory("redo")}
            >
              Redo
            </button>
          </div>
        )}
        <EventLog
          events={events}
          cursor={history?.cursor}
          busy={historyBusy}
          onRestore={
            onChangeHistory ? (eventId, steps) => onChangeHistory({ eventId }, steps) : undefined
          }
          isOpen={openDrawer === "events"}
          onToggle={() => setOpenDrawer((drawer) => (drawer === "events" ? null : "events"))}
        />
      </section>

      <div className="app-shell__overlays">
        <ChoiceRendererDispatcher
          choice={choice}
          viewerSeat={viewerSeat}
          players={playersMap}
          boardView={boardView}
          onSubmit={onSubmitChoice}
          lastError={lastError}
          selectedOptionId={selectedOptionId}
          selectedSystemId={selectedSystemId}
          onSelectOption={onSelectOption}
          isMinimized={isChoiceMinimized}
          onMinimizedChange={setIsChoiceMinimized}
          productionQueue={productionQueue?.units}
          productionError={productionError}
          onQueueProduction={(units) => {
            if (!choice || productionQueue?.units.length) return;
            const system =
              choice.context?.target && "System" in choice.context.target
                ? choice.context.target.System
                : "";
            setProductionError(null);
            submittedNonce.current = null;
            setProductionQueue({ actor: choice.actor, system, units });
          }}
        />
      </div>
    </div>
  );
};
