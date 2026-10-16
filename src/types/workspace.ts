import type { useCodexController } from "../hooks/useCodexController";

export type AppTab = "accounts" | "analytics" | "proxy" | "git" | "settings";
export type CodexController = ReturnType<typeof useCodexController>;
