import { dateFormatter } from "./intlFormatters.ts";

export function formatFullDate(
  epochSec: number | null | undefined,
  locale: string,
  emptyValue: string,
): string {
  if (!epochSec) {
    return emptyValue;
  }
  const date = new Date(epochSec * 1000);
  if (Number.isNaN(date.getTime())) return emptyValue;

  return dateFormatter(locale, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
