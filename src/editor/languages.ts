import { StreamLanguage, type StreamParser } from "@codemirror/language";
import type { Extension } from "@codemirror/state";
import type { Kind } from "./kinds";

export async function languageFor(kind: Kind): Promise<Extension> {
  switch (kind) {
    case "markdown": {
      const { markdown } = await import("@codemirror/lang-markdown");
      return markdown();
    }
    case "latex": {
      const { stex } = await import("@codemirror/legacy-modes/mode/stex");
      return StreamLanguage.define(stex);
    }
    case "typst":
      try {
        return await typstSupport();
      } catch {
        return StreamLanguage.define(typstFallback);
      }
    case "mermaid":
      return [];
  }
}

async function typstSupport(): Promise<Extension> {
  const [language, lezer] = await Promise.all([import("@codemirror/language"), import("codemirror-lang-typst/lezer")]);
  const { Language, LanguageSupport, languageDataProp, HighlightStyle, syntaxHighlighting } = language;
  const { tags } = await import("@lezer/highlight");
  const parser = new lezer.TypstLezerParser(
    languageDataProp.add((type) => (type.isTop ? lezer.typstLezerLanguageData : undefined)),
    lezer.typstLezerHighlighting,
    lezer.typstLezerIndentation,
    lezer.typstLezerFolding,
  );
  const style = HighlightStyle.define([
    { tag: tags.comment, color: "var(--hl-comment)", fontStyle: "italic" },
    { tag: tags.monospace, color: "var(--hl-attr)" },
    { tag: lezer.typstTags.mathDelimiter, color: "var(--hl-string)" },
    { tag: [lezer.typstTags.listMarker, lezer.typstTags.interpolated, tags.propertyName], color: "var(--hl-keyword)" },
    { tag: [tags.escape, tags.labelName], color: "var(--hl-attr)" },
    { tag: [tags.keyword, tags.null, tags.atom, tags.bool], color: "var(--hl-keyword)" },
    { tag: tags.number, color: "var(--hl-number)" },
    { tag: tags.string, color: "var(--hl-string)" },
    { tag: tags.function(tags.variableName), color: "var(--hl-title)" },
    { tag: tags.heading, color: "var(--hl-title)", fontWeight: "700" },
    { tag: tags.strong, fontWeight: "700" },
    { tag: tags.emphasis, fontStyle: "italic" },
    { tag: tags.link, color: "var(--accent)", textDecoration: "underline" },
    { tag: tags.invalid, color: "var(--accent)" },
  ]);
  return new LanguageSupport(new Language(lezer.typstLezerLanguageData, parser, [], "typst"), [
    syntaxHighlighting(style),
    lezer.typstLezerIndentService,
    lezer.typstLezerListKeymap,
    lezer.typstLezerFoldService,
  ]);
}

export const typstFallback: StreamParser<{ math: boolean }> = {
  startState: () => ({ math: false }),
  token(stream, state) {
    if (stream.sol() && stream.match(/^=+\s/)) {
      stream.skipToEnd();
      return "heading";
    }
    if (stream.match("//")) {
      stream.skipToEnd();
      return "comment";
    }
    if (stream.match(/^"(?:[^"\\]|\\.)*"?/)) return "string";
    if (stream.match(/^#[A-Za-z_][\w.-]*/)) return "keyword";
    if (stream.eat("$")) {
      state.math = !state.math;
      return "string";
    }
    if (stream.match(/^\d+(\.\d+)?(pt|em|cm|mm|in|%|fr)?/)) return "number";
    if (state.math) {
      stream.next();
      return "string";
    }
    stream.next();
    return null;
  },
};
