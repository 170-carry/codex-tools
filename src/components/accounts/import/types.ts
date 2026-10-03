export type AccountImportRoute =
  | "oauth"
  | "current"
  | "session"
  | "upload"
  | "api";
export type AccountImportRouteOption = {
  id: AccountImportRoute;
  label: string;
  description: string;
};
