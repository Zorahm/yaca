/**
 * Korean strings: the model router — the settings section, the feed note and the
 * switcher's notice.
 */

import type { routing as source } from "../ru/routing";

export const routing = {
  "routing.title": "라우터",
  "routing.lede":
    "작은 모델이 각 메시지에 대한 질문에 답하고, 규칙이 어느 모델이 응답할지 정합니다. routing.json 파일은 providers.json 옆에 있습니다.",
  "routing.state_absent": "설정되지 않았습니다.",
  "routing.state_ready": "연결됨: 모델 목록에 “{name}”(으)로 표시됩니다.",
  "routing.state_broken": "연결되지 않음: {error}",
  "routing.editor": "routing.json 내용",
  "routing.save": "저장",
  "routing.remove": "라우터 제거",
  "routing.saved": "저장했습니다.",
  "routing.next_chat": "이미 열려 있는 채팅은 그대로 동작하고, 새 채팅은 이 파일의 라우터를 사용합니다.",
  "routing.default_route": "기본",
  "routing.chip": "경로: {route} → {model}",
  "routing.menu_broken": "라우터가 연결되지 않았습니다: {error}",
  "routing.wizard_title": "빠른 설정",
  "routing.wizard_lede":
    "모델 세 개를 고르면 편집기에 준비된 설정이 채워집니다. 강한 모델이 어려운 요청을 맡고, 나머지는 기본 모델이 처리합니다. 저장하기 전에는 아무것도 기록되지 않습니다.",
  "routing.wizard_decider": "질문에 답하는 모델",
  "routing.wizard_default": "기본 모델",
  "routing.wizard_strong": "어려운 요청용 강한 모델",
  "routing.wizard_build": "편집기 채우기",
  "routing.wizard_none": "모델이 없습니다. 먼저 공급자를 연결하세요.",
  "routing.wizard_pick": "모델 선택",
  "routing.decision_hint":
    "결정 모델은 위의 “Decision” 공급자에서 고르는 것이 좋습니다. 로컬 또는 클라우드 주소를 지정하고 모델을 조회하세요.",
} satisfies Record<keyof typeof source, string>;
