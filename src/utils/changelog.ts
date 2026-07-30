import changelogSource from "../../changelog.md?raw";

export type ChangelogEntry = {
  version: string;
  items: string[];
};

type ReleaseNotesLocale = string | null | undefined;

type DraftChangelogEntry = {
  version: string;
  lines: string[];
};

const VERSION_HEADING_PATTERN =
  /^\s*-\s*v?([0-9]+(?:\.[0-9]+){1,3}(?:[-+][^\s:：]+)?)(?:\s*[:：-]\s*(.*))?\s*$/i;
const UNRELEASED_HEADING_PATTERN = /^\s*###\s+(?:unreleased|未发布)\s*$/i;

export function getChangelogEntryForVersion(
  version: string,
  locale?: ReleaseNotesLocale,
  source = changelogSource,
): ChangelogEntry | null {
  const targetVersion = normalizeVersion(version);
  let activeEntry: DraftChangelogEntry | null = null;

  for (const line of source.split(/\r?\n/)) {
    const headingMatch = line.match(VERSION_HEADING_PATTERN);
    if (headingMatch) {
      if (activeEntry && normalizeVersion(activeEntry.version) === targetVersion) {
        return finalizeEntry(activeEntry, locale);
      }

      activeEntry = {
        version: headingMatch[1],
        lines: [],
      };

      if (headingMatch[2]?.trim()) {
        activeEntry.lines.push(headingMatch[2]);
      }
      continue;
    }

    activeEntry?.lines.push(line);
  }

  if (activeEntry && normalizeVersion(activeEntry.version) === targetVersion) {
    return finalizeEntry(activeEntry, locale);
  }

  return null;
}

export function getLatestChangelogEntry(
  locale?: ReleaseNotesLocale,
  source = changelogSource,
): ChangelogEntry | null {
  let activeEntry: DraftChangelogEntry | null = null;

  for (const line of source.split(/\r?\n/)) {
    const headingMatch = line.match(VERSION_HEADING_PATTERN);
    if (headingMatch) {
      if (activeEntry) {
        return finalizeEntry(activeEntry, locale);
      }

      activeEntry = {
        version: headingMatch[1],
        lines: [],
      };

      if (headingMatch[2]?.trim()) {
        activeEntry.lines.push(headingMatch[2]);
      }
      continue;
    }

    activeEntry?.lines.push(line);
  }

  return activeEntry ? finalizeEntry(activeEntry, locale) : null;
}

export function getUnreleasedChangelogEntry(
  locale?: ReleaseNotesLocale,
  source = changelogSource,
): ChangelogEntry | null {
  const lines: string[] = [];
  let collecting = false;

  for (const line of source.split(/\r?\n/)) {
    if (UNRELEASED_HEADING_PATTERN.test(line)) {
      collecting = true;
      continue;
    }
    if (!collecting) {
      continue;
    }
    if (VERSION_HEADING_PATTERN.test(line) || /^\s*###\s+/.test(line)) {
      break;
    }
    lines.push(line);
  }

  const items = normalizeReleaseNoteItems(lines.join("\n"), locale);
  return items.length > 0 ? { version: "Next", items } : null;
}

export function normalizeReleaseNoteItems(
  body: string | null | undefined,
  locale?: ReleaseNotesLocale,
): string[] {
  const sections = new Map<string, string[]>();
  const unscoped: string[] = [];
  let activeLanguage: "en" | "zh" | null = null;

  for (const line of (body ?? "").split(/\r?\n/)) {
    const language = releaseNotesHeadingLanguage(line);
    if (language) {
      activeLanguage = language;
      if (!sections.has(language)) {
        sections.set(language, []);
      }
      continue;
    }

    const item = normalizeChangelogLine(line);
    if (!item) {
      continue;
    }
    if (activeLanguage) {
      sections.get(activeLanguage)?.push(item);
    } else {
      unscoped.push(item);
    }
  }

  if (sections.size === 0) {
    return unscoped;
  }

  const preferredLanguage = locale?.toLowerCase().startsWith("zh") ? "zh" : "en";
  return sections.get(preferredLanguage) ?? sections.get("en") ?? sections.get("zh") ?? unscoped;
}

function finalizeEntry(
  entry: DraftChangelogEntry,
  locale?: ReleaseNotesLocale,
): ChangelogEntry {
  return {
    version: entry.version,
    items: normalizeReleaseNoteItems(entry.lines.join("\n"), locale),
  };
}

function normalizeVersion(version: string): string {
  return version.trim().replace(/^v/i, "");
}

function normalizeChangelogLine(line: string): string | null {
  if (/^\s*#{1,6}\s+/.test(line)) {
    return null;
  }
  const item = line
    .trim()
    .replace(/^(?:\d+[.)、]|[-*+])\s*/, "")
    .trim();

  return item.length > 0 ? item : null;
}

function releaseNotesHeadingLanguage(line: string): "en" | "zh" | null {
  const normalized = line
    .trim()
    .replace(/^#{1,6}\s*/, "")
    .replace(/[:：]\s*$/, "")
    .trim()
    .toLowerCase();

  if (normalized === "english" || normalized === "en") {
    return "en";
  }
  if (normalized === "中文" || normalized === "简体中文" || normalized === "zh") {
    return "zh";
  }
  return null;
}
