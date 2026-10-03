import assert from "node:assert/strict";
import test from "node:test";
import { formatQuotaTime } from "../src/utils/quotaTime.ts";

const seconds = (iso: string) => Date.parse(iso) / 1000;

test("reset time includes year, date and time in the selected system zone", () => {
  assert.deepEqual(
    formatQuotaTime(seconds("2026-10-02T07:05:00Z"), "Asia/Shanghai"),
    {
      date: "2026/10/02",
      time: "15:05",
      iso: "2026-10-02T07:05:00.000Z",
      timeZone: "Asia/Shanghai",
      offset: "UTC+8",
    },
  );
});

test("timezone conversion preserves a reset that crosses the year boundary", () => {
  const value = formatQuotaTime(
    seconds("2026-12-31T20:00:00Z"),
    "Asia/Shanghai",
  );
  assert.equal(value?.date, "2027/01/01");
  assert.equal(value?.time, "04:00");
});

test("midnight uses 00:00 rather than 24:00", () => {
  assert.equal(
    formatQuotaTime(seconds("2026-10-01T16:00:00Z"), "Asia/Shanghai")?.time,
    "00:00",
  );
});

test("missing and malformed reset timestamps remain unknown", () => {
  for (const value of [null, undefined, NaN, Infinity, -Infinity, 1e20]) {
    assert.equal(formatQuotaTime(value, "UTC"), null);
  }
});

test("a valid Unix epoch of zero is not treated as missing", () => {
  assert.equal(formatQuotaTime(0, "UTC")?.date, "1970/01/01");
  assert.equal(formatQuotaTime(0, "UTC")?.time, "00:00");
});

test("half-hour offsets are retained", () => {
  const value = formatQuotaTime(
    seconds("2026-10-02T07:00:00Z"),
    "Asia/Kolkata",
  );
  assert.equal(value?.time, "12:30");
  assert.equal(value?.offset, "UTC+5:30");
});

test("each reset uses the offset at that date across a daylight-saving transition", () => {
  const earlier = formatQuotaTime(
    seconds("2026-11-01T05:30:00Z"),
    "America/New_York",
  );
  const later = formatQuotaTime(
    seconds("2026-11-01T06:30:00Z"),
    "America/New_York",
  );
  assert.equal(earlier?.time, "01:30");
  assert.equal(later?.time, "01:30");
  assert.equal(earlier?.offset, "UTC-4");
  assert.equal(later?.offset, "UTC-5");
  assert.notEqual(earlier?.iso, later?.iso);
});
