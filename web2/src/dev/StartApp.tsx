import { useState } from "react";
import { StartScreen } from "../game/lobby/StartScreen";
import { NICKNAME_KEY } from "../session/onlineKeys";
import { SINGLE_PLAYER } from "./defaultGame";
import { DEFAULT_NICKNAME, onlineAddress } from "./onlineStart";

/**
 * The bare address: the choice between single player and multiplayer. "Single player" loads the
 * local game, with what it saved; "Multiplayer" makes a table on the server and opens its lobby.
 */
export function StartApp() {
  const [nickname, setNickname] = useState(
    () => localStorage.getItem(NICKNAME_KEY) ?? DEFAULT_NICKNAME,
  );
  return (
    <StartScreen
      rows={[
        { key: "1", label: "Single player", detail: "A game in this page", href: SINGLE_PLAYER },
        {
          key: "2",
          label: "Multiplayer",
          detail: "A table on the server",
          href: `/${onlineAddress()}`,
        },
      ]}
      nickname={nickname}
      onNickname={(name) => {
        setNickname(name);
        localStorage.setItem(NICKNAME_KEY, name);
      }}
    />
  );
}
