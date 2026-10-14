export interface Release {
  current: string;
  latest: string;
  url: string;
}

export interface UpdateState {
  lastCheck: number;
  automatic: boolean;
}

export type Outcome =
  | { kind: "available"; release: Release }
  | { kind: "current"; release: Release }
  | { kind: "not-due" }
  | { kind: "off" }
  | { kind: "failed"; error: string };

export const CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;
export const UPGRADE_COMMAND = "brew upgrade --cask washi";

const KEY = "washi:update";

export const DEFAULT_UPDATE_STATE: UpdateState = { lastCheck: 0, automatic: true };

type Store = Pick<Storage, "getItem" | "setItem">;

function defaultStore(): Store | null {
  try {
    return localStorage;
  } catch {
    return null;
  }
}

export function loadUpdateState(store: Store | null = defaultStore()): UpdateState {
  try {
    const parsed: Record<string, unknown> = JSON.parse(store?.getItem(KEY) ?? "{}") ?? {};
    return {
      lastCheck: typeof parsed.lastCheck === "number" && Number.isFinite(parsed.lastCheck) ? parsed.lastCheck : 0,
      automatic: typeof parsed.automatic === "boolean" ? parsed.automatic : true,
    };
  } catch {
    return { ...DEFAULT_UPDATE_STATE };
  }
}

export function saveUpdateState(state: UpdateState, store: Store | null = defaultStore()) {
  try {
    store?.setItem(KEY, JSON.stringify(state));
  } catch {
    return;
  }
}

export function updateState(patch: Partial<UpdateState>, store: Store | null = defaultStore()): UpdateState {
  const next = { ...loadUpdateState(store), ...patch };
  saveUpdateState(next, store);
  return next;
}

function parts(version: string): number[] | null {
  const m = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(version.trim());
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

export function isNewer(latest: string, current: string): boolean {
  const a = parts(latest);
  const b = parts(current);
  if (!a || !b) return false;
  for (let i = 0; i < 3; i++) {
    if (a[i] !== b[i]) return a[i] > b[i];
  }
  return false;
}

export interface CheckOptions {
  fetchLatest(): Promise<Release>;
  manual: boolean;
  now: number;
  store?: Store | null;
}

export async function checkForUpdate({ fetchLatest, manual, now, store = defaultStore() }: CheckOptions): Promise<Outcome> {
  const state = loadUpdateState(store);
  if (!manual) {
    if (!state.automatic) return { kind: "off" };
    if (now >= state.lastCheck && now - state.lastCheck < CHECK_INTERVAL_MS) return { kind: "not-due" };
  }
  updateState({ lastCheck: now }, store);
  let release: Release;
  try {
    release = await fetchLatest();
  } catch (e) {
    return { kind: "failed", error: String(e) };
  }
  if (!isNewer(release.latest, release.current)) return { kind: "current", release };
  return { kind: "available", release };
}
