export type AuthorizationApp = { path: string; name: string };
export const preferenceKey = "codex-tools.authorization-app";
export function savedAuthorizationApp(): AuthorizationApp | null {
  const saved = window.localStorage.getItem(preferenceKey);
  return saved ? (JSON.parse(saved) as AuthorizationApp) : null;
}
