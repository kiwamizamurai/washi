import { autocompletion } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, redo, undo } from "@codemirror/commands";
import { bracketMatching, indentOnInput } from "@codemirror/language";
import { setDiagnostics } from "@codemirror/lint";
import { highlightSelectionMatches, openSearchPanel, search, searchKeymap } from "@codemirror/search";
import { Annotation, EditorState, Transaction, type Extension } from "@codemirror/state";
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import type { Diagnostic } from "../api";
import { columnToOffset, toCmDiagnostics } from "./lint";
import { countChars } from "./status";
import { washiTheme } from "./theme";

const FromOutside = Annotation.define<boolean>();

export interface CursorPosition {
  line: number;
  column: number;
  selected: number;
}

export interface EditorOptions {
  doc: string;
  language: Extension;
  extra?: Extension[];
  restore?: EditorState;
  onChange(text: string): void;
  onCursor?(position: CursorPosition): void;
}

export interface EditorHandle {
  getText(): string;
  setText(text: string): void;
  focus(): void;
  hasFocus(): boolean;
  undo(): void;
  redo(): void;
  openSearch(): void;
  replaceSelection(text: string): void;
  cursor(): CursorPosition;
  goTo(line: number, column: number): void;
  setDiagnostics(list: readonly Diagnostic[]): void;
  state(): EditorState;
  destroy(): void;
}

function cursorOf(state: EditorState): CursorPosition {
  const { head, from, to } = state.selection.main;
  const line = state.doc.lineAt(head);
  return {
    line: line.number,
    column: [...line.text.slice(0, head - line.from)].length + 1,
    selected: from === to ? 0 : countChars(state.sliceDoc(from, to)),
  };
}

export function createEditor(parent: HTMLElement, options: EditorOptions): EditorHandle {
  const state =
    options.restore ??
    EditorState.create({
      doc: options.doc,
      extensions: [
        lineNumbers(),
        highlightActiveLineGutter(),
        highlightActiveLine(),
        drawSelection(),
        history(),
        indentOnInput(),
        bracketMatching(),
        highlightSelectionMatches(),
        search({ top: true }),
        autocompletion({ activateOnTypingDelay: 120 }),
        keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap]),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({
          spellcheck: "false",
          autocorrect: "off",
          autocapitalize: "off",
          "aria-label": "Source",
        }),
        washiTheme,
        options.language,
        ...(options.extra ?? []),
        EditorView.updateListener.of((update) => {
          const outside = update.transactions.some((tr) => tr.annotation(FromOutside));
          if (update.docChanged && !outside) options.onChange(update.state.doc.toString());
          if (update.selectionSet || update.docChanged) options.onCursor?.(cursorOf(update.state));
        }),
      ],
    });
  const view = new EditorView({ parent, state });
  if (options.restore) view.dispatch({ effects: EditorView.scrollIntoView(state.selection.main.head, { y: "center" }) });

  return {
    getText: () => view.state.doc.toString(),
    setText(text) {
      const head = Math.min(view.state.selection.main.head, text.length);
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
        selection: { anchor: head },
        annotations: [FromOutside.of(true), Transaction.addToHistory.of(false)],
      });
    },
    focus: () => view.focus(),
    hasFocus: () => view.hasFocus,
    undo: () => void undo(view),
    redo: () => void redo(view),
    openSearch: () => void openSearchPanel(view),
    replaceSelection(text) {
      view.dispatch(view.state.replaceSelection(text), { scrollIntoView: true, userEvent: "input.paste" });
    },
    cursor: () => cursorOf(view.state),
    goTo(line, column) {
      const l = view.state.doc.line(Math.min(Math.max(line, 1), view.state.doc.lines));
      const pos = Math.min(l.from + columnToOffset(l.text, column), l.to);
      view.dispatch({ selection: { anchor: pos }, effects: EditorView.scrollIntoView(pos, { y: "center" }) });
      view.focus();
    },
    setDiagnostics(list) {
      view.dispatch(setDiagnostics(view.state, toCmDiagnostics(view.state.doc, list)));
    },
    state: () => view.state,
    destroy: () => view.destroy(),
  };
}
