// Creating Intl formatters is expensive inside account rows and chart hovers.
// Keep a small cache of formatting rules, never a cache of user values.
const dates = new Map<string, Intl.DateTimeFormat>();
const numbers = new Map<string, Intl.NumberFormat>();
const CACHE_LIMIT = 32;
let systemTimeZone: string | undefined;

export function getSystemTimeZone(): string {
  return systemTimeZone ??= Intl.DateTimeFormat().resolvedOptions().timeZone;
}

export function dateFormatter(locale?: string, options: Intl.DateTimeFormatOptions = {}) {
  const key = JSON.stringify([locale, options]);
  let formatter = dates.get(key);
  if (!formatter) {
    formatter = new Intl.DateTimeFormat(locale, options);
    if (dates.size >= CACHE_LIMIT) dates.clear();
    dates.set(key, formatter);
  }
  return formatter;
}

export function numberFormatter(locale?: string, options: Intl.NumberFormatOptions = {}) {
  const key = JSON.stringify([locale, options]);
  let formatter = numbers.get(key);
  if (!formatter) {
    formatter = new Intl.NumberFormat(locale, options);
    if (numbers.size >= CACHE_LIMIT) numbers.clear();
    numbers.set(key, formatter);
  }
  return formatter;
}

// Pick up OS locale/time-zone changes when returning from system settings.
if (typeof window !== "undefined") {
  window.addEventListener("focus", () => {
    dates.clear();
    numbers.clear();
    systemTimeZone = undefined;
  });
}
