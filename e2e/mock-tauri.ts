type Handler = (event: { event: string; id: number; payload: unknown }) => void;

const listeners = new Map<number, { event: string; handler: number }>();
const callbacks = new Map<number, Handler>();
let nextId = 1;

const params = new URLSearchParams(location.search);
const basename = (p: string) => p.split("/").pop() ?? p;

async function wire(name: string) {
  const res = await fetch(`/e2e/fixtures/${basename(name)}.wire`);
  if (!res.ok || res.headers.get("content-type")?.includes("text/html")) throw new Error(`fixture が無い: ${name}`);
  return res.arrayBuffer();
}

const w = window as unknown as Record<string, unknown>;

const files = new Map<string, string>();
const hashOf = (text: string) => `h${text.length}:${[...text].reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7)}`;
const calls: { cmd: string; args: unknown }[] = [];
w.__files = files;
w.__calls = calls;

const encode = (tag: number, diagnostics: unknown[], body: string) => {
  const json = new TextEncoder().encode(JSON.stringify(diagnostics));
  const text = new TextEncoder().encode(body);
  const out = new Uint8Array(5 + json.length + text.length);
  out[0] = tag;
  new DataView(out.buffer).setUint32(1, json.length, false);
  out.set(json, 5);
  out.set(text, 5 + json.length);
  return out.buffer;
};

function miniMarkdown(text: string) {
  const lines = text.split("\n");
  const html: string[] = [];
  lines.forEach((line, i) => {
    if (!line.trim()) return;
    const pos = `${i + 1}:1-${i + 1}:${line.length + 1}`;
    const heading = /^(#{1,3}) (.*)$/.exec(line);
    html.push(heading ? `<h${heading[1].length} data-sourcepos="${pos}">${heading[2]}</h${heading[1].length}>` : `<p data-sourcepos="${pos}">${line}</p>`);
  });
  return html.join("\n");
}

function renderBuffer(path: string, text: string) {
  if (path.endsWith(".typ")) {
    const at = text.indexOf("#bad");
    if (at >= 0) {
      const before = text.slice(0, at).split("\n");
      const line = before.length;
      const column = [...before[before.length - 1]].length + 1;
      return encode(2, [{ file: null, line, column, end_line: line, end_column: column + 4, severity: "error", message: "unknown variable: bad", hints: [] }], "error: unknown variable: bad");
    }
    const warn = text.indexOf("#warn");
    if (warn >= 0) {
      const before = text.slice(0, warn).split("\n");
      const line = before.length;
      const column = [...before[before.length - 1]].length + 1;
      return encode(0, [{ file: null, line, column, end_line: line, end_column: column + 5, severity: "warning", message: "unused", hints: [] }], miniMarkdown(text));
    }
    return encode(0, [], miniMarkdown(text));
  }
  return encode(0, [], miniMarkdown(text));
}
w.__TAURI_INTERNALS__ = {
  metadata: {
    currentWindow: { label: "main" },
    currentWebview: { label: "main", windowLabel: "main" },
  },
  transformCallback(cb: Handler) {
    const id = nextId++;
    callbacks.set(id, cb);
    return id;
  },
  unregisterCallback(id: number) {
    callbacks.delete(id);
  },
  convertFileSrc: (p: string) => p,
  async invoke(cmd: string, args: Record<string, unknown> = {}) {
    switch (cmd) {
      case "supported_extensions":
        return ["md", "markdown", "mdown", "mmd", "mermaid", "typ", "tex", "latex", "pdf"];
      case "initial_file":
        return params.get("file");
      case "render":
        return wire(String(args.path));
      case "render_text":
        return wire(params.get("text") ?? "showcase.md");
      case "render_buffer": {
        calls.push({ cmd, args });
        if (params.get("realpreview")) {
          const old = new Uint8Array(await wire(String(args.path)));
          return encode(old[0], [], new TextDecoder().decode(old.subarray(1)));
        }
        return renderBuffer(String(args.path), String(args.text));
      }
      case "read_text": {
        const text = files.get(String(args.path));
        if (text === undefined) throw new Error("ファイルが無い");
        return { text, hash: hashOf(text) };
      }
      case "write_file": {
        calls.push({ cmd, args });
        const path = String(args.path);
        const disk = files.get(path) ?? "";
        const force = Boolean(args.force);
        if (!force && args.baseHash !== null && args.baseHash !== undefined && args.baseHash !== hashOf(disk)) {
          return { status: "conflict", disk_hash: hashOf(disk) };
        }
        files.set(path, String(args.text));
        return { status: "saved", hash: hashOf(String(args.text)) };
      }
      case "autocomplete": {
        calls.push({ cmd, args });
        const text = String(args.text).slice(0, Number(args.offset));
        const m = /#(\w*)$/.exec(text);
        if (!m) return { offset: Number(args.offset), items: [] };
        const items = [{ label: "lorem", kind: "func", detail: "words: int", apply: "lorem(${words})" }, { label: "let", kind: "syntax", detail: null, apply: null }];
        return { offset: Number(args.offset) - m[1].length, items: items.filter((i) => i.label.startsWith(m[1])) };
      }
      case "latexmkrc_status": {
        if (!String(args.path).endsWith(".tex") || !files.has("/x/.latexmkrc")) return null;
        return { file: "/x/.latexmkrc", trusted: w.__rcTrusted === true };
      }
      case "trust_latexmkrc":
        calls.push({ cmd, args });
        w.__rcTrusted = true;
        return null;
      case "forward_locate":
      case "locate_source":
        calls.push({ cmd, args });
        return null;
      case "set_dirty":
        calls.push({ cmd, args });
        return null;
      case "watch":
        return null;
      case "plugin:window|set_title":
        calls.push({ cmd, args });
        return null;
      case "plugin:event|listen": {
        const id = nextId++;
        listeners.set(id, { event: String(args.event), handler: Number(args.handler) });
        return id;
      }
      case "plugin:clipboard-manager|read_text":
        return (w.__mockClipboard as string) ?? "";
      default:
        return null;
    }
  },
};

w.__emit = (event: string, payload?: unknown) => {
  for (const [id, l] of listeners) {
    if (l.event === event) callbacks.get(l.handler)?.({ event, id, payload });
  }
};
