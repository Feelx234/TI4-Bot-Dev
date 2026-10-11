// The cards in the play area of a player: the faction abilities, the leaders, and the cards that
// the player uses. The engine says what a player has and what is exhausted; the catalogue has the
// printed names and texts, and says which cards are used.
import type { ComponentCardView, PlayerRowView } from "../../model";
import type { Catalog, PlayerView, PrintedCard, SessionUpdate } from "../wire";
import { titleCase } from "./names";

const KINDS = ["agent", "commander", "hero"];
const LEADER_STATES: Record<string, ComponentCardView["state"]> = {
  locked: "locked",
  readied: "ready",
  unlocked: "ready",
  exhausted: "exhausted",
  purged: "purged",
};

/** "ACTION:", "At the start of a ground combat round:": the timing as the cards print it. */
const timing = (window: string | undefined) =>
  !window || /[:.]$/.test(window) ? (window ?? "") : `${window}:`;
/** The corpus marks keywords for another renderer: "**STAR FORGE**". */
const plain = (text: string | undefined) => (text ?? "").replaceAll("*", "");

const byId = <T extends { id: string }>(cards: T[] | undefined) =>
  new Map((cards ?? []).map((card) => [card.id, card]));

export function componentCards(
  update: SessionUpdate,
  player: PlayerView,
  catalog: Catalog | null,
): Pick<PlayerRowView, "abilities" | "leaders" | "components"> {
  const printed = catalog?.components;
  const abilities = byId(printed?.abilities);
  const leaders = byId(printed?.leaders);
  // Without the catalogue the kind of a leader is the end of its id: "solagent".
  const kindOf = (id: string) =>
    leaders.get(id)?.kind ?? KINDS.find((kind) => id.endsWith(kind)) ?? "";

  /** A card that is used is ready or exhausted. A card that always holds has no state. */
  const used = (
    card: PrintedCard | undefined,
    id: string,
    kind: string,
    exhausted: string[] | undefined,
  ): ComponentCardView => ({
    name: card?.name ?? titleCase(id),
    kind,
    window: "",
    text: plain(card?.text),
    state: card && !card.used ? null : exhausted?.includes(id) ? "exhausted" : "ready",
    unlock: "",
  });

  const technologies = byId(printed?.technologies);
  const relics = byId(printed?.relics);
  const legendary = byId(printed?.legendary);
  const kept = byId(catalog?.exploration.cards);
  const labels = new Map(
    (update.view.board.map_tiles ?? []).flatMap((tile) =>
      (tile.planets ?? []).map((planet) => [planet.id, planet] as const),
    ),
  );
  const planets = Object.values(update.view.board.systems)
    .flatMap((system) => Object.values(system.planets))
    .filter((planet) => planet.controlled_by === player.id)
    .map((planet) => planet.planet_id)
    .filter((id) => labels.get(id)?.legendary);

  return {
    abilities: (printed?.factions[player.faction] ?? []).map((id) => {
      const card = abilities.get(id);
      return {
        name: card?.name ?? titleCase(id),
        kind: "Faction ability",
        window: timing(card?.window),
        text: plain(card?.text),
        state: null,
        unlock: "",
      };
    }),
    leaders: Object.entries(player.leaders)
      .sort(([a], [b]) => KINDS.indexOf(kindOf(a)) - KINDS.indexOf(kindOf(b)))
      .map(([id, status]) => {
        const card = leaders.get(id);
        const state = LEADER_STATES[status] ?? null;
        return {
          name: card?.name ?? titleCase(kindOf(id) || id),
          kind: [titleCase(kindOf(id)), card?.title].filter(Boolean).join(" · "),
          window: timing(card?.window),
          text: plain(card?.text),
          state,
          unlock: state === "locked" ? (card?.unlock ?? "") : "",
        };
      }),
    components: [
      // Without the catalogue no technology is known to be used: the sheet lists them all by id.
      ...player.technologies
        .filter((id) => technologies.get(id)?.used)
        .map((id) => used(technologies.get(id), id, "Technology", player.exhausted_technologies)),
      ...player.relics.map((id) => used(relics.get(id), id, "Relic", player.exhausted_relics)),
      ...(player.exploration_cards ?? []).map((id) => ({
        name: kept.get(id)?.name ?? titleCase(id),
        kind: "Exploration card",
        window: "",
        text: plain(kept.get(id)?.text),
        state: "ready" as const,
        unlock: "",
      })),
      ...planets.map((id) =>
        used(
          legendary.get(id),
          id,
          `Legendary planet · ${labels.get(id)?.label ?? titleCase(id)}`,
          player.exhausted_legendary,
        ),
      ),
    ],
  };
}
