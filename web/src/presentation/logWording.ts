/**
 * Plain wording for event-log lines the host writes as "<seat> resolved <question>" because the
 * chosen option may be private. The line keeps the seat id first (the log swaps it for the
 * participant's name) and says what was asked, so a reader can follow an agenda that ended early.
 */
interface QuestionWording {
  /** The line, after the seat. */
  line: string;
  /** The rule behind it, as a hover title. */
  hint: string;
}

const RESOLVED = /^(\S+) resolved (.+)$/;

const QUESTIONS: Record<string, QuestionWording> = {
  // Four callers share this question: Committee Formation and the three ministry laws.
  "offer discard law": {
    line: "was asked whether to discard a law (Committee Formation or a ministry card)",
    hint:
      "Committee Formation: before an Elect Player agenda is voted on, its owner may discard it to " +
      "choose the elected player, and nobody votes on that agenda. A ministry law offers its own " +
      "discard effect. The log does not say which was chosen: the agenda result does.",
  },
  "committee elect": {
    line: "chose the elected player with Committee Formation (nobody votes on that agenda)",
    hint: "Committee Formation was discarded; its owner picks who the agenda elects.",
  },
  quash: {
    line: "was asked whether to use Quash on the agenda",
    hint: "Xxcha ability: spend 1 token from the strategy pool to discard the revealed agenda and reveal another; players vote on that one instead.",
  },
  "use quash": {
    line: "was asked whether to use Quash on the agenda",
    hint: "Xxcha ability: spend 1 token from the strategy pool to discard the revealed agenda and reveal another; players vote on that one instead.",
  },
  "use favor": {
    line: "was asked whether to play Political Favor on the agenda",
    hint: "Political Favor (Xxcha): discard the revealed agenda and reveal another; players vote on that one instead.",
  },
  "use secret": {
    line: "was asked whether to play Political Secret on the agenda",
    hint: "Political Secret: its owner cannot vote, play action cards or use faction abilities until this agenda is resolved.",
  },
};

/** `{ text, title }` for a "<seat> resolved <question>" line the table below knows; null otherwise. */
export function friendlyLogLine(detail: string): { text: string; title: string } | null {
  const match = RESOLVED.exec(detail.trim());
  if (!match) return null;
  const wording = QUESTIONS[match[2]];
  return wording ? { text: `${match[1]} ${wording.line}`, title: wording.hint } : null;
}
