// The lobby of a table: the rows of the players with their signs, the setup, and the footer.
// The components decide nothing from names: what a row shows and which controls it has is here.
import type {
  LobbyButtonView,
  LobbyFooterView,
  LobbyRowView,
  LobbySetupRowView,
  LobbySignView,
  LobbyView,
} from "../../model";
import type { TableState } from "../table";
import type { Catalog, LobbySlot, LobbyState } from "../wire";
import { SEAT_PLACE, selectPreviewBoard } from "./board";
import { factionName, titleCase } from "./names";

/** What the page knows of the table besides the table itself. */
export interface LobbyLocal {
  /** The players that this browser has at the table, by id. */
  mine: string[];
  inspected: string | null;
  offline: boolean;
}

/** The kinds of a seat that the server plays in its own process, with the name of each. */
const BOT_KINDS: Record<string, { name: string; sign: LobbySignView["kind"]; about: string }> = {
  smart: {
    name: "Rule bot",
    sign: "rule",
    about: "The server plays this seat by rules: it builds, moves and scores.",
  },
  random: {
    name: "Random",
    sign: "random",
    about: "The server plays this seat at random, among what the game allows.",
  },
};
/** The order of the buttons of an open seat: the usual choice first. */
const KIND_ORDER = ["smart", "random"];

/**
 * The sets of strategy cards that a table can play (`ti4_server::card_set::OFFERED`). The names
 * come from the setup options of the server once it has them.
 */
const CARD_SETS: Record<string, string> = {
  te: "Thunder's Edge",
  pok: "Prophecy of Kings",
  base_game_codex1: "Base game with Codex I",
};

const plural = (count: number, word: string) => `${count} ${word}${count === 1 ? "" : "s"}`;

const kindsOf = (lobby: LobbyState) => KIND_ORDER.filter((kind) => lobby.bot_kinds.includes(kind));

function sign(slot: LobbySlot, lobby: LobbyState, you: boolean): LobbySignView | null {
  if (slot.occupant === null) {
    return { kind: "open", word: "Open seat" };
  }
  if (slot.bot) {
    const kind = BOT_KINDS[slot.bot];
    return kind
      ? { kind: kind.sign, word: `${kind.name}: played by the server` }
      : { kind: "random", word: "Played by the server" };
  }
  const host = slot.occupant === lobby.host_player_id;
  // The page shows its own table: its player is here, whatever the server said last.
  if (!slot.connected && !you) {
    return { kind: "away", word: host ? "Host · not connected" : "Not connected" };
  }
  return host ? { kind: "host", word: "Host" } : null;
}

function mapName(lobby: LobbyState): string {
  const { map } = lobby;
  const name = map.kind === "random" ? "Random" : (map.name ?? titleCase(map.alias ?? "Map"));
  return `${name} · ${plural(map.systems, "system")}${map.hyperlanes ? " · hyperlanes" : ""}`;
}

/** The name of a new player for another tab of the viewer: "Ada 2", "Ada 3". */
export function nextNickname(lobby: LobbyState, viewer: string): string {
  const taken = new Set(lobby.slots.map((slot) => slot.nickname));
  const own = lobby.slots.find((slot) => slot.occupant === viewer)?.nickname ?? "Player";
  const base = own.replace(/ \d+$/, "");
  for (let number = 2; ; number++) {
    // A nickname has 64 bytes at most: a long name gives room to its number.
    const name = `${base.slice(0, 56)} ${number}`;
    if (!taken.has(name)) {
      return name;
    }
  }
}

export function selectLobby(
  table: TableState,
  local: LobbyLocal,
  catalog: Catalog | null,
): LobbyView {
  const { lobby, viewer } = table;
  const host = viewer !== null && viewer === lobby.host_player_id;
  const seats = lobby.slots.length;
  const open = lobby.slots.filter((slot) => slot.occupant === null).length;
  const kinds = kindsOf(lobby);
  const hostName =
    lobby.slots.find((slot) => slot.occupant === lobby.host_player_id)?.nickname ?? "the host";

  let firstOpen = true;
  const rows = lobby.slots.map((slot): LobbyRowView => {
    const you = slot.occupant !== null && slot.occupant === viewer;
    const buttons: LobbyButtonView[] = [];
    if (slot.occupant === null) {
      if (host) {
        for (const kind of kinds) {
          buttons.push({ label: BOT_KINDS[kind].name, intent: { type: "addBot", kind } });
        }
      }
      // One row has it: every open seat is the same seat until the game starts.
      if (viewer !== null && firstOpen) {
        buttons.push({
          label: "Join in a new tab",
          title: "Take this seat as another player, in a new tab of this browser",
          intent: { type: "joinTab" },
        });
      }
      firstOpen = false;
    }
    const removable = host && slot.occupant !== null && !you;
    return {
      id: slot.slot_id,
      sign: sign(slot, lobby, you),
      name: slot.nickname ?? "Open",
      you,
      become:
        slot.occupant !== null && !you && local.mine.includes(slot.occupant)
          ? { type: "become", player: slot.occupant }
          : null,
      faction:
        slot.occupant === null ? "" : slot.faction ? factionName(slot.faction, catalog) : "Random",
      buttons,
      remove: removable
        ? {
            label: `Remove ${slot.nickname}`,
            intent: { type: "remove", player: slot.occupant! },
          }
        : null,
    };
  });

  const setup: LobbySetupRowView[] = [
    // The controls of the host come with the settings of a table: the server has no call yet.
    { id: "players", label: "Players", value: String(seats), editable: false },
    { id: "map", label: "Map", value: mapName(lobby), editable: false },
    {
      id: "cards",
      label: "Strategy cards",
      value: CARD_SETS[lobby.strategy_card_set] ?? titleCase(lobby.strategy_card_set),
      editable: false,
    },
  ];

  const waiting = lobby.slots.find(
    (slot) => slot.occupant !== null && !slot.bot && !slot.ready && slot.occupant !== viewer,
  );
  const blocked = open
    ? `Waiting for ${plural(open, "open seat")}`
    : waiting
      ? `Waiting for ${waiting.nickname} to be ready`
      : "";
  const footer: LobbyFooterView =
    viewer === null
      ? {
          note: open ? plural(open, "open seat") : "Every seat is taken",
          main: { label: "Join", disabled: open === 0, intent: { type: "join" } },
          nickname: open > 0,
        }
      : host
        ? {
            note: blocked,
            main: { label: "Start game", disabled: blocked !== "", intent: { type: "start" } },
            nickname: false,
          }
        : { note: blocked || `Waiting for ${hostName} to start`, main: null, nickname: false };

  const preview = table.preview;
  return {
    title: `Lobby · ${plural(seats, "player")}`,
    status: local.offline ? "Offline · reconnecting" : "",
    board: selectPreviewBoard(
      preview?.tiles ?? [],
      preview?.seats.map((seat) => seat.home_system_id) ?? [],
      local.inspected,
    ),
    seats: {
      [SEAT_PLACE]: { id: SEAT_PLACE, name: "", faction: "Seat", symbol: "●", color: "#8a94a6" },
    },
    setup,
    filled: `${seats - open} of ${seats}`,
    help: kinds.map((kind) => ({ title: BOT_KINDS[kind].name, text: BOT_KINDS[kind].about })),
    rows,
    fill:
      host && open > 0 && kinds.includes("smart")
        ? { label: "Fill with rule bots", intent: { type: "fillBots", kind: "smart" } }
        : null,
    footer,
  };
}
