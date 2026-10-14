import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import type { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

const view = EditorView.theme({
  "&": {
    height: "100%",
    color: "var(--ink)",
    backgroundColor: "var(--paper)",
    fontSize: "0.9rem",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    fontFamily: "var(--font-mono)",
    lineHeight: "1.65",
    overflow: "auto",
  },
  ".cm-content": { caretColor: "var(--accent)", padding: "1rem 0 40vh" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accent)" },
  ".cm-line": { padding: "0 1rem 0 0.5rem" },
  ".cm-gutters": {
    backgroundColor: "var(--paper)",
    color: "var(--muted)",
    border: "none",
    borderRight: "1px solid var(--rule)",
  },
  ".cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--code-bg) 60%, transparent)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--ink)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
    backgroundColor: "color-mix(in srgb, var(--accent) 24%, transparent)",
  },
  ".cm-matchingBracket, &.cm-focused .cm-matchingBracket": {
    backgroundColor: "color-mix(in srgb, var(--accent) 22%, transparent)",
    outline: "none",
  },
  ".cm-selectionMatch": { backgroundColor: "color-mix(in srgb, var(--accent) 14%, transparent)" },
  ".cm-searchMatch": {
    backgroundColor: "color-mix(in srgb, var(--hl-number) 30%, transparent)",
    outline: "1px solid color-mix(in srgb, var(--hl-number) 60%, transparent)",
  },
  ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "color-mix(in srgb, var(--accent) 35%, transparent)" },
  ".cm-panels": {
    backgroundColor: "var(--paper)",
    color: "var(--ink)",
    borderColor: "var(--rule)",
    fontFamily: "var(--font-sans)",
  },
  ".cm-panels.cm-panels-top": { borderBottom: "1px solid var(--rule)" },
  ".cm-search": { padding: "0.4rem 0.6rem", fontSize: "0.82rem" },
  ".cm-search input, .cm-search button": { font: "inherit", color: "var(--ink)" },
  ".cm-textfield": {
    backgroundColor: "var(--code-bg)",
    border: "1px solid var(--rule)",
    borderRadius: "4px",
  },
  ".cm-button": {
    backgroundImage: "none",
    backgroundColor: "var(--code-bg)",
    border: "1px solid var(--rule)",
    borderRadius: "4px",
  },
  ".cm-tooltip": {
    backgroundColor: "var(--paper)",
    color: "var(--ink)",
    border: "1px solid var(--rule)",
    borderRadius: "6px",
    boxShadow: "0 6px 24px rgba(0, 0, 0, 0.18)",
    fontFamily: "var(--font-sans)",
  },
  ".cm-tooltip-autocomplete > ul": { fontFamily: "var(--font-mono)", maxHeight: "16em" },
  ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
    backgroundColor: "color-mix(in srgb, var(--accent) 22%, transparent)",
    color: "var(--ink)",
  },
  ".cm-completionDetail": { color: "var(--muted)", fontStyle: "normal", marginLeft: "0.6em" },
  ".cm-diagnostic": { fontFamily: "var(--font-sans)", whiteSpace: "pre-wrap" },
  ".cm-diagnostic-error": { borderLeft: "3px solid var(--accent)" },
  ".cm-diagnostic-warning": { borderLeft: "3px solid var(--hl-number)" },
  ".cm-lintRange-error": { backgroundImage: "none", textDecoration: "underline wavy var(--accent)" },
  ".cm-lintRange-warning": { backgroundImage: "none", textDecoration: "underline wavy var(--hl-number)" },
});

const highlight = HighlightStyle.define([
  { tag: tags.heading, color: "var(--hl-title)", fontWeight: "700" },
  { tag: tags.strong, fontWeight: "700" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: tags.link, color: "var(--accent)" },
  { tag: tags.url, color: "var(--hl-attr)" },
  { tag: [tags.keyword, tags.operatorKeyword, tags.modifier], color: "var(--hl-keyword)" },
  { tag: [tags.string, tags.special(tags.string)], color: "var(--hl-string)" },
  { tag: [tags.number, tags.bool, tags.atom], color: "var(--hl-number)" },
  { tag: [tags.comment, tags.meta, tags.quote], color: "var(--hl-comment)", fontStyle: "italic" },
  { tag: [tags.monospace, tags.processingInstruction], color: "var(--hl-attr)" },
  { tag: [tags.function(tags.variableName), tags.labelName, tags.typeName, tags.tagName], color: "var(--hl-title)" },
  { tag: [tags.attributeName, tags.propertyName], color: "var(--hl-attr)" },
  { tag: tags.contentSeparator, color: "var(--muted)" },
]);

export const washiTheme: Extension = [view, syntaxHighlighting(highlight)];
