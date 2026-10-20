/**
 * Korean strings: connecting to a server.
 */

import type { link as source } from "../ru/link";

export const link = {
  "link.title": "Yaca 서버에 연결",
  "link.intro": "Yaca는 사용자의 서버에서 실행되며, 이 앱은 그 서버를 보는 창입니다. 설치 스크립트가 알려 준 주소와 토큰을 입력하세요.",
  "link.address": "서버 주소",
  "link.address_hint": "예: yaca.example.com",
  "link.token": "토큰",
  "link.token_hint": "yaca-server token add 명령이 출력합니다.",
  "link.connect": "연결",
  "link.connecting": "연결하는 중…",
  "link.local": "이 컴퓨터에서 작업",
  "link.err_unreachable": "서버가 응답하지 않습니다. 주소와 네트워크를 확인하세요.",
  "link.err_unauthorized": "서버가 토큰을 받아들이지 않았습니다.",
  "link.err_bad_address": "서버 주소처럼 보이지 않습니다.",
  "link.err_replaced": "다른 기기가 서버에 연결해 채팅을 가져갔습니다. 다시 연결하면 여기로 가져올 수 있습니다.",
  "link.err_revoked": "이 기기의 토큰이 폐기되었습니다.",
  "link.lost": "서버와의 연결이 끊어졌습니다. 다시 연결하는 중…",
  "link.settings_nav": "연결",
  "link.settings_title": "연결",
  "link.settings_this_pc": "작업이 이 컴퓨터에서 이루어집니다.",
  "link.settings_server": "작업이 서버 {name}에서 이루어집니다.",
  "link.settings_to_server": "서버에 연결",
  "link.settings_to_local": "이 컴퓨터로 돌아가기",
  "link.settings_disconnect": "서버 연결 끊기",
  "link.settings_reload_note": "창이 다시 로드됩니다. 서버의 채팅은 계속 실행됩니다.",
  "link.pick_up": "위로",
  "link.pick_home": "홈",
  "link.pick_choose": "이 폴더 선택",
  "link.pick_new": "새 폴더",
  "link.pick_new_name": "폴더 이름",
  "link.pick_create": "만들기",
  "link.pick_cancel": "취소",
  "link.pick_empty": "하위 폴더가 없습니다",
  "link.pick_hidden": "숨김 폴더",
  "link.pick_broad": "프로젝트가 아니라 홈 또는 루트 폴더로 보입니다. 에이전트가 그 안의 모든 것을 다루게 되며, 되돌리기용 스냅숏도 만들 수 없습니다.",
  "link.pick_loading": "폴더를 읽는 중…",
} satisfies Record<keyof typeof source, string>;
