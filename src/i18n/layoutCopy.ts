const copy = {
  "zh-CN": {
    label: "界面布局",
    classic: "原版布局",
    compact: "精简布局",
    description: "默认使用原版布局，切换立即生效并记住选择。",
  },
  "en-US": {
    label: "Interface layout",
    classic: "Original layout",
    compact: "Compact layout",
    description:
      "Original is the default. Changes apply immediately and your choice is remembered.",
  },
  "ja-JP": {
    label: "画面レイアウト",
    classic: "従来のレイアウト",
    compact: "コンパクト",
    description:
      "既定は従来のレイアウトです。変更はすぐに適用され、選択が保存されます。",
  },
  "ko-KR": {
    label: "화면 레이아웃",
    classic: "기존 레이아웃",
    compact: "간결한 레이아웃",
    description:
      "기존 레이아웃이 기본값입니다. 변경은 즉시 적용되며 선택이 저장됩니다.",
  },
  "ru-RU": {
    label: "Макет интерфейса",
    classic: "Исходный макет",
    compact: "Компактный макет",
    description:
      "По умолчанию используется исходный макет. Выбор применяется сразу и сохраняется.",
  },
};
export function getLayoutCopy(locale: string) {
  return copy[locale as keyof typeof copy] ?? copy["en-US"];
}
