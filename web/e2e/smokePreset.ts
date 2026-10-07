/** Start presets the server knows (crates/ti4-server/src/preset.rs). */
export const KNOWN_PRESETS = ["combat"] as const;

/** Reads TI4_SMOKE_PRESET: unset or empty means a normal opening; an unknown name is an error. */
export function presetFromEnv(value: string | undefined): string | undefined {
  const name = value?.trim();
  if (!name) return undefined;
  if (!(KNOWN_PRESETS as readonly string[]).includes(name)) {
    throw new Error(`unknown TI4_SMOKE_PRESET "${name}" (known: ${KNOWN_PRESETS.join(", ")})`);
  }
  return name;
}

/** The POST /api/games body; `start_preset` is only sent when a preset was asked for. */
export function createGameBody(playerCount: number, seed: number, preset?: string) {
  return {
    player_count: playerCount,
    seed,
    nickname: "E2E Host",
    ...(preset ? { start_preset: preset } : {}),
  };
}
