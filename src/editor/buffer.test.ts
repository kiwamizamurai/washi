import { describe, expect, it } from "vitest";
import { Buffer, onDiskChange } from "./buffer";

const disk = (text: string, hash = `h:${text}`) => ({ text, hash });

describe("Buffer", () => {
  it("starts clean and becomes dirty once the text differs from what was saved", () => {
    const b = new Buffer(disk("one"));
    expect(b.dirty).toBe(false);
    b.edit("two");
    expect(b.dirty).toBe(true);
    b.edit("one");
    expect(b.dirty).toBe(false);
  });

  it("a save makes the current text the saved text and moves the base hash", () => {
    const b = new Buffer(disk("one"));
    b.edit("two");
    b.markSaved("two", "h:two");
    expect(b.dirty).toBe(false);
    expect(b.baseHash).toBe("h:two");
    b.edit("three");
    expect(b.dirty).toBe(true);
  });

  it("reload takes the disk version and drops local edits", () => {
    const b = new Buffer(disk("one"));
    b.edit("mine");
    b.reload(disk("theirs"));
    expect([b.text, b.dirty, b.baseHash]).toEqual(["theirs", false, "h:theirs"]);
  });

  it("rebase keeps the text and dirty state but accepts the disk hash", () => {
    const b = new Buffer(disk("one"));
    b.edit("mine");
    b.rebase("h:theirs");
    expect([b.text, b.dirty, b.baseHash]).toEqual(["mine", true, "h:theirs"]);
  });
});

describe("onDiskChange", () => {
  it("ignores a notification whose content is what we already based on (our own save)", () => {
    const b = new Buffer(disk("one"));
    b.edit("two");
    b.markSaved("two", "h:two");
    expect(onDiskChange(b, disk("two"))).toBe("ignore");
  });

  it("covers every combination", () => {
    const cases: [dirty: boolean, sameHash: boolean, sameText: boolean, expected: string][] = [
      [false, true, true, "ignore"],
      [true, true, false, "ignore"],
      [false, false, false, "reload"],
      [true, false, true, "reload"],
      [true, false, false, "conflict"],
    ];
    for (const [dirty, sameHash, sameText, expected] of cases) {
      const b = new Buffer(disk("base", "base-hash"));
      if (dirty) b.edit("mine");
      const incoming = disk(sameText ? b.text : "other", sameHash ? "base-hash" : "other-hash");
      expect(onDiskChange(b, incoming), JSON.stringify({ dirty, sameHash, sameText })).toBe(expected);
    }
  });
});
