import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { DemoApp } from "./dev/DemoApp";
import { Gallery } from "./dev/Gallery";
import { LocalApp, localGame } from "./dev/LocalApp";
import { OnlineApp } from "./dev/OnlineApp";
import { onlineStart } from "./dev/onlineStart";
import "./styles/theme.css";

const params = new URLSearchParams(location.search);
// `?online` starts a game on the server (with `seed`, `name` and `map` when the address has them),
// and `?online=<game_id>` is that game. Every other address is as before.
const online = params.has("online");
const local = online ? null : localGame(params);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {online ? (
      <OnlineApp gameId={params.get("online") || null} start={onlineStart(params)} />
    ) : local ? (
      <LocalApp game={local} />
    ) : params.has("gallery") ? (
      <Gallery />
    ) : (
      <DemoApp />
    )}
  </StrictMode>,
);
