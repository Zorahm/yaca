/** Display paths keep the useful ends; full paths remain available for copying. */
export function compactPluginPath(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/^\/\/\?\//, "");
  const home = normalized.replace(/^(?:[A-Za-z]:\/Users\/[^/]+|\/Users\/[^/]+|\/home\/[^/]+)(?=\/)/i, "~");
  if (home.length <= 56) return home;
  const parts = home.split("/");
  const tail = parts.pop() ?? "";
  return `${parts.slice(0, 3).join("/")}/…/${tail}`;
}

export function newerPluginVersion(current: string | null, next: string | null): boolean {
  if (!current || !next) return false;
  const parse = (v: string) => /^v?(\d+)\.(\d+)\.(\d+)(?:[-+].*)?$/.exec(v)?.slice(1).map(Number);
  const a = parse(current), b = parse(next);
  if (!a || !b) return false;
  for (let i = 0; i < 3; i++) {
    if (a[i] !== b[i]) return b[i] > a[i];
  }
  return current.includes("-") && !next.includes("-");
}

export function marketplaceSourceTarget(source: string): { kind: "url" | "path"; value: string } | null {
  if (/^https?:\/\//i.test(source)) return { kind: "url", value: source.replace(/\.git$/, "") };
  const ssh = /^git@([^:]+):(.+?)(?:\.git)?$/.exec(source);
  if (ssh) return { kind: "url", value: `https://${ssh[1]}/${ssh[2]}` };
  if (/^[\w.-]+\/[\w.-]+$/.test(source)) return { kind: "url", value: `https://github.com/${source}` };
  if (/^(?:[A-Za-z]:[\\/]|\/|\\\\)/.test(source)) return { kind: "path", value: source };
  return null;
}

export function showMarketplaceSource(name: string, source: string): boolean {
  return source !== name && !(/^[\w.-]+\/[\w.-]+$/.test(source) && source.split("/")[1] === name);
}
