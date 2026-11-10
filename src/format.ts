import type { SyncOptions } from "./types";

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** "9 Nov, 17:47"; the year is added when it isn't the current one. */
export function formatDate(iso: string | null): string {
  if (!iso) return "Never";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    year: d.getFullYear() === new Date().getFullYear() ? undefined : "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Whole calendar days between two local dates (ignoring the time of day). */
function calendarDaysBetween(earlier: Date, later: Date): number {
  const start = new Date(earlier.getFullYear(), earlier.getMonth(), earlier.getDate());
  const end = new Date(later.getFullYear(), later.getMonth(), later.getDate());
  return Math.round((end.getTime() - start.getTime()) / 86_400_000);
}

/**
 * "3 hours ago" within the same day, "Yesterday" / "3 days ago" by calendar
 * day for the last week, the plain date beyond that.
 */
export function formatRelative(iso: string | null, now: Date = new Date()): string {
  if (!iso) return "Never";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const minutes = Math.floor((now.getTime() - d.getTime()) / 60_000);
  if (minutes < 1) return "Just now";
  const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
  const days = calendarDaysBetween(d, now);
  let text: string;
  if (minutes < 60) text = rtf.format(-minutes, "minute");
  else if (days === 0) text = rtf.format(-Math.floor(minutes / 60), "hour");
  else if (days <= 7) text = rtf.format(-days, "day");
  else return formatDate(iso);
  // "yesterday" stands alone in the UI, so it starts with a capital.
  return text.charAt(0).toLocaleUpperCase() + text.slice(1);
}

export function hasExtensionData(o: SyncOptions): boolean {
  return o.extensionStorage || o.extensionPermissions || o.extensionShortcuts;
}

export function syncsAnything(o: SyncOptions): boolean {
  return (
    o.sineMods ||
    o.modSettings ||
    hasExtensionData(o) ||
    o.zenShortcuts ||
    o.aboutConfig
  );
}

/** "Sine mods, mod settings and extension data" for the enabled options. */
export function describeSynced(o: SyncOptions): string {
  const items = [
    o.sineMods && "Sine mods",
    o.modSettings && "mod settings",
    hasExtensionData(o) && "extension data",
    o.zenShortcuts && "Zen shortcuts",
    o.aboutConfig && "about:config prefs",
  ].filter((item): item is string => Boolean(item));
  if (items.length <= 1) return items.join("");
  return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}
