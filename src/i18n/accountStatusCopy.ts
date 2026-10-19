const unavailableLabels: Record<string, string> = {
  "zh-CN": "不适用",
  "en-US": "Not applicable",
  "ja-JP": "対象外",
  "ko-KR": "해당 없음",
  "ru-RU": "Не применимо",
};

export function accountUnavailableLabel(locale: string): string {
  return unavailableLabels[locale] ?? unavailableLabels["en-US"];
}
