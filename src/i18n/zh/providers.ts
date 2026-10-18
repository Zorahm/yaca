/**
 * Simplified Chinese strings: providers — the settings section and the panel before it.
 */

import type { providers as source } from "../ru/providers";

export const providers = {
  // Providers: the panel that exists until the settings window.
  "prov.title": "提供商",
  "prov.home_button": "配置提供商",
  "prov.add": "添加提供商",
  "prov.edit_row": "配置",
  "prov.delete": "删除",
  "prov.delete_confirm": "永久删除？",
  "prov.models_count.one": "{n} 个模型",
  "prov.models_count.few": "{n} 个模型",
  "prov.models_count.many": "{n} 个模型",

  "prov.form_new": "新建提供商",
  "prov.form_name": "名称",
  "prov.form_more": "高级",
  "prov.form_protocol": "协议",
  "prov.form_base_url": "基础 URL",
  "prov.form_base_url_hint":
    "协议路径会自动添加：OpenAI 为 /v1/chat/completions，Anthropic 为 /v1/messages。OpenRouter 为 https://openrouter.ai/api。如果地址中已包含版本号（/v1、/v1beta、/v4），则不再添加 /v1。",
  "prov.form_headers": "自定义请求头",
  "prov.form_headers_hint":
    "添加到协议自带的请求头之上，并可覆盖它们——包括 authorization。值中的 {key} 会替换为钥匙串中的密钥；密钥本身永远不会写入文件。值为空时将移除协议自带的请求头。",
  "prov.form_header_name": "名称",
  "prov.form_header_value": "值",
  "prov.form_add_header": "添加请求头",
  "prov.form_remove_header": "移除请求头",

  "prov.form_key_title": "密钥",
  "prov.form_key_hint":
    "保存到系统钥匙串中，以提供商名称为键——providers.json 中不保存密钥",
  "prov.form_key_new_hint":
    "保存后将自动向提供商查询模型列表：它返回什么，模型选择器中就显示什么",
  "prov.key_save": "保存密钥",
  "prov.key_forget": "忘记密钥",
  "prov.key_source_keychain": "密钥在系统钥匙串中",
  "prov.key_source_environment": "密钥在环境变量中",
  "prov.key_source_none": "没有密钥",
  "prov.key_source_unknown": "保存提供商后即可看到密钥来源",
  "prov.key_not_needed": "无需密钥：{key} 未出现在任何请求头中",

  "prov.form_models": "模型",
  "prov.form_models_hint":
    "列表由提供商自身提供：保存密钥后会立即查询，点击“查询”则再次查询。提供商返回什么，就与之通信；价格以每百万 tokens 的美元数计，0 表示免费或未知。",
  "prov.form_model_context": "上下文，tokens",
  "prov.form_model_output": "最大输出，tokens",
  "prov.form_model_effort": "支持强度调节",
  "prov.models_ask": "查询",
  "prov.models_asking": "查询中…",
  "prov.models_none": "没有模型",
  "prov.models_price": "$ / 1M 输入与输出",
  "prov.models_effort": "强度",

  "prov.form_save": "保存提供商",
  "prov.form_cancel": "取消",

  "prov.saved_unasked":
    "“{name}”已保存，但未返回模型列表：{error}",
  "prov.error_name": "请填写名称",
  "prov.error_url": "请填写基础 URL",
  "prov.error_header_name": "第 {n} 个请求头没有名称",
  "prov.error_header_dup": "请求头 {name} 重复出现",

  "prov.lede": "对话和密钥发送到的位置。应用级设置：项目无法决定这一点。",
  "prov.answered": "已返回模型列表",
  "prov.unasked": "尚未查询",
  "prov.unasked_note":
    "空心圆点表示提供商从未返回模型列表。它不会出现在模型选择器中：会话将无法向其发送请求。",
} satisfies Record<keyof typeof source, string>;
