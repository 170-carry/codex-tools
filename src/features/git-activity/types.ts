export type GitSource = { kind: 'local' | 'remote'; location: string };
export type GitActivityConfig = {
  sources: GitSource[];
  aliases: Record<string, string>;
  excludedAuthors: string[];
  restDays: number[];
  threshold: number;
  syncRemote: boolean;
};
export type GitCommit = {
  sha: string; repo_key: string; person: string; author: string; email: string;
  subject: string; commit_day: string; source_add: number; source_del: number;
  source_files: number; bulk: boolean; keep: boolean; company_unique: boolean; all_unique: boolean;
};
export type GitActivityReport = {
  start: string; end: string; snapshot: string;
  people: { identity: string; person: string; names: string[]; emails: string[]; company_unique: number }[];
  authors?: { name: string; email: string }[];
  repos: { key: string; repo: string; category: string; synced: boolean; shallow: boolean }[];
  rows: GitCommit[]; warnings: string[]; config: GitActivityConfig;
};
export const defaultGitConfig: GitActivityConfig = {
  sources: [], aliases: {}, excludedAuthors: [], restDays: [0], threshold: 500, syncRemote: true,
};
