export interface Disk {
  text: string;
  hash: string;
}

export class Buffer {
  text: string;
  private saved: string;
  baseHash: string;

  constructor(disk: Disk) {
    this.text = disk.text;
    this.saved = disk.text;
    this.baseHash = disk.hash;
  }

  get dirty() {
    return this.text !== this.saved;
  }

  edit(text: string) {
    this.text = text;
  }

  markSaved(text: string, hash: string) {
    this.saved = text;
    this.baseHash = hash;
  }

  reload(disk: Disk) {
    this.text = disk.text;
    this.saved = disk.text;
    this.baseHash = disk.hash;
  }

  rebase(diskHash: string) {
    this.baseHash = diskHash;
  }
}

export type DiskChange = "ignore" | "reload" | "conflict";

export function onDiskChange(buffer: Buffer, disk: Disk): DiskChange {
  if (disk.hash === buffer.baseHash) return "ignore";
  if (!buffer.dirty) return "reload";
  if (disk.text === buffer.text) return "reload";
  return "conflict";
}
