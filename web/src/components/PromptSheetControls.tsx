import React from "react";
import type { PromptHolder } from "../presentation/pausablePrompts.ts";
import { offersOwnTurnMode, type PausablePrompt } from "../presentation/pausablePrompts.ts";
import { askSettingFor, usePausedPrompts } from "../hooks/promptSettings.ts";
import { openPromptsPanel } from "../hooks/promptsPanel.ts";
import { listedPrompts } from "./PromptsPanel.tsx";

/** Up to this many switchable abilities get a button each on the sheet; more share one Prompts button. */
export const MAX_SHEET_BUTTONS = 3;

const buttonStyle = (active: boolean): React.CSSProperties => ({
  background: active ? "rgba(148, 163, 184, 0.1)" : "transparent",
  border: "1px solid #94a3b8",
  color: "#94a3b8",
  borderRadius: 4,
  padding: "4px 8px",
  cursor: "pointer",
  fontSize: 12,
  fontWeight: "500",
});

const AskButton: React.FC<{ prompt: PausablePrompt }> = ({ prompt }) => {
  const { mode, cycle } = askSettingFor(prompt.id).useSetting();
  const withOwnTurn = offersOwnTurnMode(prompt);
  const ask = mode === "ask";
  return (
    <button
      type="button"
      data-testid={`${prompt.id}-ask-btn`}
      aria-pressed={ask}
      title={
        mode === "ownturn"
          ? `${prompt.label} (${prompt.when}) is asked only in your own turn and declined for you in other players' turns, in all games on this browser (the game still asks). Click to be asked every time.`
          : ask
          ? `You are asked about ${prompt.label} (${prompt.source}, ${prompt.when}). Click to skip it automatically (kept in this browser for all games until you switch it back on).`
          : `${prompt.label} is declined for you (${prompt.when}), in all games on this browser (the game still asks). ${withOwnTurn ? "Click to ask only in your own turn." : "Click to be asked again."}`
      }
      onClick={() => cycle(withOwnTurn)}
      style={buttonStyle(!ask)}
    >
      {mode === "ownturn" ? `${prompt.label} my turn only` : ask ? `${prompt.label} ask` : `${prompt.label} skipped`}
    </button>
  );
};

/**
 * The viewer's switchable abilities on the player sheet: a button each while there are few, one
 * "Prompts" button that opens the list when there are many. An ability appears for the seats that
 * have it and, whoever has it, while it is switched off.
 */
export const PromptSheetControls: React.FC<{ holder: PromptHolder | null | undefined }> = ({ holder }) => {
  const paused = usePausedPrompts();
  const listed = listedPrompts(holder, paused);
  if (listed.length === 0) return null;
  if (listed.length <= MAX_SHEET_BUTTONS) {
    return (
      <>
        {listed.map((prompt) => (
          <AskButton key={prompt.id} prompt={prompt} />
        ))}
      </>
    );
  }
  return (
    <button
      type="button"
      data-testid="prompts-btn"
      aria-haspopup="dialog"
      title="The optional abilities the game asks you about. Switch one to Skipped and it is declined for you, in all games on this browser, until you switch it back."
      onClick={openPromptsPanel}
      style={buttonStyle(paused.length > 0)}
    >
      {paused.length > 0 ? `Prompts (${paused.length} skipped)` : `Prompts (${listed.length})`}
    </button>
  );
};
