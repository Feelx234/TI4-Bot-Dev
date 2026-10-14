import { engineHashOf, engineUrl } from "./engineAsset.ts";
import type { FromWorker, HostPort, ToWorker } from "./hostProtocol.ts";
import { LocalTransport, type LocalTransportOptions } from "./LocalTransport.ts";

/**
 * The lazily loaded half of the launcher: the engine file, the Web Worker and the transport.
 * Nothing here is part of the initial bundle; `launcher.ts` imports it when a game is opened.
 */

export interface LoadedEngine {
  module: WebAssembly.Module;
  /** Content hash of the engine file's name. */
  hash: string;
}

let loading: Promise<LoadedEngine> | null = null;

/** Fetches and compiles the engine once per page (off the main thread). */
export function loadEngine(): Promise<LoadedEngine> {
  loading ??= (async () => {
    const url = await engineUrl();
    const module = await compile(url);
    return { module, hash: engineHashOf(url) };
  })();
  loading.catch(() => {
    loading = null;
  });
  return loading;
}

async function compile(url: string): Promise<WebAssembly.Module> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`The engine could not be loaded (${response.status})`);
  // compileStreaming needs the wasm content type; a static host may not send it.
  if (response.headers.get("content-type")?.startsWith("application/wasm"))
    return WebAssembly.compileStreaming(response);
  return WebAssembly.compile(await response.arrayBuffer());
}

/** A new Web Worker with the engine module handed in; resolves when it is ready. */
export function workerPort(engine: LoadedEngine): Promise<HostPort> {
  const worker = new Worker(new URL("./hostWorker.ts", import.meta.url), { type: "module" });
  let listener: (message: FromWorker) => void = () => undefined;
  const port: HostPort = {
    post: (message: ToWorker) => worker.postMessage(message),
    listen: (next) => {
      listener = next;
    },
    terminate: () => worker.terminate(),
  };
  return new Promise<HostPort>((resolve, reject) => {
    worker.onmessage = (event: MessageEvent<FromWorker>) => {
      if (event.data.t === "ready") {
        worker.onmessage = (next: MessageEvent<FromWorker>) => listener(next.data);
        resolve(port);
      } else if (event.data.t === "fatal") {
        worker.terminate();
        reject(new Error(event.data.message));
      }
    };
    worker.onerror = (event) => {
      worker.terminate();
      reject(new Error(event.message || "The engine worker failed to start"));
    };
    worker.postMessage({ t: "init", module: engine.module } satisfies ToWorker);
  });
}

/** A transport on a new worker per run (Retry and recovery open a new one). */
export function openTransport(
  engine: LoadedEngine,
  options: Omit<LocalTransportOptions, "createPort">,
): LocalTransport {
  return new LocalTransport({ ...options, createPort: () => workerPort(engine) });
}
