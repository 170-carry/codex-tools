const labels = {
  "zh-CN": {
    navigation: "切换页面",
    switching: "账号切换",
    warmup: "自动预热",
    about: "关于与更新",
    proxyConfiguration: "配置与访问密钥",
    developer: "开发工具",
    debugUpdate: "预览更新弹窗",
  },
  "en-US": {
    navigation: "Switch view",
    switching: "Account switching",
    warmup: "Automatic warm-up",
    about: "About & updates",
    proxyConfiguration: "Configuration & access keys",
    developer: "Developer tools",
    debugUpdate: "Preview update dialog",
  },
  "ja-JP": {
    navigation: "画面を切り替え",
    switching: "アカウント切り替え",
    warmup: "自動ウォームアップ",
    about: "情報とアップデート",
    proxyConfiguration: "設定とアクセスキー",
    developer: "開発ツール",
    debugUpdate: "更新ダイアログを表示",
  },
  "ko-KR": {
    navigation: "화면 전환",
    switching: "계정 전환",
    warmup: "자동 워밍업",
    about: "정보 및 업데이트",
    proxyConfiguration: "설정 및 액세스 키",
    developer: "개발 도구",
    debugUpdate: "업데이트 창 미리 보기",
  },
  "ru-RU": {
    navigation: "Переключить раздел",
    switching: "Переключение аккаунтов",
    warmup: "Автоматический прогрев",
    about: "О приложении и обновления",
    proxyConfiguration: "Настройки и ключи доступа",
    developer: "Инструменты разработчика",
    debugUpdate: "Предпросмотр обновления",
  },
};

export function getWorkspaceCopy(locale: string) {
  return labels[locale as keyof typeof labels] ?? labels["en-US"];
}
