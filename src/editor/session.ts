import type { SaveResult } from "../api";
import { Buffer, onDiskChange, type Disk, type DiskChange } from "./buffer";
import { renderPolicy, type Kind } from "./kinds";
import { Scheduler, type Timers } from "./scheduler";

export type Banner =
  | null
  | { kind: "conflict" }
  | { kind: "error"; message: string };

export const AUTOSAVE_IDLE_MS = 1000;
export const AUTOSAVE_MAX_MS = 5000;

export interface SessionHost {
  readText(path: string): Promise<Disk>;
  writeFile(path: string, text: string, baseHash: string | null, force: boolean): Promise<SaveResult>;
  render(path: string, text: string): Promise<unknown>;
  setText(text: string): void;
  dirtyChanged(dirty: boolean): void;
  banner(banner: Banner): void;
  notify(message: string): void;
}

export interface SessionOptions {
  kind: Kind;
  autosave(): boolean;
  timers?: Timers;
}

export type SaveOutcome = "saved" | "clean" | "conflict" | "failed";

const realTimers: Timers = {
  now: () => performance.now(),
  set: (fn, ms) => window.setTimeout(fn, ms),
  clear: (handle) => window.clearTimeout(handle as number),
};

export class EditSession {
  readonly buffer: Buffer;
  private readonly scheduler: Scheduler;
  private readonly timers: Timers;
  private idleTimer: unknown;
  private maxTimer: unknown;
  private lastDirty = false;
  private conflict: Disk | null = null;
  private saving: Promise<SaveOutcome> | null = null;

  constructor(
    private readonly host: SessionHost,
    readonly path: string,
    disk: Disk,
    private readonly options: SessionOptions,
  ) {
    this.timers = options.timers ?? realTimers;
    this.buffer = new Buffer(disk);
    const policy = renderPolicy(options.kind);
    this.scheduler = new Scheduler({
      ...policy,
      timers: this.timers,
      run: () => this.host.render(this.path, this.buffer.text),
    });
  }

  get dirty() {
    return this.buffer.dirty;
  }

  get text() {
    return this.buffer.text;
  }

  start() {
    this.scheduler.flush();
  }

  edit(text: string) {
    if (text === this.buffer.text) return;
    this.buffer.edit(text);
    this.scheduler.trigger();
    this.syncDirty();
    if (this.options.autosave()) this.armAutosave();
  }

  async save(): Promise<SaveOutcome> {
    if (this.saving) await this.saving.catch(() => {});
    this.saving = this.doSave();
    try {
      return await this.saving;
    } finally {
      this.saving = null;
    }
  }

  private async doSave(): Promise<SaveOutcome> {
    this.disarmAutosave();
    if (!this.buffer.dirty) return "clean";
    const text = this.buffer.text;
    let result: SaveResult;
    try {
      result = await this.host.writeFile(this.path, text, this.buffer.baseHash, false);
    } catch (e) {
      const message = String(e);
      this.host.banner({ kind: "error", message });
      this.host.notify(message);
      return "failed";
    }
    if (result.status === "conflict") {
      this.conflict = await this.readDisk();
      this.host.banner({ kind: "conflict" });
      return "conflict";
    }
    this.buffer.markSaved(text, result.hash);
    this.host.banner(null);
    this.syncDirty();
    this.scheduler.flush();
    return "saved";
  }

  async diskChanged(): Promise<DiskChange | "unreadable"> {
    const disk = await this.readDisk();
    if (!disk) return "unreadable";
    const change = onDiskChange(this.buffer, disk);
    if (change === "reload") this.adopt(disk);
    if (change === "conflict") {
      this.conflict = disk;
      this.host.banner({ kind: "conflict" });
    }
    return change;
  }

  refreshPreview() {
    this.scheduler.flush();
  }

  keepMine() {
    if (!this.conflict) return;
    this.buffer.rebase(this.conflict.hash);
    this.conflict = null;
    this.host.banner(null);
  }

  async loadDisk() {
    const disk = this.conflict ?? (await this.readDisk());
    this.conflict = null;
    this.host.banner(null);
    if (disk) this.adopt(disk);
  }

  dispose() {
    this.scheduler.cancel();
    this.disarmAutosave();
  }

  private adopt(disk: Disk) {
    this.buffer.reload(disk);
    this.host.setText(disk.text);
    this.syncDirty();
    this.host.banner(null);
    this.scheduler.flush();
  }

  private async readDisk(): Promise<Disk | null> {
    try {
      return await this.host.readText(this.path);
    } catch {
      return null;
    }
  }

  private syncDirty() {
    if (this.lastDirty === this.buffer.dirty) return;
    this.lastDirty = this.buffer.dirty;
    this.host.dirtyChanged(this.lastDirty);
  }

  private armAutosave() {
    if (this.idleTimer !== undefined) this.timers.clear(this.idleTimer);
    this.idleTimer = this.timers.set(() => void this.autosave(), AUTOSAVE_IDLE_MS);
    this.maxTimer ??= this.timers.set(() => void this.autosave(), AUTOSAVE_MAX_MS);
  }

  private disarmAutosave() {
    if (this.idleTimer !== undefined) this.timers.clear(this.idleTimer);
    if (this.maxTimer !== undefined) this.timers.clear(this.maxTimer);
    this.idleTimer = undefined;
    this.maxTimer = undefined;
  }

  private async autosave() {
    this.disarmAutosave();
    if (this.conflict) return;
    await this.save();
  }
}
