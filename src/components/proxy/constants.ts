export const DEFAULT_PROXY_PORT = "8787";

export const DEFAULT_REMOTE_SSH_PORT = "22";

export const DEFAULT_REMOTE_LISTEN_PORT = "8787";

export const REMOTE_DRAFTS_CACHE_KEY = "codex-tools:proxy-remote-drafts";

export const REMOTE_EXPANDED_CACHE_KEY = "codex-tools:proxy-remote-expanded-id";

export const REMOTE_SELECTED_CACHE_KEY = "codex-tools:proxy-remote-selected-id";

export const REMOTE_HISTORY_CACHE_KEY = "codex-tools:proxy-remote-history";

export const API_PROXY_REASONING_OPTION_IDS = [
  "none",
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
] as const;

export const API_PROXY_SERVICE_TIER_OPTION_IDS = [
  "auto",
  "default",
  "fast",
  "flex",
] as const;
