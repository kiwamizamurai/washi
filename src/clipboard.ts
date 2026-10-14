const SINGLE_PATH = /^(?:file:\/\/)?(\/[^\n\r]+)$/;

export type Pasted = { kind: "file"; path: string } | { kind: "text"; text: string };

export function interpretPaste(
  raw: string,
  isSupported: (path: string) => boolean,
): Pasted | null {
  const text = raw.replace(/\r\n?/g, "\n");
  if (text.trim() === "") return null;

  const match = SINGLE_PATH.exec(text.trim());
  if (match) {
    let path = match[1];
    try {
      path = decodeURIComponent(path);
    } catch {
      path = match[1];
    }
    if (isSupported(path)) return { kind: "file", path };
  }
  return { kind: "text", text };
}

export function isEditable(target: EventTarget | null) {
  const el = target as HTMLElement | null;
  return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
}
