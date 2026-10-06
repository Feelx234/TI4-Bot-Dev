// Re-exports of the app's synthetic gallery fixtures, so captures share one realistic board.
export { galleryBoard, galleryPlayers, hitAssignmentBoard } from "../../../src/dev/galleryBoard";
export { actor } from "../../../src/dev/decisionGalleryCases";
export { galleryEventLog } from "../../../src/dev/galleryEventLog";

/** An agenda card as the engine puts it into a voting decision's context details. */
export const agendaCard = {
  name: "Sling Relay",
  yes_outcome: "All players may move their ships in non-home systems.",
  no_outcome: "Players cannot move ships during this agenda phase.",
};

import { galleryCases } from "../../../src/dev/decisionGalleryCases";

/** The gallery's activation decision: about thirty systems the player may activate. */
export const systemActivationOptions = galleryCases.find((c) => c.workflow === "system_activation")!.choice.options;

import { fallbackCases } from "../../../src/dev/decisionGalleryCases";

const galleryChoice = (title: string) =>
  [...galleryCases, ...fallbackCases].find((c) => c.title === title)!.choice;

/** Decisions from the dev gallery, as { prompt, context, options } ready for openMockedGame. */
export const galleryDecision = (title: string) => {
  const { prompt, context, options } = galleryChoice(title);
  return { prompt, context: context as Record<string, unknown>, options };
};
