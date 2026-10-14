// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UpdateNotice } from "./update-notice";

const html = readFileSync(join(__dirname, "..", "index.html"), "utf8");
const markup = /<aside id="update"[\s\S]*?<\/aside>/.exec(html)![0];

const release = { current: "2.0.0", latest: "2.1.0", url: "https://github.com/kiwamizamurai/washi/releases/tag/v2.1.0" };

function setup(copy: (text: string) => Promise<unknown> = async () => {}) {
  document.body.innerHTML = markup;
  const el = document.getElementById("update")!;
  const actions = { openNotes: vi.fn(), copy: vi.fn(copy) };
  const notice = new UpdateNotice(el, actions);
  const button = (name: string) => el.querySelector<HTMLButtonElement>(`[data-update="${name}"]`)!;
  return { el, actions, notice, button };
}

const settle = () => new Promise((r) => setTimeout(r, 0));

describe("UpdateNotice", () => {
  beforeEach(() => (document.body.innerHTML = ""));

  it("is hidden until there is a release, then names both versions and the brew command", () => {
    const { el, notice } = setup();
    expect(el.hidden).toBe(true);
    notice.show(release);
    expect(el.hidden).toBe(false);
    expect(el.textContent).toContain("Washi 2.1.0 is available");
    expect(el.textContent).toContain("You have 2.0.0");
    expect(el.querySelector(".command code")!.textContent).toBe("brew upgrade --cask washi");
  });

  it("opens the release notes and stays open", () => {
    const { el, actions, notice, button } = setup();
    notice.show(release);
    button("notes").click();
    expect(actions.openNotes).toHaveBeenCalledWith(release.url);
    expect(el.hidden).toBe(false);
  });


  it("closes with the × button or Escape", () => {
    const { el, notice, button } = setup();
    notice.show(release);
    button("close").click();
    expect(el.hidden).toBe(true);
    notice.show(release);
    el.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(el.hidden).toBe(true);
  });

  it("copies the command and says so, or says that it failed", async () => {
    const ok = setup();
    ok.notice.show(release);
    ok.button("copy").click();
    await settle();
    expect(ok.actions.copy).toHaveBeenCalledWith("brew upgrade --cask washi");
    expect(ok.button("copy").textContent).toBe("Copied");

    const bad = setup(async () => Promise.reject(new Error("denied")));
    bad.notice.show(release);
    bad.button("copy").click();
    await settle();
    expect(bad.button("copy").textContent).toBe("Copy failed");
  });
});
