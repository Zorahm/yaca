/**
 * Japanese strings: providers — the settings section and the panel before it.
 */

import type { providers as source } from "../ru/providers";

export const providers = {
  // Providers: the panel that exists until the settings window.
  "prov.title": "プロバイダー",
  "prov.home_button": "プロバイダーを設定",
  "prov.add": "プロバイダーを追加",
  "prov.edit_row": "設定",
  "prov.delete": "削除",
  "prov.delete_confirm": "完全に削除しますか？",
  "prov.models_count.one": "モデル {n} 件",
  "prov.models_count.few": "モデル {n} 件",
  "prov.models_count.many": "モデル {n} 件",

  "prov.form_new": "新しいプロバイダー",
  "prov.form_name": "名前",
  "prov.form_more": "詳細設定",
  "prov.form_protocol": "プロトコル",
  "prov.form_base_url": "ベース URL",
  "prov.form_base_url_hint":
    "プロトコルのパスは自動で追加されます: OpenAI は /v1/chat/completions、Anthropic は /v1/messages。OpenRouter の場合は https://openrouter.ai/api です。アドレスにすでにバージョン（/v1、/v1beta、/v4）が含まれている場合、/v1 は追加されません。",
  "prov.form_headers": "カスタムヘッダー",
  "prov.form_headers_hint":
    "プロトコル標準のヘッダーに追加され、それを置き換えることもあります（authorization を含む）。値の中の {key} はキーチェーンのキーに置き換えられます。キー自体がファイルに書き込まれることはありません。値を空にするとプロトコルのヘッダーを削除します。",
  "prov.form_header_name": "名前",
  "prov.form_header_value": "値",
  "prov.form_add_header": "ヘッダーを追加",
  "prov.form_remove_header": "ヘッダーを削除",

  "prov.form_key_title": "キー",
  "prov.form_key_hint":
    "プロバイダー名でシステムのキーチェーンに保存されます — providers.json には保存されません",
  "prov.form_key_new_hint":
    "保存後、プロバイダーにモデル一覧を問い合わせます。返ってきたものがモデル選択に表示されます",
  "prov.key_save": "キーを保存",
  "prov.key_forget": "キーを消去",
  "prov.key_source_keychain": "キーはシステムのキーチェーンにあります",
  "prov.key_source_environment": "キーは環境変数にあります",
  "prov.key_source_none": "キーがありません",
  "prov.key_source_unknown": "保存後にキーの保存先が表示されます",
  "prov.key_not_needed": "キーは不要です: {key} はどのヘッダーにも含まれていません",

  "prov.form_models": "モデル",
  "prov.form_models_hint":
    "一覧はプロバイダー自身から取得されます。キーを保存するとすぐに問い合わせられ、「問い合わせ」を押すと再度問い合わせます。返ってきた内容がそのまま使われます。料金は百万トークンあたりのドルで、0 は「無料または不明」を意味します。",
  "prov.form_model_context": "コンテキスト（トークン）",
  "prov.form_model_output": "最大出力（トークン）",
  "prov.form_model_effort": "推論の強度に対応",
  "prov.models_ask": "問い合わせ",
  "prov.models_asking": "問い合わせ中…",
  "prov.models_none": "モデルがありません",
  "prov.models_price": "$ / 1M 入力・出力",
  "prov.models_effort": "推論の強度",

  "prov.form_save": "プロバイダーを保存",
  "prov.form_cancel": "キャンセル",

  "prov.saved_unasked":
    "「{name}」は保存されましたが、モデル一覧に応答しませんでした: {error}",
  "prov.error_name": "プロバイダーの名前を入力してください",
  "prov.error_url": "ベース URL を入力してください",
  "prov.error_header_name": "ヘッダー {n} に名前がありません",
  "prov.error_header_dup": "ヘッダー {name} が二重に指定されています",

  "prov.lede": "会話とキーの送信先。アプリケーション全体の設定で、プロジェクトでは決められません。",
  "prov.answered": "モデル一覧に応答しました",
  "prov.unasked": "まだ問い合わせていません",
  "prov.unasked_note":
    "空の丸は、プロバイダーがモデル一覧に応答しなかったことを示します。モデル選択には表示されません: セッションはリクエストを送る先がないためです。",
} satisfies Record<keyof typeof source, string>;
