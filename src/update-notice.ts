import { UPGRADE_COMMAND, type Release } from "./update";

export interface NoticeActions {
  openNotes(url: string): unknown;
  copy(text: string): Promise<unknown>;
}

const COPIED_MS = 1600;

export class UpdateNotice {
  private release: Release | null = null;
  private readonly title: HTMLElement;
  private readonly detail: HTMLElement;
  private readonly copyButton: HTMLButtonElement;
  private copiedTimer: number | undefined;

  constructor(
    private readonly el: HTMLElement,
    private readonly actions: NoticeActions,
  ) {
    this.title = el.querySelector(".title")!;
    this.detail = el.querySelector(".detail")!;
    this.copyButton = el.querySelector('[data-update="copy"]')!;
    el.querySelector(".command code")!.textContent = UPGRADE_COMMAND;
    el.addEventListener("click", (e) => {
      const button = (e.target as Element).closest<HTMLButtonElement>("button[data-update]");
      if (button) void this.run(button.dataset.update ?? "");
    });
    el.addEventListener("keydown", (e) => {
      if (e.key === "Escape") this.hide();
    });
  }

  get visible() {
    return !this.el.hidden;
  }

  show(release: Release) {
    this.release = release;
    this.title.textContent = `Washi ${release.latest} is available`;
    this.detail.textContent = `You have ${release.current}. Update with Homebrew:`;
    this.copyButton.textContent = "Copy";
    this.el.hidden = false;
  }

  hide() {
    this.el.hidden = true;
  }

  private async run(action: string) {
    const release = this.release;
    if (!release) return;
    if (action === "close") this.hide();
    if (action === "notes") this.actions.openNotes(release.url);
    if (action === "copy") {
      try {
        await this.actions.copy(UPGRADE_COMMAND);
        this.copyButton.textContent = "Copied";
      } catch {
        this.copyButton.textContent = "Copy failed";
      }
      clearTimeout(this.copiedTimer);
      this.copiedTimer = window.setTimeout(() => (this.copyButton.textContent = "Copy"), COPIED_MS);
    }
  }
}
