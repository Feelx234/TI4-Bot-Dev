import { useEffect, useState } from "react";

/** What `GET /health` says about the running server. */
export interface BuildInfo {
  commit: string;
  protocolVersion: number | null;
  /** True on the public playtest server (`TI4_PUBLIC=1`): Alpha notice and Feedback button show. */
  isPublic: boolean;
}

const NONE: BuildInfo = { commit: "dev", protocolVersion: null, isPublic: false };
const STORED_KEY = "ti4.buildInfo.v1";
/** Longest wait for `/health` before the page goes on with what it last knew. */
const HEALTH_TIMEOUT_MS = 4000;

/** The last answer this browser got, so the Feedback button survives a reload without network. */
function lastKnown(): BuildInfo {
  try {
    const raw = JSON.parse(localStorage.getItem(STORED_KEY) ?? "null") as Partial<BuildInfo> | null;
    if (raw && typeof raw.commit === "string") {
      return {
        commit: raw.commit,
        protocolVersion: typeof raw.protocolVersion === "number" ? raw.protocolVersion : null,
        isPublic: raw.isPublic === true,
      };
    }
  } catch {
    // storage blocked or corrupt: nothing remembered
  }
  return NONE;
}

let cached: Promise<BuildInfo> | null = null;

/**
 * A failed lookup (offline, timeout) is not cached: the next call asks again, and meanwhile the
 * last answer this browser got is used, so a train tunnel does not hide the Feedback button.
 */
export function fetchBuildInfo(): Promise<BuildInfo> {
  cached ??= (async () => {
    const controller = typeof AbortController === "undefined" ? null : new AbortController();
    const timer = setTimeout(() => controller?.abort(), HEALTH_TIMEOUT_MS);
    try {
      const response = await fetch("/health", controller ? { signal: controller.signal } : undefined);
      if (!response.ok) throw new Error(`health ${response.status}`);
      const body = (await response.json()) as {
        commit?: string;
        protocol_version?: number;
        public?: boolean;
      };
      const info: BuildInfo = {
        commit: typeof body.commit === "string" ? body.commit : "dev",
        protocolVersion: typeof body.protocol_version === "number" ? body.protocol_version : null,
        isPublic: body.public === true,
      };
      try {
        localStorage.setItem(STORED_KEY, JSON.stringify(info));
      } catch {
        // not remembered
      }
      return info;
    } catch {
      cached = null; // ask again next time
      return lastKnown();
    } finally {
      clearTimeout(timer);
    }
  })();
  return cached;
}

export function resetBuildInfoForTests(): void {
  cached = null;
}

export function useBuildInfo(): BuildInfo {
  const [info, setInfo] = useState<BuildInfo>(NONE);
  useEffect(() => {
    let live = true;
    const load = () => void fetchBuildInfo().then((value) => live && setInfo(value));
    load();
    window.addEventListener("online", load);
    return () => {
      live = false;
      window.removeEventListener("online", load);
    };
  }, []);
  return info;
}
