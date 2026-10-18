/**
 * Simplified Chinese strings: the frame of the window — sidebar, top bar, title bar,
 * menus, home screen.
 */

import type { shell as source } from "../ru/shell";

export const shell = {
  "side.new": "新建聊天",
  "side.projects": "项目",
  "side.new_here": "在此项目中新建聊天",
  "side.toggle": "侧边栏",
  "side.update_do": "更新",
  "side.update_more": "详情",
  "side.forget": "移除项目",
  "side.forget_busy": "请先关闭此项目的聊天",
  "side.no_projects": "暂无项目",
  "side.working": "运行中",
  "side.waiting": "等待回复",
  "side.close_chat": "关闭聊天",

  "top.menu": "应用菜单",
  "top.sessions": "会话",
  "top.search": "搜索本会话",
  "top.back": "后退",
  "top.forward": "前进",
  "top.processes": "进程",
  "top.checkpoints": "检查点",
  "panel.tab_mcp": "MCP",
  "panel.tab_lsp": "LSP",
  "panel.toggle": "右侧面板",
  "panel.resize": "面板宽度",
  "panel.tab_agents": "智能体",
  "side.resize": "侧边栏宽度",
  "top.session_menu": "会话菜单",
  "top.untitled": "新会话",
  "top.rename_hint": "双击重命名",

  "home.greeting": "从哪里开始？",
  "home.greeting_named": "从哪里开始，{name}？",
  "home.pick_project": "选择文件夹",
  "home.pick_title": "项目文件夹",
  "home.not_chosen": "未选择",
  "home.lead":
    "选择项目文件夹，智能体便会开始处理其中的文件。模式、模型和强度可以在发送第一条消息之前更改。",
  "home.no_provider": "没有提供商。",
  "home.no_provider_hint": "添加模型密钥后才能发送消息。",

  "title.minimize": "最小化",
  "title.maximize": "最大化",
  "title.restore": "还原",
  "title.close": "关闭",
  "menu.help": "帮助",
  "menu.check_updates": "检查更新",
  "menu.about": "关于 Yaca",

  "common.open_project": "打开项目",
  "common.close": "关闭",
  "common.seconds": "{n} 秒",
} satisfies Record<keyof typeof source, string>;
