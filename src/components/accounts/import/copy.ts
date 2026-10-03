const messages = {
  "zh-CN": {
    manualCallback: "手动填写回调链接",
    showKey: "显示 API Key",
    hideKey: "隐藏 API Key",
    show: "显示",
    hide: "隐藏",
  },
  "en-US": {
    manualCallback: "Enter callback manually",
    showKey: "Show API key",
    hideKey: "Hide API key",
    show: "Show",
    hide: "Hide",
  },
  "ja-JP": {
    manualCallback: "コールバックを手動入力",
    showKey: "API キーを表示",
    hideKey: "API キーを隠す",
    show: "表示",
    hide: "隠す",
  },
  "ko-KR": {
    manualCallback: "콜백 링크 직접 입력",
    showKey: "API 키 표시",
    hideKey: "API 키 숨기기",
    show: "표시",
    hide: "숨기기",
  },
  "ru-RU": {
    manualCallback: "Ввести ссылку вручную",
    showKey: "Показать API-ключ",
    hideKey: "Скрыть API-ключ",
    show: "Показать",
    hide: "Скрыть",
  },
};

export function getAccountImportCopy(locale: string) {
  return messages[locale as keyof typeof messages] ?? messages["en-US"];
}
