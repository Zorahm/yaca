/**
 * Japanese strings: the browser column.
 */

import type { browser as source } from "../ru/browser";

export const browser = {
  "browser.toggle": "ブラウザー",
  "browser.back": "戻る",
  "browser.forward": "進む",
  "browser.reload": "再読み込み",
  "browser.stop": "読み込みを停止",
  "browser.address": "アドレスまたは localhost:ポート",
  "browser.external": "システムのブラウザーで開く",
  "browser.devtools": "開発者ツール",
  "browser.empty_title": "まだ何も開いていません",
  "browser.empty_body": "モデルまたはあなたが開いたページがここに表示されます",
  "browser.size_model": "{size} · モデルが指定",
  "browser.reset_size": "列のサイズに戻す",
  "browser.resize": "ブラウザーの幅を変更",
  "browser.bad_address": "ページのアドレスではないようです",
  "browser.open_failed": "ページを開けませんでした",
  "browser.busy": "モデルがブラウザーを使用中",
} satisfies Record<keyof typeof source, string>;
