import { dateFormatter, getSystemTimeZone } from "./intlFormatters.ts";

export type QuotaTime = {
  date: string;
  time: string;
  iso: string;
  timeZone: string;
  offset: string;
};

/** Convert a backend Unix timestamp into an unambiguous local date and time. */
export function formatQuotaTime(
  epochSeconds: number | null | undefined,
  timeZone = getSystemTimeZone(),
): QuotaTime | null {
  if (epochSeconds == null || !Number.isFinite(epochSeconds)) return null;
  const value = new Date(epochSeconds * 1000);
  if (Number.isNaN(value.getTime())) return null;
  const parts = dateFormatter("en-GB", {
    timeZone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZoneName: "shortOffset",
  }).formatToParts(value);
  const part = (type: Intl.DateTimeFormatPartTypes) =>
    parts.find((entry) => entry.type === type)?.value ?? "";
  return {
    date: `${part("year")}/${part("month")}/${part("day")}`,
    time: `${part("hour")}:${part("minute")}`,
    iso: value.toISOString(),
    timeZone,
    offset: part("timeZoneName").replace("GMT", "UTC"),
  };
}
