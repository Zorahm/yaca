/**
 * Chinese strings: the model router — the settings section, the feed note and the
 * switcher's notice.
 */

import type { routing as source } from "../ru/routing";

export const routing = {
  "routing.title": "路由器",
  "routing.lede": "一个小模型回答关于每条消息的问题，再由规则决定由哪个模型来回复。routing.json 文件与 providers.json 放在一起。",
  "routing.state_absent": "尚未设置。",
  "routing.state_ready": "已连接：它在模型列表中显示为“{name}”。",
  "routing.state_broken": "未连接：{error}",
  "routing.editor": "routing.json 的内容",
  "routing.save": "保存",
  "routing.remove": "移除路由器",
  "routing.saved": "已保存。",
  "routing.next_chat": "已打开的聊天保持原样；新聊天会使用此文件中的路由器。",
  "routing.default_route": "默认",
  "routing.chip": "路由：{route} → {model}",
  "routing.menu_broken": "路由器未连接：{error}",
  "routing.wizard_title": "快速设置",
  "routing.wizard_lede": "选择三个模型，编辑器就会填入现成的配置：强模型处理复杂请求，其余交给默认模型。保存之前不会写入任何内容。",
  "routing.wizard_decider": "回答问题的模型",
  "routing.wizard_default": "默认模型",
  "routing.wizard_strong": "处理复杂请求的强模型",
  "routing.wizard_build": "填入编辑器",
  "routing.wizard_none": "还没有模型：请先连接提供商。",
  "routing.wizard_pick": "选择模型",
  "routing.decision_hint":
    "决策模型最好使用上方的 Decision 提供商：填写其地址（本地或云端），并查询模型。",
} satisfies Record<keyof typeof source, string>;
