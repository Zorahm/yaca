/**
 * Korean strings: the frame of the window — sidebar, top bar, title bar,
 * menus, home screen.
 */

import type { shell as source } from "../ru/shell";

export const shell = {
  "side.new": "새 채팅",
  "side.projects": "프로젝트",
  "side.new_here": "이 프로젝트에서 새 채팅",
  "side.toggle": "사이드바",
  "side.update_do": "업데이트",
  "side.update_more": "자세히",
  "side.forget": "프로젝트 제거",
  "side.forget_busy": "먼저 이 프로젝트의 채팅을 닫으세요",
  "side.no_projects": "아직 프로젝트가 없습니다",
  "side.working": "작업 중",
  "side.waiting": "답변 대기 중",
  "side.close_chat": "채팅 닫기",

  "top.menu": "애플리케이션 메뉴",
  "top.sessions": "세션",
  "top.search": "세션 검색",
  "top.back": "뒤로",
  "top.forward": "앞으로",
  "top.processes": "프로세스",
  "top.checkpoints": "체크포인트",
  "panel.tab_mcp": "MCP",
  "panel.tab_lsp": "LSP",
  "panel.toggle": "오른쪽 패널",
  "panel.resize": "패널 너비",
  "panel.tab_agents": "에이전트",
  "side.resize": "사이드바 너비",
  "top.session_menu": "세션 메뉴",
  "top.untitled": "새 세션",
  "top.rename_hint": "더블클릭하여 이름 변경",

  "home.greeting": "무엇부터 시작할까요?",
  "home.greeting_named": "{name}님, 무엇부터 시작할까요?",
  "home.pick_project": "폴더 선택",
  "home.pick_title": "프로젝트 폴더",
  "home.not_chosen": "선택되지 않음",
  "home.lead":
    "프로젝트 폴더를 고르세요. 그러면 에이전트가 그 안의 파일로 작업을 시작합니다. 모드, 모델, 추론 강도는 첫 메시지 전에 바꿀 수 있습니다.",
  "home.no_provider": "프로바이더가 없습니다.",
  "home.no_provider_hint": "메시지를 쓰려면 모델 키를 추가하세요.",

  "title.minimize": "최소화",
  "title.maximize": "최대화",
  "title.restore": "이전 크기로",
  "title.close": "닫기",
  "menu.help": "도움말",
  "menu.check_updates": "업데이트 확인",
  "menu.about": "Yaca 정보",

  "common.open_project": "프로젝트 열기",
  "common.close": "닫기",
  "common.seconds": "{n}초",
} satisfies Record<keyof typeof source, string>;
