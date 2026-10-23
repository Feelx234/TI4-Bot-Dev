import React from "react";
import { Dialog } from "../primitives/index.ts";
import type { AskMode } from "../hooks/askSetting.ts";
import { holdsPrompt, offersOwnTurnMode, promptsHeldBy, PAUSABLE_PROMPTS, type PausablePrompt, type PromptHolder } from "../presentation/pausablePrompts.ts";
import { askSettingFor, usePausedPrompts } from "../hooks/promptSettings.ts";
import { closePromptsPanel, openPromptsPanel, usePromptsPanelOpen } from "../hooks/promptsPanel.ts";
import "./PromptsPanel.css";

/** The abilities to list for a seat: the ones it has, then any other that is switched off. */
export function listedPrompts(holder: PromptHolder | null | undefined, paused: readonly PausablePrompt[]): PausablePrompt[] {
  const held = promptsHeldBy(holder);
  return [...held, ...PAUSABLE_PROMPTS.filter((p) => paused.includes(p) && !held.includes(p))];
}

const MODES: ReadonlyArray<readonly [AskMode, string, string]> = [
  ["ask", "Ask", "Ask every time"],
  ["ownturn", "My turn", "Ask only in your own turn; declined for you while another player has the turn"],
  ["skip", "Skip", "Never ask; declined for you"],
];

const Row: React.FC<{ prompt: PausablePrompt; mine: boolean }> = ({ prompt, mine }) => {
  const { ask, mode, setMode, toggle } = askSettingFor(prompt.id).useSetting();
  const withOwnTurn = offersOwnTurnMode(prompt);
  return (
    <li className="prompts-panel__row" data-testid={`prompt-row-${prompt.id}`} data-paused={mode !== "ask" || undefined}>
      <div className="prompts-panel__text">
        <div className="prompts-panel__name">{prompt.label}</div>
        <div className="prompts-panel__when">
          {prompt.source}, {prompt.when}
          {!mine && " (not yours in this game)"}
        </div>
      </div>
      {withOwnTurn ? (
        <div className="prompts-panel__modes" role="group" aria-label={`${prompt.label}: when to ask`}>
          {MODES.map(([value, text, hint]) => (
            <button
              key={value}
              type="button"
              className="prompts-panel__mode"
              data-testid={`prompt-mode-${prompt.id}-${value}`}
              aria-pressed={mode === value}
              title={hint}
              onClick={() => setMode(value)}
            >
              {text}
            </button>
          ))}
        </div>
      ) : (
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
      )}
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
            browser declines it for you, in all games on this browser, until you set it back to Ask. Where an ability
            also comes up in other players' turns, My turn asks only while you have the turn. The game still asks,
            and an undo brings the question back.
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
