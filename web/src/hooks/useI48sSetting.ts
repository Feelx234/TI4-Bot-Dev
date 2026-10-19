import { createAskSetting } from "./askSetting.ts";

export const ASK_I48S_KEY = "player_ask_i48s";

const setting = createAskSetting(ASK_I48S_KEY);

/** Default ON (ask every time): only an explicit "false" stops the I48S prompt from being shown. */
export const readAskI48s = setting.read;

/** Viewer-local switch: ask about the L1Z1X agent I48S after each activation, or decline for me. */
export const useI48sSetting = setting.useSetting;
