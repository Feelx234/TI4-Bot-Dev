/**
 * The game that `?online` asks the server for: eight seats, this page has the first, the other
 * seats decide at random. `?online&seed=3&name=Ada&map=random`: each part can be left out.
 */
export interface OnlineStart {
  /** The name of the player of this page in the game. */
  nickname: string;
  /** Without it the server chooses one. */
  seed?: number;
  /** A map of the server (`GET /api/maps`), or "random". Without it: the map that the server starts eight seats on. */
  map?: string;
}

export const DEFAULT_NICKNAME = "Player";

export function onlineStart(params: URLSearchParams): OnlineStart {
  const seed = Number(params.get("seed") || Number.NaN);
  const map = params.get("map");
  return {
    nickname: params.get("name")?.trim() || DEFAULT_NICKNAME,
    ...(Number.isSafeInteger(seed) && seed >= 0 ? { seed } : {}),
    ...(map ? { map } : {}),
  };
}

/** The address that starts such a game. */
export function onlineAddress(start: Partial<OnlineStart> = {}): string {
  const params = new URLSearchParams();
  if (start.seed !== undefined) {
    params.set("seed", String(start.seed));
  }
  if (start.nickname && start.nickname !== DEFAULT_NICKNAME) {
    params.set("name", start.nickname);
  }
  if (start.map) {
    params.set("map", start.map);
  }
  const rest = params.toString();
  return rest ? `?online&${rest}` : "?online";
}

/** What `POST /api/games` is sent for it. */
export function createRequest(start: OnlineStart) {
  return {
    player_count: 8,
    nickname: start.nickname,
    fill_seats: "random",
    ...(start.seed === undefined ? {} : { seed: start.seed }),
    ...(start.map === undefined
      ? {}
      : start.map === "random"
        ? { map: "random" }
        : { map_template: start.map }),
  };
}
