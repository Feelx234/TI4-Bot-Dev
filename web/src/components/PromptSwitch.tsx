import React from "react";
import type { PendingChoiceDto } from "../protocol/types.ts";
import { offersOwnTurnMode, promptsOfChoice, type PausablePrompt } from "../presentation/pausablePrompts.ts";
import { askSettingFor } from "../hooks/promptSettings.ts";

const labelStyle: React.CSSProperties = { display: "flex", alignItems: "center", gap: 8, fontSize: 13, margin: "8px 0", cursor: "pointer" };

/**
 * Under an ability's offer: switch the prompt off (it is then declined for you) or back on, and,
 * for an ability that also comes up in other players' turns, ask about it only in your own turn.
 */
export const PromptSwitch: React.FC<{ prompt: PausablePrompt }> = ({ prompt }) => {
  const { mode, setMode } = askSettingFor(prompt.id).useSetting();
  return (
    <>
      <label data-testid={`${prompt.id}-switch`} style={labelStyle}>
        <input
          type="checkbox"
          data-testid={`${prompt.id}-switch-input`}
          checked={mode === "skip"}
          onChange={() => setMode(mode === "skip" ? "ask" : "skip")}
        />
        <span>
          Don't ask about {prompt.label} ({prompt.when}); decline it for me until I switch it back on
        </span>
      </label>
      {offersOwnTurnMode(prompt) && (
        <label data-testid={`${prompt.id}-ownturn`} style={labelStyle}>
          <input
            type="checkbox"
            data-testid={`${prompt.id}-ownturn-input`}
            checked={mode === "ownturn"}
            onChange={() => setMode(mode === "ownturn" ? "ask" : "ownturn")}
          />
          <span>Only ask about {prompt.label} in my own turn; decline it for me in other players' turns</span>
        </label>
      )}
    </>
  );
};

/** One switch for every registered ability the question is about (none for other questions). */
export const PromptSwitches: React.FC<{ choice: PendingChoiceDto | null | undefined }> = ({ choice }) => (
  <>
    {promptsOfChoice(choice).map((prompt) => (
      <PromptSwitch key={prompt.id} prompt={prompt} />
    ))}
  </>
);
