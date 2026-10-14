import { useRef, useState } from "react";
import { type ImportOutcome, type OfflineSaveStore, type EngineIdentity, StoreError } from "./types.ts";

/**
 * "Import a game" button: a file picker that hands the chosen JSON file to `store.importGame`.
 * Not wired anywhere yet; T5/T2 put it into "My offline games". No styling beyond a button.
 */
export function ImportGameButton(props: {
  store: OfflineSaveStore;
  currentEngine?: EngineIdentity;
  onImported: (outcome: ImportOutcome) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const [problem, setProblem] = useState<string | null>(null);
  return (
    <>
      <button type="button" onClick={() => input.current?.click()}>
        Import a game
      </button>
      <input
        ref={input}
        type="file"
        accept="application/json,.json"
        hidden
        data-testid="import-game-file"
        onChange={async (e) => {
          const file = e.currentTarget.files?.[0];
          e.currentTarget.value = "";
          if (!file) return;
          setProblem(null);
          try {
            props.onImported(await props.store.importGame(file, props.currentEngine));
          } catch (err) {
            setProblem(err instanceof StoreError ? err.message : "The file could not be imported.");
          }
        }}
      />
      {problem && <p role="alert">{problem}</p>}
    </>
  );
}
