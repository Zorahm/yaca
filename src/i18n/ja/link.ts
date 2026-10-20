/**
 * Japanese strings: connecting to a server.
 */

import type { link as source } from "../ru/link";

export const link = {
  "link.title": "Yaca サーバーに接続",
  "link.intro": "Yaca はあなたのサーバーで動き、このアプリはその窓口です。インストールスクリプトが出力したアドレスとトークンを入力してください。",
  "link.address": "サーバーのアドレス",
  "link.address_hint": "例: yaca.example.com",
  "link.token": "トークン",
  "link.token_hint": "yaca-server token add が表示します。",
  "link.connect": "接続",
  "link.connecting": "接続中…",
  "link.local": "このコンピューターで作業する",
  "link.err_unreachable": "サーバーが応答しません。アドレスとネットワークを確認してください。",
  "link.err_unauthorized": "サーバーがトークンを受け付けませんでした。",
  "link.err_bad_address": "サーバーのアドレスとして認識できません。",
  "link.err_replaced": "別のデバイスがサーバーに接続し、チャットを引き継ぎました。再接続するとここに戻せます。",
  "link.err_revoked": "このデバイスのトークンは失効しました。",
  "link.lost": "サーバーとの接続が切れました。再接続しています…",
  "link.settings_nav": "接続",
  "link.settings_title": "接続",
  "link.settings_this_pc": "作業はこのコンピューターで行われます。",
  "link.settings_server": "作業はサーバー {name} で行われます。",
  "link.settings_to_server": "サーバーに接続",
  "link.settings_to_local": "このコンピューターに戻る",
  "link.settings_disconnect": "サーバーから切断",
  "link.settings_reload_note": "ウィンドウが再読み込みされます。サーバー上のチャットは動き続けます。",
  "link.pick_up": "上へ",
  "link.pick_home": "ホーム",
  "link.pick_choose": "このフォルダーを選択",
  "link.pick_new": "新しいフォルダー",
  "link.pick_new_name": "フォルダー名",
  "link.pick_create": "作成",
  "link.pick_cancel": "キャンセル",
  "link.pick_empty": "サブフォルダーはありません",
  "link.pick_hidden": "隠しフォルダー",
  "link.pick_broad": "プロジェクトではなく、ホームまたはルートのフォルダーのようです。エージェントは中のすべてを対象に作業し、巻き戻し用のスナップショットも取れません。",
  "link.pick_loading": "フォルダーを読み込み中…",
} satisfies Record<keyof typeof source, string>;
