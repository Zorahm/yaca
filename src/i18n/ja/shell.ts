/**
 * Japanese strings: the frame of the window — sidebar, top bar, title bar,
 * menus, home screen.
 */

import type { shell as source } from "../ru/shell";

export const shell = {
  "side.new": "新しいチャット",
  "side.projects": "プロジェクト",
  "side.new_here": "このプロジェクトで新しいチャット",
  "side.toggle": "サイドバー",
  "side.update_do": "アップデート",
  "side.update_more": "詳細",
  "side.forget": "プロジェクトを一覧から削除",
  "side.forget_busy": "先にこのプロジェクトのチャットを閉じてください",
  "side.no_projects": "プロジェクトはまだありません",
  "side.working": "作業中",
  "side.waiting": "回答待ち",
  "side.close_chat": "チャットを閉じる",

  "top.menu": "アプリケーションメニュー",
  "top.sessions": "セッション",
  "top.search": "このセッションを検索",
  "top.back": "戻る",
  "top.forward": "進む",
  "top.processes": "プロセス",
  "top.checkpoints": "チェックポイント",
  "panel.tab_mcp": "MCP",
  "panel.tab_lsp": "LSP",
  "panel.toggle": "右パネル",
  "panel.resize": "パネルの幅",
  "panel.tab_agents": "エージェント",
  "side.resize": "サイドバーの幅",
  "top.session_menu": "セッションメニュー",
  "top.untitled": "新しいセッション",
  "top.rename_hint": "ダブルクリックで名前を変更",

  "home.greeting": "何から始めますか？",
  "home.greeting_named": "{name}さん、何から始めますか？",
  "home.pick_project": "フォルダーを選択",
  "home.pick_title": "プロジェクトのフォルダー",
  "home.not_chosen": "未選択",
  "home.lead":
    "プロジェクトのフォルダーを選ぶと、エージェントがそのファイルを使って作業を始めます。モード、モデル、推論の強度は、最初のメッセージの前に変更できます。",
  "home.no_provider": "プロバイダーがありません。",
  "home.no_provider_hint": "メッセージを送るには、モデルのキーを追加してください。",

  "title.minimize": "最小化",
  "title.maximize": "最大化",
  "title.restore": "元に戻す",
  "title.close": "閉じる",
  "menu.help": "ヘルプ",
  "menu.check_updates": "アップデートを確認",
  "menu.about": "Yaca について",

  "common.open_project": "プロジェクトを開く",
  "common.close": "閉じる",
  "common.seconds": "{n}秒",
} satisfies Record<keyof typeof source, string>;
