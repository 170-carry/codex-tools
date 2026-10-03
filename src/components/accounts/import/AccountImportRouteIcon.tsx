import type { AccountImportRoute } from "./types";

export function AccountImportRouteIcon({
  route,
}: {
  route: AccountImportRoute;
}) {
  if (route === "oauth") {
    return (
      <svg
        className="iconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M12 3a9 9 0 1 0 9 9" />
        <path d="M12 3v6l4 2" />
        <path d="M21 5v4h-4" />
      </svg>
    );
  }

  if (route === "current") {
    return (
      <svg
        className="iconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M12 4v16" />
        <path d="m7 9 5-5 5 5" />
        <path d="M5 19h14" />
      </svg>
    );
  }

  if (route === "api") {
    return (
      <svg
        className="iconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M4 8.5h16" />
        <path d="M4 15.5h16" />
        <path d="M7 4.5v15" />
        <path d="M17 4.5v15" />
      </svg>
    );
  }

  if (route === "session") {
    return (
      <svg
        className="iconGlyph"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M9 3h6" />
        <path d="M10 3v4.5L5.8 17a3 3 0 0 0 2.7 4.2h7a3 3 0 0 0 2.7-4.2L14 7.5V3" />
        <path d="M8 14h8" />
      </svg>
    );
  }

  return (
    <svg
      className="iconGlyph"
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M12 16V4" />
      <path d="m7 11 5 5 5-5" />
      <path d="M5 20h14" />
    </svg>
  );
}
