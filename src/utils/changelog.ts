import changelogSource from "../../changelog.md?raw";

export type ChangelogEntry = {
  version: string;
  items: string[];
};

type DraftChangelogEntry = {
  version: string;
  lines: string[];
};

const VERSION_HEADING_PATTERN =
  /^\s*-\s*v?([0-9]+(?:\.[0-9]+){1,3}(?:[-+][^\s:：]+)?)(?:\s*[:：-]\s*(.*))?\s*$/i;

export function getChangelogEntryForVersion(
  version: string,
  source = changelogSource,
): ChangelogEntry | null {
  const targetVersion = normalizeVersion(version);
  let activeEntry: DraftChangelogEntry | null = null;

  for (const line of source.split(/\r?\n/)) {
    const headingMatch = line.match(VERSION_HEADING_PATTERN);
    if (headingMatch) {
      if (activeEntry && normalizeVersion(activeEntry.version) === targetVersion) {
        return finalizeEntry(activeEntry);
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
    return finalizeEntry(activeEntry);
  }

  return null;
}

export function getLatestChangelogEntry(source = changelogSource): ChangelogEntry | null {
  let activeEntry: DraftChangelogEntry | null = null;

  for (const line of source.split(/\r?\n/)) {
    const headingMatch = line.match(VERSION_HEADING_PATTERN);
    if (headingMatch) {
      if (activeEntry) {
        return finalizeEntry(activeEntry);
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

  return activeEntry ? finalizeEntry(activeEntry) : null;
}

export function normalizeReleaseNoteItems(body: string | null | undefined): string[] {
  return (body ?? "")
    .split(/\r?\n/)
    .map(normalizeChangelogLine)
    .filter((item): item is string => Boolean(item));
}

function finalizeEntry(entry: DraftChangelogEntry): ChangelogEntry {
  return {
    version: entry.version,
    items: entry.lines
      .map(normalizeChangelogLine)
      .filter((item): item is string => Boolean(item)),
  };
}

function normalizeVersion(version: string): string {
  return version.trim().replace(/^v/i, "");
}

function normalizeChangelogLine(line: string): string | null {
  const item = line
    .trim()
    .replace(/^(?:\d+[.)、]|[-*+])\s*/, "")
    .trim();

  return item.length > 0 ? item : null;
}
