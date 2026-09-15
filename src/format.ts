import type { SyncOptions } from "./types";

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function formatDate(iso: string | null): string {
  if (!iso) return "Never";
  try {
    const d = new Date(iso);
    return d.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

export function hasExtensionData(o: SyncOptions): boolean {
  return o.extensionStorage || o.extensionPermissions || o.extensionShortcuts;
}

export function syncsAnything(o: SyncOptions): boolean {
  return o.sineMods || o.modSettings || hasExtensionData(o);
}

/** "Sine mods, mod settings and extension data" for the enabled options. */
export function describeSynced(o: SyncOptions): string {
  const items = [
    o.sineMods && "Sine mods",
    o.modSettings && "mod settings",
    hasExtensionData(o) && "extension data",
  ].filter((item): item is string => Boolean(item));
  if (items.length <= 1) return items.join("");
  return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}
