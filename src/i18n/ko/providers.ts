/**
 * Korean strings: providers — the settings section and the panel before it.
 */

import type { providers as source } from "../ru/providers";

export const providers = {
  // Providers: the panel that exists until the settings window.
  "prov.title": "프로바이더",
  "prov.home_button": "프로바이더 설정",
  "prov.add": "프로바이더 추가",
  "prov.edit_row": "설정",
  "prov.delete": "삭제",
  "prov.delete_confirm": "영구적으로 삭제할까요?",
  "prov.models_count.one": "모델 {n}개",
  "prov.models_count.few": "모델 {n}개",
  "prov.models_count.many": "모델 {n}개",

  "prov.form_new": "새 프로바이더",
  "prov.form_name": "이름",
  "prov.form_more": "고급",
  "prov.form_protocol": "프로토콜",
  "prov.form_base_url": "기본 URL",
  "prov.form_base_url_hint":
    "프로토콜 경로는 자동으로 붙습니다: OpenAI는 /v1/chat/completions, Anthropic은 /v1/messages입니다. OpenRouter는 https://openrouter.ai/api입니다. 주소에 이미 버전이 있으면(/v1, /v1beta, /v4) /v1은 붙지 않습니다.",
  "prov.form_headers": "사용자 지정 헤더",
  "prov.form_headers_hint":
    "프로토콜의 기본 헤더에 추가되며, 그것을 대체할 수도 있습니다. authorization도 포함입니다. 값 안의 {key}는 키체인의 키로 바뀝니다. 키 자체는 파일에 저장되지 않습니다. 값을 비우면 프로토콜 헤더가 제거됩니다.",
  "prov.form_header_name": "이름",
  "prov.form_header_value": "값",
  "prov.form_add_header": "헤더 추가",
  "prov.form_remove_header": "헤더 제거",

  "prov.form_key_title": "키",
  "prov.form_key_hint":
    "프로바이더 이름으로 시스템 키체인에 저장됩니다 — providers.json에는 저장되지 않습니다",
  "prov.form_key_new_hint":
    "저장하면 프로바이더에 모델 목록을 바로 요청합니다. 응답한 모델이 모델 선택 목록에 나타납니다",
  "prov.key_save": "키 저장",
  "prov.key_forget": "키 지우기",
  "prov.key_source_keychain": "키가 시스템 키체인에 있음",
  "prov.key_source_environment": "키가 환경 변수에 있음",
  "prov.key_source_none": "키 없음",
  "prov.key_source_unknown": "프로바이더를 저장하면 키의 출처가 표시됩니다",
  "prov.key_not_needed": "키가 필요 없습니다: {key}는 어떤 헤더에도 쓰이지 않습니다",

  "prov.form_models": "모델",
  "prov.form_models_hint":
    "목록은 프로바이더 자체에서 옵니다. 키를 저장하면 바로 요청하며, '조회'를 누르면 다시 요청합니다. 응답한 목록을 그대로 사용합니다. 가격은 100만 토큰당 달러이며, 0은 무료 또는 알 수 없음을 뜻합니다.",
  "prov.form_model_context": "컨텍스트, 토큰",
  "prov.form_model_output": "최대 출력, 토큰",
  "prov.form_model_effort": "추론 강도 지원",
  "prov.models_ask": "조회",
  "prov.models_asking": "조회 중…",
  "prov.models_none": "모델 없음",
  "prov.models_price": "$ / 1M 입력·출력",
  "prov.models_effort": "추론 강도",

  "prov.form_save": "프로바이더 저장",
  "prov.form_cancel": "취소",

  "prov.saved_unasked":
    "“{name}” 저장됨, 하지만 모델 목록에 응답하지 않았습니다: {error}",
  "prov.error_name": "프로바이더 이름을 입력하세요",
  "prov.error_url": "기본 URL을 입력하세요",
  "prov.error_header_name": "헤더 {n}의 이름이 없습니다",
  "prov.error_header_dup": "헤더 {name}이(가) 두 번 지정되었습니다",

  "prov.lede": "대화와 키가 전송되는 곳입니다. 앱 수준의 설정이라 프로젝트에서는 정하지 않습니다.",
  "prov.answered": "모델 목록으로 응답함",
  "prov.unasked": "아직 조회하지 않음",
  "prov.unasked_note":
    "빈 원은 프로바이더가 모델 목록으로 응답하지 않았다는 뜻입니다. 모델 선택 목록에 나타나지 않습니다. 세션이 요청을 보낼 방법이 없기 때문입니다.",
} satisfies Record<keyof typeof source, string>;
