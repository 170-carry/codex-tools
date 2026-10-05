import assert from "node:assert/strict";
import test from "node:test";
import { dateFormatter, numberFormatter } from "../src/utils/intlFormatters.ts";
import { formatFullDate } from "../src/utils/dateFormatting.ts";
import { formatResetValue } from "../src/components/accounts/accountPresentation.ts";

test("malformed backend dates remain empty instead of crashing account rows", () => {
  for (const timestamp of [undefined, null, NaN, Infinity, 1e20]) {
    assert.equal(formatFullDate(timestamp, "zh-CN", "--"), "--");
    assert.equal(formatResetValue(timestamp, "zh-CN", "--"), "--");
  }
});

test("shared date rules preserve locale, explicit zones and daylight-saving offsets", () => {
  for (const locale of ["zh-CN", "en-US", "ja-JP", "ko-KR", "ru-RU"]) {
    for (const timeZone of ["Asia/Shanghai", "America/New_York", "Asia/Kolkata"]) {
      const options: Intl.DateTimeFormatOptions = {
        timeZone, year: "numeric", month: "2-digit", day: "2-digit",
        hour: "2-digit", minute: "2-digit", timeZoneName: "shortOffset",
      };
      for (const date of [new Date(0), new Date("2026-01-01T00:00:00Z"), new Date("2026-07-01T00:00:00Z")]) {
        assert.equal(dateFormatter(locale, options).format(date), new Intl.DateTimeFormat(locale, options).format(date));
      }
    }
  }
});

test("shared number rules preserve compact notation, rounding and currencies", () => {
  for (const locale of ["zh-CN", "en-US", "ja-JP", "ko-KR", "ru-RU"]) {
    for (const options of [
      {},
      { notation: "compact", maximumFractionDigits: 1 },
      { style: "currency", currency: "USD", minimumFractionDigits: 4, maximumFractionDigits: 4 },
    ] satisfies Intl.NumberFormatOptions[]) {
      for (const value of [-1, 0, .0123, 999, 1000, 1234567]) {
        assert.equal(numberFormatter(locale, options).format(value), new Intl.NumberFormat(locale, options).format(value));
      }
    }
  }
});
