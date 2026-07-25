import assert from "node:assert/strict";
import test from "node:test";
import {
  HEATMAP_LEVEL_COUNT,
  tokenHeatmapLevel,
} from "../src/utils/heatmapScale.ts";

const CURRENT_HEATMAP_MAX = 658_728_321;

test("keeps empty buckets visually separate", () => {
  assert.equal(tokenHeatmapLevel(0, CURRENT_HEATMAP_MAX), 0);
  assert.equal(tokenHeatmapLevel(-1, CURRENT_HEATMAP_MAX), 0);
  assert.equal(tokenHeatmapLevel(Number.NaN, CURRENT_HEATMAP_MAX), 0);
});

test("maps the current example buckets to visibly different levels", () => {
  assert.equal(tokenHeatmapLevel(147_529_899, CURRENT_HEATMAP_MAX), 5);
  assert.equal(tokenHeatmapLevel(533_999_254, CURRENT_HEATMAP_MAX), 9);
});

test("uses the full nine-level range and stays monotonic", () => {
  const values = [1, 10_000_000, 50_000_000, 100_000_000, 250_000_000, 533_999_254];
  const levels = values.map((tokens) =>
    tokenHeatmapLevel(tokens, CURRENT_HEATMAP_MAX),
  );

  assert.equal(tokenHeatmapLevel(CURRENT_HEATMAP_MAX, CURRENT_HEATMAP_MAX), 9);
  assert.equal(tokenHeatmapLevel(CURRENT_HEATMAP_MAX * 2, CURRENT_HEATMAP_MAX), 9);
  assert.ok(levels.every((level) => level >= 1 && level <= HEATMAP_LEVEL_COUNT));
  assert.deepEqual(levels, [...levels].sort((left, right) => left - right));
});
