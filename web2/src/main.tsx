import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { DemoApp } from "./dev/DemoApp";
import { Gallery } from "./dev/Gallery";
import { LocalApp, localGame } from "./dev/LocalApp";
import { OnlineApp } from "./dev/OnlineApp";
import "./styles/theme.css";

const params = new URLSearchParams(location.search);
// `?online` is a game on the server, and `?online=<game_id>` that game. Every other address is as before.
const online = params.has("online");
const local = online ? null : localGame(params);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {online ? (
      <OnlineApp gameId={params.get("online") || null} />
    ) : local ? (
      <LocalApp game={local} />
    ) : params.has("gallery") ? (
      <Gallery />
    ) : (
      <DemoApp />
    )}
  </StrictMode>,
);
