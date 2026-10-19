import { createAskSetting } from "./askSetting.ts";

export const ASK_AETHERPASSAGE_KEY = "player_ask_aetherpassage";

const setting = createAskSetting(ASK_AETHERPASSAGE_KEY);

/** Default ON (ask every time): only an explicit "false" stops the Aetherpassage prompt. */
export const readAskAetherpassage = setting.read;

/** Viewer-local switch (all games, this browser): ask about Empyrean Aetherpassage, or decline for me. */
export const useAetherpassageSetting = setting.useSetting;
