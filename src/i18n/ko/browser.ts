/**
 * Korean strings: the browser column.
 */

import type { browser as source } from "../ru/browser";

export const browser = {
  "browser.toggle": "브라우저",
  "browser.back": "뒤로",
  "browser.forward": "앞으로",
  "browser.reload": "새로 고침",
  "browser.stop": "로딩 중지",
  "browser.address": "주소 또는 localhost:포트",
  "browser.external": "시스템 브라우저에서 열기",
  "browser.devtools": "개발자 도구",
  "browser.empty_title": "아직 열린 페이지가 없습니다",
  "browser.empty_body": "모델이나 사용자가 연 페이지가 여기에 표시됩니다",
  "browser.size_model": "{size} · 모델이 지정",
  "browser.reset_size": "열 크기로 되돌리기",
  "browser.resize": "브라우저 너비 조정",
  "browser.bad_address": "페이지 주소가 아닌 것 같습니다",
  "browser.open_failed": "페이지를 열지 못했습니다",
  "browser.busy": "모델이 브라우저를 사용하는 중",
} satisfies Record<keyof typeof source, string>;
