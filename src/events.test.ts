import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it, vi } from "vitest";

const windowListen = vi.fn(async () => () => {});
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({ listen: windowListen }),
}));

const { listenHere } = await import("./events");

function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sources(path);
    return entry.name.endsWith(".ts") && !entry.name.endsWith(".test.ts") ? [path] : [];
  });
}

describe("listenHere", () => {
  it("listens on the current window only", async () => {
    const handler = () => {};
    await listenHere("washi://menu", handler);
    expect(windowListen).toHaveBeenCalledWith("washi://menu", handler);
  });

  it("is the only way the app listens to Tauri events", () => {
    const root = new URL(".", import.meta.url).pathname;
    const offenders = sources(root).filter((file) =>
      /import\s*\{[^}]*\blisten\b[^}]*\}\s*from\s*"@tauri-apps\/api\/event"/.test(readFileSync(file, "utf8")),
    );
    expect(offenders, "the global listen also receives events sent to other windows and tabs").toEqual([]);
  });
});
