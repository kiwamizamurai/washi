const SCHEME = /^[a-zA-Z][a-zA-Z0-9+.-]*:/;

export const basename = (path: string) => path.split("/").pop() ?? path;

export const dirname = (path: string) => path.slice(0, Math.max(path.lastIndexOf("/"), 0)) || "/";

export const extensionOf = (path: string) =>
  basename(path).split(".").slice(1).pop()?.toLowerCase() ?? "";

export function resolveLink(basePath: string, href: string): string | null {
  if (!href || href.startsWith("#") || href.startsWith("//") || SCHEME.test(href)) {
    return null;
  }
  const file = href.split(/[#?]/)[0];
  if (!file) return null;

  let decoded: string;
  try {
    decoded = decodeURIComponent(file);
  } catch {
    return null;
  }

  const parts = decoded.startsWith("/")
    ? []
    : basePath.split("/").slice(0, -1);
  for (const segment of decoded.split("/")) {
    if (segment === "" || segment === ".") continue;
    if (segment === "..") parts.pop();
    else parts.push(segment);
  }
  return `/${parts.filter(Boolean).join("/")}`;
}
