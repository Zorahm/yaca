/**
 * Simplified Chinese strings: MCP servers — the settings section and the panel.
 */

import type { mcp as source } from "../ru/mcp";

export const mcp = {
  "mcp.title": "MCP 服务器",
  "mcp.lede_app":
    "你自己的服务器，在每个项目中都会连接。智能体会获得它们的工具，与内置工具并列。",
  "mcp.lede_project":
    "此项目的服务器。保存在 .yaca/settings.json 中，同名服务器会覆盖通用列表和 .mcp.json 中的服务器。",
  "mcp.add": "添加服务器",
  "mcp.none_declared": "没有服务器。",
  "mcp.edit": "编辑",
  "mcp.enabled": "连接 {name}",
  "mcp.delete": "删除",
  "mcp.delete_confirm": "删除？",
  "mcp.file_title": "来自 .mcp.json",
  "mcp.file_hint":
    "项目文件，其他工具也会读取。此处仅作显示。其中的服务器需在聊天中批准后才会启动：/mcp。",
  "mcp.file_replaced": "已被上方同名服务器覆盖",
  "mcp.next_chat_note":
    "列表在聊天打开时读取。更改会在下一个聊天中生效；已打开的聊天仍使用启动时的那些服务器。",
  "mcp.form_new": "新建服务器",
  "mcp.form_save": "保存服务器",
  "mcp.form_name": "名称",
  "mcp.form_name_hint":
    "会成为该服务器每个工具名称的一部分。允许拉丁字母、数字、连字符和下划线。",
  "mcp.form_kind": "连接方式",
  "mcp.kind_stdio": "程序",
  "mcp.kind_http": "地址",
  "mcp.form_command": "程序",
  "mcp.form_command_hint": "直接启动，不经过 shell。",
  "mcp.form_args": "参数",
  "mcp.form_args_hint": "每行一个。",
  "mcp.form_env": "环境变量",
  "mcp.form_env_hint":
    "NAME=value，每行一个。值可以引用环境变量，如示例所示：这样令牌就不会写入文件。",
  "mcp.form_cwd": "工作目录",
  "mcp.form_cwd_hint": "留空则为项目根目录。",
  "mcp.form_url": "地址",
  "mcp.form_url_hint":
    "支持 Streamable HTTP 的服务器。不支持通过 OAuth 登录：令牌通过请求头传递。",
  "mcp.form_headers": "请求头",
  "mcp.form_headers_hint":
    "名称：值，每行一个。值可以引用环境变量，如示例所示。",
  "mcp.problem_name_empty": "请填写名称。",
  "mcp.problem_name_chars": "名称只能包含拉丁字母、数字、连字符和下划线。",
  "mcp.problem_name_taken": "已存在同名服务器。",
  "mcp.problem_command_empty": "请填写程序。",
  "mcp.problem_env": "环境变量的每一行都应为 NAME=value。",
  "mcp.problem_url": "地址必须以 http:// 或 https:// 开头。",
  "mcp.problem_headers": "请求头的每一行都应为“名称：值”的形式。",

  "mcp.panel_empty":
    "此聊天没有 MCP 服务器。在设置中声明后，它们会出现在下一个聊天中。",
  "mcp.status_connecting": "连接中",
  "mcp.status_ready": "就绪",
  "mcp.status_failed": "失败",
  "mcp.status_disabled": "已关闭",
  "mcp.status_needs_approval": "等待批准",
  "mcp.source_global": "通用列表",
  "mcp.source_project_file": "项目的 .mcp.json",
  "mcp.source_project": "项目设置",
  "mcp.source_plugin": "插件",
  "mcp.source_app": "应用",
  "mcp.approve": "批准",
  "mcp.reconnect": "重新连接",
  "mcp.status_needs_login": "需要登录",
  "mcp.status_logging_in": "正在登录",
  "mcp.login": "登录",
  "mcp.logout": "退出登录",
  "mcp.login_note": "请在浏览器中完成登录，完成后服务器将自动连接。",
  "mcp.login_open": "打开登录页面",
  "lsp.panel_empty": "此聊天中没有语言服务器。内置服务器在其程序位于 PATH 中时启动；其他服务器在设置中添加。",
  "lsp.status_idle": "未启动",
  "lsp.status_starting": "启动中",
  "lsp.status_ready": "就绪",
  "lsp.status_failed": "失败",
  "lsp.status_disabled": "已关闭",
  "lsp.restart": "重启",
  "lsp.start": "启动",
  "lsp.extensions": "文件：{list}",
  "mcp.approval_note":
    "在项目的 .mcp.json 中声明，但尚未启动。批准后将启动上方的程序，并记住该批准，直至声明发生变化。",
  "mcp.tools.one": "{n} 个工具",
  "mcp.tools.few": "{n} 个工具",
  "mcp.tools.many": "{n} 个工具",
  "mcp.resources": "资源",
  "mcp.tool_reads": "读取",
  "mcp.tool_writes": "写入",
} satisfies Record<keyof typeof source, string>;
