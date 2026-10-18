/**
 * Japanese strings: MCP servers — the settings section and the panel.
 */

import type { mcp as source } from "../ru/mcp";

export const mcp = {
  "mcp.title": "MCP サーバー",
  "mcp.lede_app":
    "あなたのサーバー: すべてのプロジェクトで接続されます。そのツールは、組み込みのツールと並んでエージェントに提供されます。",
  "mcp.lede_project":
    "このプロジェクトのサーバー。.yaca/settings.json に保存され、あなたの共通リストや .mcp.json の同名のサーバーを置き換えます。",
  "mcp.add": "サーバーを追加",
  "mcp.none_declared": "サーバーはありません。",
  "mcp.edit": "編集",
  "mcp.enabled": "{name} に接続",
  "mcp.delete": "削除",
  "mcp.delete_confirm": "削除しますか？",
  "mcp.file_title": ".mcp.json から",
  "mcp.file_hint":
    "プロジェクトのファイル。他のツールも読み込みます。ここでは表示のみです。このファイルのサーバーは、チャットで承認された後に起動します: /mcp。",
  "mcp.file_replaced": "上にある同名のサーバーに置き換えられています",
  "mcp.next_chat_note":
    "一覧はチャットを開くときに読み込まれます。変更は次のチャットに反映されます。開いているチャットは、開始時のサーバーを使い続けます。",
  "mcp.form_new": "新しいサーバー",
  "mcp.form_save": "サーバーを保存",
  "mcp.form_name": "名前",
  "mcp.form_name_hint":
    "サーバーの各ツールの名前の一部になります。英字、数字、ハイフン、アンダースコアが使えます。",
  "mcp.form_kind": "接続方法",
  "mcp.kind_stdio": "プログラム",
  "mcp.kind_http": "アドレス",
  "mcp.form_command": "プログラム",
  "mcp.form_command_hint": "シェルを介さず直接起動します。",
  "mcp.form_args": "引数",
  "mcp.form_args_hint": "1行に1つ。",
  "mcp.form_env": "環境変数",
  "mcp.form_env_hint":
    "NAME=値、1行に1つ。値は例のように環境変数を参照できます。そうすればトークンがファイルに残りません。",
  "mcp.form_cwd": "作業ディレクトリ",
  "mcp.form_cwd_hint": "空欄の場合はプロジェクトのルート。",
  "mcp.form_url": "アドレス",
  "mcp.form_url_hint":
    "Streamable HTTP に対応したサーバー。OAuth によるサインインには対応していません: トークンはヘッダーで指定します。",
  "mcp.form_headers": "ヘッダー",
  "mcp.form_headers_hint":
    "名前: 値、1行に1つ。値は例のように環境変数を参照できます。",
  "mcp.problem_name_empty": "名前を入力してください。",
  "mcp.problem_name_chars": "名前には英字、数字、ハイフン、アンダースコアを使えます。",
  "mcp.problem_name_taken": "同じ名前のサーバーがすでにあります。",
  "mcp.problem_command_empty": "プログラムを入力してください。",
  "mcp.problem_env": "環境変数の各行は NAME=値 の形式にしてください。",
  "mcp.problem_url": "アドレスは http:// または https:// で始まる必要があります。",
  "mcp.problem_headers": "ヘッダーの各行は 名前: 値 の形式にしてください。",

  "mcp.panel_empty":
    "このチャットには MCP サーバーがありません。設定で宣言すると、次のチャットに表示されます。",
  "mcp.status_connecting": "接続中",
  "mcp.status_ready": "準備完了",
  "mcp.status_failed": "失敗",
  "mcp.status_disabled": "オフ",
  "mcp.status_needs_approval": "承認待ち",
  "mcp.source_global": "共通リスト",
  "mcp.source_project_file": "プロジェクトの .mcp.json",
  "mcp.source_project": "プロジェクト設定",
  "mcp.source_plugin": "プラグイン",
  "mcp.source_app": "アプリ",
  "mcp.approve": "承認",
  "mcp.reconnect": "再接続",
  "mcp.status_needs_login": "ログインが必要",
  "mcp.status_logging_in": "ログイン中",
  "mcp.login": "ログイン",
  "mcp.logout": "ログアウト",
  "mcp.login_note": "ブラウザーでログインを完了してください。完了するとサーバーに接続します。",
  "mcp.login_open": "ログインページを開く",
  "lsp.panel_empty": "このチャットには言語サーバーがありません。組み込みのサーバーはプログラムが PATH にあれば起動します。その他は設定で追加します。",
  "lsp.status_idle": "未起動",
  "lsp.status_starting": "起動中",
  "lsp.status_ready": "準備完了",
  "lsp.status_failed": "失敗",
  "lsp.status_disabled": "オフ",
  "lsp.restart": "再起動",
  "lsp.start": "起動",
  "lsp.extensions": "ファイル: {list}",
  "mcp.approval_note":
    "プロジェクトの .mcp.json に宣言されていて、まだ起動していません。承認すると上のプログラムが起動し、宣言が変わるまで記憶されます。",
  "mcp.tools.one": "ツール {n} 件",
  "mcp.tools.few": "ツール {n} 件",
  "mcp.tools.many": "ツール {n} 件",
  "mcp.resources": "リソース",
  "mcp.tool_reads": "読み取り",
  "mcp.tool_writes": "書き込み",
} satisfies Record<keyof typeof source, string>;
