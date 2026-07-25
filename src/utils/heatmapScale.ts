export const HEATMAP_LEVEL_COUNT = 9;

const HEATMAP_LOG_CURVE_STRENGTH = 9;

export function tokenHeatmapLevel(tokens: number, maxTokens: number): number {
  if (!Number.isFinite(tokens) || tokens <= 0) {
    return 0;
  }

  const safeMaxTokens =
    Number.isFinite(maxTokens) && maxTokens > 0 ? maxTokens : tokens;
  const ratio = Math.min(1, Math.max(0, tokens / safeMaxTokens));
  const normalized =
    Math.log1p(HEATMAP_LOG_CURVE_STRENGTH * ratio) /
    Math.log1p(HEATMAP_LOG_CURVE_STRENGTH);

  return Math.max(
    1,
    Math.min(HEATMAP_LEVEL_COUNT, Math.ceil(normalized * HEATMAP_LEVEL_COUNT)),
  );
}
