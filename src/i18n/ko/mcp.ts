/**
 * Korean strings: MCP servers — the settings section and the panel.
 */

import type { mcp as source } from "../ru/mcp";

export const mcp = {
  "mcp.title": "MCP 서버",
  "mcp.lede_app":
    "사용자의 서버입니다. 모든 프로젝트에 연결됩니다. 에이전트는 이 서버의 도구를 기본 도구와 함께 받습니다.",
  "mcp.lede_project":
    "이 프로젝트의 서버입니다. .yaca/settings.json에 저장되며, 전역 목록과 .mcp.json에 있는 같은 이름의 서버를 대체합니다.",
  "mcp.add": "서버 추가",
  "mcp.none_declared": "서버가 없습니다.",
  "mcp.edit": "편집",
  "mcp.enabled": "{name} 연결",
  "mcp.delete": "삭제",
  "mcp.delete_confirm": "삭제할까요?",
  "mcp.file_title": ".mcp.json에서",
  "mcp.file_hint":
    "다른 도구도 읽는 프로젝트 파일입니다. 여기서는 보기만 합니다. 이 파일의 서버는 채팅에서 승인한 뒤에 시작됩니다: /mcp.",
  "mcp.file_replaced": "위의 같은 이름의 서버에 의해 대체됨",
  "mcp.next_chat_note":
    "목록은 채팅이 열릴 때 읽힙니다. 변경 사항은 다음 채팅부터 적용되며, 열린 채팅은 시작할 때의 서버를 계속 사용합니다.",
  "mcp.form_new": "새 서버",
  "mcp.form_save": "서버 저장",
  "mcp.form_name": "이름",
  "mcp.form_name_hint":
    "서버 도구의 모든 이름에 포함됩니다. 영문자, 숫자, 하이픈, 밑줄을 쓸 수 있습니다.",
  "mcp.form_kind": "연결 방식",
  "mcp.kind_stdio": "프로그램",
  "mcp.kind_http": "주소",
  "mcp.form_command": "프로그램",
  "mcp.form_command_hint": "셸을 거치지 않고 바로 실행됩니다.",
  "mcp.form_args": "인수",
  "mcp.form_args_hint": "한 줄에 하나씩.",
  "mcp.form_env": "환경",
  "mcp.form_env_hint":
    "이름=값, 한 줄에 하나씩. 값은 예시처럼 환경 변수를 참조할 수 있습니다. 그러면 토큰이 파일에 들어가지 않습니다.",
  "mcp.form_cwd": "작업 폴더",
  "mcp.form_cwd_hint": "비워 두면 프로젝트 루트입니다.",
  "mcp.form_url": "주소",
  "mcp.form_url_hint":
    "Streamable HTTP를 사용하는 서버입니다. OAuth 로그인은 지원되지 않습니다. 토큰은 헤더로 전달합니다.",
  "mcp.form_headers": "헤더",
  "mcp.form_headers_hint":
    "이름: 값, 한 줄에 하나씩. 값은 예시처럼 환경 변수를 참조할 수 있습니다.",
  "mcp.problem_name_empty": "이름을 입력하세요.",
  "mcp.problem_name_chars": "이름에는 영문자, 숫자, 하이픈, 밑줄만 쓸 수 있습니다.",
  "mcp.problem_name_taken": "같은 이름의 서버가 이미 있습니다.",
  "mcp.problem_command_empty": "프로그램을 입력하세요.",
  "mcp.problem_env": "환경의 각 줄은 이름=값 형식이어야 합니다.",
  "mcp.problem_url": "주소는 http:// 또는 https://로 시작해야 합니다.",
  "mcp.problem_headers": "헤더의 각 줄은 이름: 값 형식이어야 합니다.",

  "mcp.panel_empty":
    "이 채팅에는 MCP 서버가 없습니다. 설정에서 선언하면 다음 채팅에 나타납니다.",
  "mcp.status_connecting": "연결 중",
  "mcp.status_ready": "준비됨",
  "mcp.status_failed": "실패",
  "mcp.status_disabled": "꺼짐",
  "mcp.status_needs_approval": "승인 대기",
  "mcp.source_global": "전역 목록",
  "mcp.source_project_file": "프로젝트의 .mcp.json",
  "mcp.source_project": "프로젝트 설정",
  "mcp.source_plugin": "플러그인",
  "mcp.source_app": "앱",
  "mcp.approve": "승인",
  "mcp.reconnect": "다시 연결",
  "mcp.status_needs_login": "로그인 필요",
  "mcp.status_logging_in": "로그인 중",
  "mcp.login": "로그인",
  "mcp.logout": "로그아웃",
  "mcp.login_note": "브라우저에서 로그인을 마치세요. 마치면 서버에 연결됩니다.",
  "mcp.login_open": "로그인 페이지 열기",
  "lsp.panel_empty": "이 채팅에는 언어 서버가 없습니다. 내장 서버는 프로그램이 PATH에 있으면 시작되고, 다른 서버는 설정에서 추가합니다.",
  "lsp.status_idle": "시작 안 됨",
  "lsp.status_starting": "시작 중",
  "lsp.status_ready": "준비됨",
  "lsp.status_failed": "실패",
  "lsp.status_disabled": "꺼짐",
  "lsp.restart": "다시 시작",
  "lsp.start": "시작",
  "lsp.extensions": "파일: {list}",
  "mcp.approval_note":
    "프로젝트의 .mcp.json에 선언되어 있으며 아직 시작되지 않았습니다. 승인하면 위의 프로그램이 시작되고, 선언이 바뀔 때까지 기억됩니다.",
  "mcp.tools.one": "도구 {n}개",
  "mcp.tools.few": "도구 {n}개",
  "mcp.tools.many": "도구 {n}개",
  "mcp.resources": "리소스",
  "mcp.tool_reads": "읽기",
  "mcp.tool_writes": "쓰기",
} satisfies Record<keyof typeof source, string>;
