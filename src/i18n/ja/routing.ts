/**
 * Japanese strings: the model router — the settings section, the feed note and the
 * switcher's notice.
 */

import type { routing as source } from "../ru/routing";

export const routing = {
  "routing.title": "ルーター",
  "routing.lede":
    "小さなモデルが各メッセージについての質問に答え、ルールがどのモデルが返信するかを決めます。routing.json は providers.json と同じ場所にあります。",
  "routing.state_absent": "未設定です。",
  "routing.state_ready": "接続済み：モデル一覧では「{name}」と表示されます。",
  "routing.state_broken": "未接続：{error}",
  "routing.editor": "routing.json の内容",
  "routing.save": "保存",
  "routing.remove": "ルーターを削除",
  "routing.saved": "保存しました。",
  "routing.next_chat": "すでに開いているチャットはこれまでどおり動作します。新しいチャットはこのファイルのルーターを使います。",
  "routing.default_route": "デフォルト",
  "routing.chip": "ルート：{route} → {model}",
  "routing.menu_broken": "ルーターが接続されていません：{error}",
  "routing.wizard_title": "クイック設定",
  "routing.wizard_lede":
    "モデルを3つ選ぶと、エディターに完成した設定が入ります。強いモデルが難しいリクエストを担当し、それ以外は既定のモデルが処理します。保存するまで何も書き込まれません。",
  "routing.wizard_decider": "質問に答えるモデル",
  "routing.wizard_default": "既定のモデル",
  "routing.wizard_strong": "難しい依頼向けの強いモデル",
  "routing.wizard_build": "エディターに入力",
  "routing.wizard_none": "モデルがありません。先にプロバイダーを接続してください。",
  "routing.wizard_pick": "モデルを選択",
  "routing.decision_hint":
    "判定モデルは上の「Decision」プロバイダーから選ぶのがおすすめです。ローカルまたはクラウドのアドレスを設定し、モデルを取得してください。",
} satisfies Record<keyof typeof source, string>;
