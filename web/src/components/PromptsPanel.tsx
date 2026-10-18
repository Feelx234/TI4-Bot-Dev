import React from "react";
import { Dialog } from "../primitives/index.ts";
import { holdsPrompt, promptsHeldBy, PAUSABLE_PROMPTS, type PausablePrompt, type PromptHolder } from "../presentation/pausablePrompts.ts";
import { askSettingFor, usePausedPrompts } from "../hooks/promptSettings.ts";
import { closePromptsPanel, openPromptsPanel, usePromptsPanelOpen } from "../hooks/promptsPanel.ts";
import "./PromptsPanel.css";

/** The abilities to list for a seat: the ones it has, then any other that is switched off. */
export function listedPrompts(holder: PromptHolder | null | undefined, paused: readonly PausablePrompt[]): PausablePrompt[] {
  const held = promptsHeldBy(holder);
  return [...held, ...PAUSABLE_PROMPTS.filter((p) => paused.includes(p) && !held.includes(p))];
}

const Row: React.FC<{ prompt: PausablePrompt; mine: boolean }> = ({ prompt, mine }) => {
  const { ask, toggle } = askSettingFor(prompt.id).useSetting();
  return (
    <li className="prompts-panel__row" data-testid={`prompt-row-${prompt.id}`} data-paused={!ask || undefined}>
      <div className="prompts-panel__text">
        <div className="prompts-panel__name">{prompt.label}</div>
        <div className="prompts-panel__when">
          {prompt.source}, {prompt.when}
          {!mine && " (not yours in this game)"}
        </div>
      </div>
      <button
        type="button"
        className="prompts-panel__switch"
        data-testid={`prompt-toggle-${prompt.id}`}
        aria-pressed={!ask}
        aria-label={`${prompt.label}: ${ask ? "asked, press to skip" : "skipped, press to ask again"}`}
        onClick={toggle}
      >
        {ask ? "Ask" : "Skipped"}
      </button>
    </li>
  );
};

/** The viewer's optional-ability prompts with a switch each, opened from the sheet or the chip. */
export const PromptsPanel: React.FC<{ holder: PromptHolder | null | undefined }> = ({ holder }) => {
  const open = usePromptsPanelOpen();
  const paused = usePausedPrompts();
  const rows = listedPrompts(holder, paused);
  return (
    <Dialog.Root open={open} onOpenChange={(next) => (next ? openPromptsPanel() : closePromptsPanel())}>
      <Dialog.Content
        data-testid="prompts-panel"
        className="choice-workflow-dialog prompts-panel-dialog"
        onClick={(e) => {
          if (e.target === e.currentTarget) closePromptsPanel();
        }}
      >
        <div className="panel choice-workflow-modal prompts-panel">
          <header className="prompts-panel__header">
            <Dialog.Title as="h2" className="prompts-panel__title">
              Prompts
            </Dialog.Title>
            <button type="button" className="button button--secondary" data-testid="prompts-panel-close" onClick={closePromptsPanel}>
              Close
            </button>
          </header>
          <p className="prompts-panel__note">
            The game asks about each of these abilities every time it can be used. Set one to Skipped and this
            browser declines it for you, in all games on this browser, until you set it back to Ask. The game still
            asks, and an undo brings the question back.
          </p>
          {rows.length === 0 ? (
            <p className="prompts-panel__empty">Your faction has no ability that can be switched off.</p>
          ) : (
            <ul className="prompts-panel__list">
              {rows.map((prompt) => (
                <Row key={prompt.id} prompt={prompt} mine={holdsPrompt(prompt, holder)} />
              ))}
            </ul>
          )}
        </div>
      </Dialog.Content>
    </Dialog.Root>
  );
};
