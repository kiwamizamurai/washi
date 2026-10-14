const DROPPED_TAGS = "script, iframe, object, embed, base, meta, link, frame, frameset";
const LINK_ATTRS = ["href", "xlink:href", "action", "formaction"];
const SAFE_LINK = /^(https?:|mailto:|#|\/|\.|[^:]*$)/i;
const SAFE_IMAGE = /^data:image\//i;

const compact = (value: string) => value.replace(/[\u0000- ]/g, "");

export function sanitize(html: string): string {
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
  for (const el of doc.body.querySelectorAll(DROPPED_TAGS)) el.remove();
  for (const el of doc.body.querySelectorAll("*")) {
    for (const attr of [...el.attributes]) {
      const name = attr.name.toLowerCase();
      const value = compact(attr.value);
      if (name.startsWith("on")) el.removeAttribute(attr.name);
      else if (LINK_ATTRS.includes(name) && !SAFE_LINK.test(value)) el.removeAttribute(attr.name);
      else if (name === "src" && !SAFE_LINK.test(value) && !SAFE_IMAGE.test(value)) el.removeAttribute(attr.name);
    }
  }
  return doc.body.innerHTML;
}
