import { rank, type Item, type Ranked, type Source } from "./items";

export class CommandPalette {
  private readonly input: HTMLInputElement;
  private readonly list: HTMLUListElement;
  private results: Ranked[] = [];
  private selected = 0;
  private source: Source = { commands: [], files: [], headings: [] };

  constructor(
    private readonly dialog: HTMLDialogElement,
    private readonly build: () => Source,
  ) {
    this.input = dialog.querySelector("input")!;
    this.list = dialog.querySelector("ul")!;
    this.input.addEventListener("input", () => this.update());
    this.input.addEventListener("keydown", (e) => this.onKey(e));
    dialog.addEventListener("mousedown", (e) => {
      if (e.target === dialog) dialog.close();
    });
  }

  get isOpen() {
    return this.dialog.open;
  }

  open() {
    if (this.dialog.open) {
      this.input.select();
      return;
    }
    this.source = this.build();
    this.input.value = "";
    this.update();
    this.dialog.showModal();
    this.input.focus();
  }

  private update() {
    this.results = rank(this.source, this.input.value);
    this.selected = 0;
    this.render();
  }

  private render() {
    this.list.replaceChildren(
      ...(this.results.length === 0
        ? [this.message("No results")]
        : this.results.map((r, i) => this.option(r, i))),
    );
    this.syncSelection();
  }

  private message(text: string) {
    const li = document.createElement("li");
    li.className = "none";
    li.textContent = text;
    return li;
  }

  private option({ item, indices }: Ranked, index: number) {
    const li = document.createElement("li");
    li.id = `palette-option-${index}`;
    li.setAttribute("role", "option");
    const group = document.createElement("span");
    group.className = "group";
    group.textContent = item.group;
    const title = document.createElement("span");
    title.className = "title";
    title.append(...highlight(item.title, indices));
    li.append(group, title);
    if (item.detail) {
      const detail = document.createElement("span");
      detail.className = "detail";
      detail.textContent = item.detail;
      li.append(detail);
    }
    if (item.shortcut) {
      const keys = document.createElement("kbd");
      keys.textContent = item.shortcut;
      li.append(keys);
    }
    li.addEventListener("mousemove", () => {
      if (this.selected !== index) {
        this.selected = index;
        this.syncSelection();
      }
    });
    li.addEventListener("click", () => this.run(item));
    return li;
  }

  private syncSelection() {
    const options = [...this.list.querySelectorAll<HTMLElement>('[role="option"]')];
    options.forEach((el, i) => el.setAttribute("aria-selected", String(i === this.selected)));
    const current = options[this.selected];
    if (current) {
      this.input.setAttribute("aria-activedescendant", current.id);
      current.scrollIntoView({ block: "nearest" });
    } else {
      this.input.removeAttribute("aria-activedescendant");
    }
  }

  private onKey(e: KeyboardEvent) {
    const count = this.results.length;
    switch (e.key) {
      case "ArrowDown":
      case "ArrowUp": {
        e.preventDefault();
        if (count === 0) return;
        this.selected = (this.selected + (e.key === "ArrowDown" ? 1 : count - 1)) % count;
        this.syncSelection();
        return;
      }
      case "Home":
      case "End":
        if (!(e.metaKey || e.ctrlKey) || count === 0) return;
        e.preventDefault();
        this.selected = e.key === "Home" ? 0 : count - 1;
        this.syncSelection();
        return;
      case "Enter": {
        e.preventDefault();
        const chosen = this.results[this.selected];
        if (chosen) this.run(chosen.item);
        return;
      }
    }
  }

  private run(item: Item) {
    this.dialog.close();
    setTimeout(() => void item.run(), 0);
  }
}

function highlight(text: string, indices: number[]): Node[] {
  if (indices.length === 0) return [document.createTextNode(text)];
  const marked = new Set(indices);
  const nodes: Node[] = [];
  let run = "";
  let inMark = false;
  const flush = () => {
    if (!run) return;
    if (inMark) {
      const mark = document.createElement("mark");
      mark.textContent = run;
      nodes.push(mark);
    } else {
      nodes.push(document.createTextNode(run));
    }
    run = "";
  };
  [...text].forEach((ch, i) => {
    const hit = marked.has(i);
    if (hit !== inMark) {
      flush();
      inMark = hit;
    }
    run += ch;
  });
  flush();
  return nodes;
}
