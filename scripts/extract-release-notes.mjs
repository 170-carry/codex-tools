import { appendFileSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const VERSION_HEADING_PATTERN =
  /^\s*-\s*v?([0-9]+(?:\.[0-9]+){1,3}(?:[-+][^\s:：]+)?)(?:\s*[:：-]\s*(.*))?\s*$/i;
const UNRELEASED_HEADING_PATTERN = /^\s*###\s+(?:unreleased|未发布)\s*$/i;
const SECTION_HEADING_PATTERN = /^\s*#{1,3}\s+/;

const args = process.argv.slice(2);
const writeGithubOutput = args.includes("--github-output");
const requestedTag = args.find((arg) => !arg.startsWith("--"));

if (!requestedTag) {
  throw new Error("Usage: node scripts/extract-release-notes.mjs <tag> [--github-output]");
}

const targetVersion = requestedTag.trim().replace(/^refs\/tags\//i, "").replace(/^v/i, "");
const changelogPath = resolve(process.cwd(), "changelog.md");
const releaseTag = requestedTag.trim().replace(/^refs\/tags\//i, "");
const lines = readOptionalChangelog(changelogPath);
const exactBody = lines
  ? normalizeReleaseBody(extractVersionEntry(lines, targetVersion) ?? [])
  : "";
const unreleasedBody = lines
  ? normalizeReleaseBody(extractUnreleasedEntry(lines) ?? [])
  : "";

let body = exactBody;
let notesSource = "version";

if (!body && unreleasedBody) {
  body = unreleasedBody;
  notesSource = "unreleased";
  emitWarning(
    `No release-note entry matches ${releaseTag}; using optional Unreleased notes. / ` +
      `没有与 ${releaseTag} 对应的更新说明，将使用可选的 Unreleased 内容。`,
  );
} else if (!body) {
  body = createGenericReleaseBody(releaseTag);
  notesSource = "generated";
  emitWarning(
    `No optional release notes were found for ${releaseTag}; using generated generic text. ` +
      `The build and release will continue. / 未找到 ${releaseTag} 的可选更新说明，将使用自动生成的通用文字；构建和发布将继续。`,
  );
}

if (writeGithubOutput) {
  const outputPath = process.env.GITHUB_OUTPUT;
  if (!outputPath) {
    throw new Error("GITHUB_OUTPUT is required with --github-output.");
  }
  const delimiter = `CODEX_TOOLS_RELEASE_NOTES_${Date.now()}`;
  appendFileSync(
    outputPath,
    `body<<${delimiter}\n${body}\n${delimiter}\nnotes_source=${notesSource}\n`,
    "utf8",
  );
} else {
  process.stdout.write(`${body}\n`);
}

function readOptionalChangelog(path) {
  try {
    return readFileSync(path, "utf8").split(/\r?\n/);
  } catch (error) {
    if (error && typeof error === "object" && "code" in error && error.code === "ENOENT") {
      return null;
    }
    throw error;
  }
}

function createGenericReleaseBody(tag) {
  return [
    "## English",
    "",
    `Release ${tag}`,
    "",
    "Build matrix includes:",
    "- macOS Apple Silicon (aarch64)",
    "- macOS Intel (x86_64)",
    "- Windows",
    "",
    "## 中文",
    "",
    `发布 ${tag}`,
    "",
    "构建平台包括：",
    "- macOS Apple Silicon（aarch64）",
    "- macOS Intel（x86_64）",
    "- Windows",
  ].join("\n");
}

function emitWarning(message) {
  if (writeGithubOutput) {
    process.stdout.write(
      `::warning title=Optional release notes / 可选更新说明::${escapeWorkflowCommand(message)}\n`,
    );
    return;
  }
  process.stderr.write(`[release-notes] Warning: ${message}\n`);
}

function escapeWorkflowCommand(value) {
  return value.replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");
}

function extractVersionEntry(changelogLines, version) {
  let collecting = false;
  const entry = [];

  for (const line of changelogLines) {
    const match = line.match(VERSION_HEADING_PATTERN);
    if (match) {
      if (collecting) {
        break;
      }
      collecting = normalizeVersion(match[1]) === normalizeVersion(version);
      if (collecting && match[2]?.trim()) {
        entry.push(match[2].trim());
      }
      continue;
    }

    if (collecting) {
      entry.push(line);
    }
  }

  return collecting || entry.length > 0 ? entry : null;
}

function extractUnreleasedEntry(changelogLines) {
  let collecting = false;
  const entry = [];

  for (const line of changelogLines) {
    if (UNRELEASED_HEADING_PATTERN.test(line)) {
      collecting = true;
      continue;
    }
    if (!collecting) {
      continue;
    }
    if (VERSION_HEADING_PATTERN.test(line) || SECTION_HEADING_PATTERN.test(line)) {
      break;
    }
    entry.push(line);
  }

  return entry;
}

function normalizeReleaseBody(entryLines) {
  const bodyLines = entryLines.map((line) => {
    if (/^\s*####\s+English\s*$/i.test(line)) {
      return "## English";
    }
    if (/^\s*####\s+中文\s*$/.test(line)) {
      return "## 中文";
    }
    return line.trimEnd();
  });

  while (bodyLines[0]?.trim().length === 0) {
    bodyLines.shift();
  }
  while (bodyLines.at(-1)?.trim().length === 0) {
    bodyLines.pop();
  }

  return bodyLines.join("\n");
}

function normalizeVersion(version) {
  return version.trim().replace(/^v/i, "");
}
