/**
 * Chinese strings: the browser column.
 */

import type { browser as source } from "../ru/browser";

export const browser = {
  "browser.toggle": "浏览器",
  "browser.back": "后退",
  "browser.forward": "前进",
  "browser.reload": "重新加载",
  "browser.stop": "停止加载",
  "browser.address": "地址或 localhost:端口",
  "browser.external": "在系统浏览器中打开",
  "browser.devtools": "开发者工具",
  "browser.empty_title": "还没有打开任何页面",
  "browser.empty_body": "模型或你打开的页面会显示在这里",
  "browser.size_model": "{size} · 由模型设置",
  "browser.reset_size": "恢复为栏宽",
  "browser.resize": "调整浏览器宽度",
  "browser.bad_address": "这看起来不像页面地址",
  "browser.open_failed": "页面没有打开",
  "browser.busy": "模型正在使用浏览器",
} satisfies Record<keyof typeof source, string>;
