import { autocompletion, snippet, type Completion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import type { Extension } from "@codemirror/state";
import type { CompletionItem, Completions } from "../api";

type Ask = (text: string, offset: number, explicit: boolean) => Promise<Completions>;

export function completionType(kind: string): string {
  switch (kind) {
    case "func":
      return "function";
    case "syntax":
      return "keyword";
    case "type":
      return "type";
    case "param":
      return "property";
    case "constant":
      return "constant";
    case "package":
      return "namespace";
    case "label":
      return "variable";
    default:
      return "text";
  }
}

export function toOption(item: CompletionItem): Completion {
  const apply = item.apply ?? item.label;
  return {
    label: item.label,
    detail: item.detail ?? undefined,
    type: completionType(item.kind),
    apply: apply.includes("${") ? snippet(apply) : apply,
  };
}

export function typstCompletion(ask: Ask): Extension {
  const source = async (context: CompletionContext): Promise<CompletionResult | null> => {
    const found = await ask(context.state.doc.toString(), context.pos, context.explicit);
    if (context.aborted || found.items.length === 0) return null;
    return { from: found.offset, options: found.items.map(toOption), validFor: /^[\w-]*$/ };
  };
  return autocompletion({ override: [source], activateOnTypingDelay: 120 });
}
