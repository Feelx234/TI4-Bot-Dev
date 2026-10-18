import React from "react";
import type { PendingChoiceDto } from "../protocol/types.ts";
import { promptsOfChoice, type PausablePrompt } from "../presentation/pausablePrompts.ts";
import { askSettingFor } from "../hooks/promptSettings.ts";

/** Under an ability's offer: switch the prompt off (it is then declined for you) or back on. */
export const PromptSwitch: React.FC<{ prompt: PausablePrompt }> = ({ prompt }) => {
  const { ask, toggle } = askSettingFor(prompt.id).useSetting();
  return (
    <label
      data-testid={`${prompt.id}-switch`}
      style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, margin: "8px 0", cursor: "pointer" }}
    >
      <input type="checkbox" data-testid={`${prompt.id}-switch-input`} checked={!ask} onChange={toggle} />
      <span>
        Don't ask about {prompt.label} ({prompt.when}); decline it for me until I switch it back on
      </span>
    </label>
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
