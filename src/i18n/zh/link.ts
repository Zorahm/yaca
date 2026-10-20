/**
 * Chinese strings: connecting to a server.
 */

import type { link as source } from "../ru/link";

export const link = {
  "link.title": "连接到 Yaca 服务器",
  "link.intro": "Yaca 运行在你的服务器上，这个应用只是通向它的窗口。请输入安装脚本给出的地址和令牌。",
  "link.address": "服务器地址",
  "link.address_hint": "例如 yaca.example.com",
  "link.token": "令牌",
  "link.token_hint": "由 yaca-server token add 输出。",
  "link.connect": "连接",
  "link.connecting": "正在连接…",
  "link.local": "在这台电脑上工作",
  "link.err_unreachable": "服务器没有响应。请检查地址和网络。",
  "link.err_unauthorized": "服务器不接受这个令牌。",
  "link.err_bad_address": "这看起来不是服务器地址。",
  "link.err_replaced": "另一台设备连接了服务器并接管了对话。重新连接即可把它们取回这里。",
  "link.err_revoked": "这台设备的令牌已被撤销。",
  "link.lost": "与服务器的连接已断开，正在重新连接…",
  "link.settings_nav": "连接",
  "link.settings_title": "连接",
  "link.settings_this_pc": "工作在这台电脑上进行。",
  "link.settings_server": "工作在服务器 {name} 上进行。",
  "link.settings_to_server": "连接到服务器",
  "link.settings_to_local": "回到这台电脑",
  "link.settings_disconnect": "断开服务器",
  "link.settings_reload_note": "窗口将重新加载。服务器上的对话会继续运行。",
  "link.pick_up": "上一级",
  "link.pick_home": "主目录",
  "link.pick_choose": "选择此文件夹",
  "link.pick_new": "新建文件夹",
  "link.pick_new_name": "文件夹名称",
  "link.pick_create": "创建",
  "link.pick_cancel": "取消",
  "link.pick_empty": "这里没有子文件夹",
  "link.pick_hidden": "隐藏的文件夹",
  "link.pick_broad": "这看起来是主目录或根目录，而不是项目。智能体会处理其中的所有内容，用于回退的快照也无法创建。",
  "link.pick_loading": "正在读取文件夹…",
} satisfies Record<keyof typeof source, string>;
