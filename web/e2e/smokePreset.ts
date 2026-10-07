/** Start presets the server knows (crates/ti4-server/src/preset.rs). */
export const KNOWN_PRESETS = ["combat", "cards", "agenda", "relics", "invasion"] as const;

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

/**
 * What a run must have offered. `TI4_SMOKE_EXPECT` is a comma list; each item is a decision
 * subtype, `a|b|c` (at least one of them) or `a|b|c>=N` (at least N distinct of them), and the
 * word `preset` stands for the start preset's own list (`PRESET_EXPECT`). Unset means no check.
 */
export interface Expectation {
  subtypes: string[];
  atLeast: number;
}

/** The subtypes each preset exists to reach; asserted when `TI4_SMOKE_EXPECT=preset`. */
export const PRESET_EXPECT: Record<string, string> = {
  // Thunder's Edge cards dealt to hands: their pick prompts, and the "action card played" window.
  cards:
    "overrule_pick_strategy card|strategize1_pick_strategy card|exchange_program_answer|exchangeprogram_pick_player|exchangeprogram_pick_planet>=2,reaction_when_ACTION_CARD_PLAYED",
  // The agenda phase ran and somebody voted; a window or pick of the dealt agenda cards fired.
  agenda:
    "cast_vote,play_reaction_after_AGENDA_REVEALED|predict_agenda_outcome|bribery_pick_count|assassin_pick_player>=2",
  // Relic prompts: purge to move, explore with the Crown, Neuraloop, JR-XS455-O, Heart of Ixth.
  relics:
    "crown_of_emphidia_choose_planet|dominus_orb_purge_to_move|neuraloop_choose_relic_to_purge|titan_prototype_choose_builder|stellar_converter_choose_target|heart_ixth_die_adjust>=3",
  // An invasion of a defended colony: landing and ground combat rounds. The two PDS's space cannon
  // usually costs the invader a sustain or a ship, but that is dice, so it is not asserted.
  // (assign_ground_casualty is never asked: the server's timing path assigns ground hits itself,
  // and bombardment_target is coexistence-only.)
  invasion: "commit_ground_forces,fight_ground_combat_round",
};

export function parseExpect(value: string | undefined, preset?: string): Expectation[] {
  const text = value?.trim();
  if (!text) return [];
  return text
    .split(",")
    .flatMap((item) =>
      item.trim() === "preset" ? (PRESET_EXPECT[preset ?? ""] ?? "").split(",") : [item],
    )
    .map((item) => item.trim())
    .filter(Boolean)
    .map((item) => {
      const match = /^(.*?)(?:>=(\d+))?$/.exec(item);
      return {
        subtypes: (match?.[1] ?? item).split("|").map((s) => s.trim()),
        atLeast: Number(match?.[2] ?? 1),
      };
    });
}

/** The expectations that `seen` (report.subtypes) does not meet, as readable text. */
export function missingExpected(seen: Record<string, number>, expected: Expectation[]): string[] {
  return expected.flatMap(({ subtypes, atLeast }) => {
    const got = subtypes.filter((s) => (seen[s] ?? 0) > 0).length;
    return got >= atLeast ? [] : [`${subtypes.join("|")} (need ${atLeast}, saw ${got})`];
  });
}
