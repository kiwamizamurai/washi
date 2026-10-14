import { invoke } from "@tauri-apps/api/core";
import type { Release } from "./update";

export type Output = { kind: "html"; html: string };

export function decodeOutput(wire: Uint8Array): Output {
  return { kind: "html", html: new TextDecoder().decode(wire.subarray(1)) };
}

export async function render(path: string): Promise<Output> {
  const wire = await invoke<ArrayBuffer>("render", { path });
  return decodeOutput(new Uint8Array(wire));
}

export interface DocumentList {
  root: string;
  files: string[];
  truncated: boolean;
}

export const listDocuments = () => invoke<DocumentList>("list_documents");

export const openInTab = (path: string) => invoke<void>("open_in_tab", { path });

export async function renderText(text: string): Promise<Output> {
  const wire = await invoke<ArrayBuffer>("render_text", { text });
  return decodeOutput(new Uint8Array(wire));
}

export type Severity = "error" | "warning";

export interface Diagnostic {
  file: string | null;
  line: number;
  column: number;
  endLine: number;
  endColumn: number;
  severity: Severity;
  message: string;
  hints: string[];
}

export type BufferResult =
  | { ok: true; output: Output; diagnostics: Diagnostic[] }
  | { ok: false; message: string; diagnostics: Diagnostic[] };

const FAILED_TAG = 2;
const HEADER_BYTES = 5;

interface WireDiagnostic {
  file: string | null;
  line: number;
  column: number;
  end_line: number;
  end_column: number;
  severity: Severity;
  message: string;
  hints: string[];
}

export function decodeBuffer(wire: Uint8Array): BufferResult {
  const length = new DataView(wire.buffer, wire.byteOffset, wire.byteLength).getUint32(1, false);
  const json = new TextDecoder().decode(wire.subarray(HEADER_BYTES, HEADER_BYTES + length));
  const diagnostics = (JSON.parse(json) as WireDiagnostic[]).map(
    (d): Diagnostic => ({
      file: d.file,
      line: d.line,
      column: d.column,
      endLine: d.end_line,
      endColumn: d.end_column,
      severity: d.severity,
      message: d.message,
      hints: d.hints,
    }),
  );
  const body = wire.subarray(HEADER_BYTES + length);
  if (wire[0] === FAILED_TAG) {
    return { ok: false, message: new TextDecoder().decode(body), diagnostics };
  }
  return { ok: true, output: { kind: "html", html: new TextDecoder().decode(body) }, diagnostics };
}

export async function renderBuffer(path: string, text: string): Promise<BufferResult> {
  const wire = await invoke<ArrayBuffer>("render_buffer", { path, text });
  return decodeBuffer(new Uint8Array(wire));
}

export interface DiskText {
  text: string;
  hash: string;
}

export type SaveResult = { status: "saved"; hash: string } | { status: "conflict"; disk_hash: string };

export const readText = (path: string) => invoke<DiskText>("read_text", { path });

export const writeFile = (path: string, text: string, baseHash: string | null, force = false) =>
  invoke<SaveResult>("write_file", { path, text, baseHash, force });

export interface CompletionItem {
  label: string;
  apply: string | null;
  detail: string | null;
  kind: string;
}

export interface Completions {
  offset: number;
  items: CompletionItem[];
}

export const autocomplete = (path: string, text: string, offset: number, explicit: boolean) =>
  invoke<Completions>("autocomplete", { path, text, offset, explicit });

export const setDirty = (dirty: boolean) => invoke<void>("set_dirty", { dirty });

export const supportedExtensions =() =>
  invoke<string[]>("supported_extensions");

export const initialFile = () => invoke<string | null>("initial_file");

export const openSource = (file: string, line: number, column: number) =>
  invoke<string>("open_source", { file, line, column });

export const print = () => invoke<void>("print");

export const latestRelease = () => invoke<Release>("latest_release");

export const watch =(path: string) => invoke<void>("watch", { path });
