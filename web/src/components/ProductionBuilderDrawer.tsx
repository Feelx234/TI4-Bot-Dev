import React, { useMemo } from 'react';
import { PendingChoiceDto } from '../protocol/types.ts';
import { Dialog } from '../primitives/index.ts';
import { ChoiceRendererModel } from '../presentation/choiceModel.ts';
import { WorkflowShell } from './WorkflowShell.tsx';
import { usePlayerIdentity } from '../presentation/PlayerIdentity.tsx';

export interface ProductionBuilderDrawerProps {
  choice: PendingChoiceDto | null;
  model?: ChoiceRendererModel | null;
  viewerSeat?: string | null;
  onSubmit: (optionId: string) => Promise<void>;
  isOpen: boolean;
  onClose: () => void;
  lastError?: string | null;
}

export const ProductionBuilderDrawer: React.FC<ProductionBuilderDrawerProps> = ({
  choice,
  model,
  viewerSeat,
  onSubmit,
  isOpen,
  onClose,
  lastError,
}) => {
  const display = usePlayerIdentity();
  const subtype = choice?.context?.subtype ?? '';
  const isPlaceUnit = subtype === 'place_unit';
  const isProduceUnit = subtype === 'produce_unit' || !isPlaceUnit;

  const productionOptions = useMemo(() => {
    if (!choice) return [];
    return choice.options.filter((o) => o.id !== 'decline' && o.kind !== 'decline');
  }, [choice]);

  const constraints = model?.outstanding?.[0] ?? choice?.context?.outstanding?.[0];
  const capacityLimit = model?.selectionMode.mode === 'production'
    ? model.selectionMode.capacity
    : (constraints?.amount ?? 0);
  const capacitySpent = constraints?.paid ?? 0;
  const capacityRemaining = Math.max(0, capacityLimit - capacitySpent);

  const systemId = model?.selectionMode.mode === 'production'
    ? model.selectionMode.systemId
    : (choice?.context?.target && 'System' in choice.context.target
      ? choice.context.target.System
      : '');

  if (!isOpen || !choice) return null;

  return (
    <Dialog.Root open={isOpen} onOpenChange={(open) => { if (!open) onClose(); }}>
      <Dialog.Content
        data-testid="production-builder-drawer"
        className="production-drawer"
      >
        {/* Header */}
        <div className="choice-workflow-header">
          <div>
            <div className="choice-workflow-eyebrow">
              Space Dock Production {systemId ? `• System ${systemId}` : ''}
            </div>
            <Dialog.Title
              as="h2"
              data-testid="production-drawer-title"
              className="choice-workflow-title"
            >
              {isPlaceUnit ? 'Place Produced Unit' : 'Unit Production Builder'}
            </Dialog.Title>
          </div>

          <button
            type="button"
            data-testid="close-production-drawer"
            onClick={onClose}
            aria-label="Close production builder"
            className="button button--secondary button--icon choice-workflow-close"
          >
            ✕
          </button>
        </div>

        <WorkflowShell
          choice={choice}
          model={model}
          viewerSeat={viewerSeat}
          onSubmit={onSubmit}
          lastError={lastError}
           spectatorNotice={`Observing unit production in progress for ${display(choice.actor).label}...`}
          spectatorNoticeTestId="spectator-production-notice"
          errorTestId="production-error-banner"
        >
          {({ isActor, isDirectSubmitting: isSubmitting, declineOption, submitDirect }) => <>

        {/* Produce Unit Mode */}
        {isActor && isProduceUnit && (
          <div className="workflow-stack">
            {/* Capacity Progress Meter */}
            {capacityLimit > 0 && (
              <div
                className="workflow-card production-drawer__meter"
              >
                <div className="workflow-card--row">
                  <span className="text-muted">Production Capacity:</span>
                  <span
                    data-testid="production-capacity-counter"
                    className="text-success"
                  >
                    {capacitySpent} / {capacityLimit} Units ({capacityRemaining} Left)
                  </span>
                </div>

                <progress className="production-drawer__progress" data-full={capacityRemaining === 0} max={capacityLimit} value={capacitySpent} />
              </div>
            )}

            <div className="workflow-copy">
              Select a unit to build, or click Done to proceed to payment:
            </div>

            {/* Units Grid */}
            <div
              data-testid="production-options-grid"
              className="production-drawer__grid"
            >
              {productionOptions.map((opt) => (
                <button
                  key={opt.id}
                  type="button"
                  data-testid={`produce-unit-btn-${opt.id}`}
                  onClick={() => submitDirect(opt.id)}
                  disabled={isSubmitting}
                  className="button button--secondary production-drawer__unit"
                >
                  <span>{opt.label}</span>
                </button>
              ))}
            </div>

            {/* Done Producing Action */}
            {declineOption && (
              <div>
                <button
                  type="button"
                  data-testid="done-producing-btn"
                   onClick={() => submitDirect(declineOption.id)}
                  disabled={isSubmitting}
                  className="button button--primary production-drawer__done"
                >
                  {declineOption.label || 'Done Producing'}
                </button>
              </div>
            )}
          </div>
        )}

        {/* Place Unit Mode */}
        {isActor && isPlaceUnit && (
          <div className="workflow-stack">
            <div className="workflow-copy">
              {choice.prompt}. Select destination location:
            </div>

            <div className="workflow-stack workflow-stack--compact">
              {productionOptions.map((opt) => (
                <button
                  key={opt.id}
                  type="button"
                  data-testid={`place-spot-btn-${opt.id}`}
                  onClick={() => submitDirect(opt.id)}
                  disabled={isSubmitting}
                  className="button button--secondary workflow-button--wide"
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>
        )}

          </>}
        </WorkflowShell>
      </Dialog.Content>
    </Dialog.Root>
  );
};
