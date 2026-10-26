# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

설정을 카테고리 카드와 별도 단축키 편집 화면으로 구성하고, 하단 Ctrl·Shift·Alt 토글과 공통 Rust 키 인코더를 추가했습니다.
이전 변경의 검증 이력은 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust core·UniFFI | 34 통과. 설정 문자열·기존/빈 목록·기본값, 모든 modifier 합집합·커서 모드·미지원 입력·바인딩 경로 확인. 격리 SSH 통합 6개는 기본 ignore |
| Rust 데스크톱 | 62 통과, 대화형 Keychain 1개는 기본 ignore. 밝은/어두운 테마의 설정 상세 왕복·포커스/스크롤 복원·드래그 확정/취소·제외/재추가/재시작, 토글 중첩·재해제·1회 소비·크기 변경 유지 확인 |
| JVM·결과 검증기 | 74 통과, 외부 SSH/Windows 환경 9개는 명시적 예상 skip. 저장 호환·정렬 모델·조합키 수명·대상 변경·모달/백그라운드 해제, 기존 매크로·볼륨키 계약 확인 |
| 터미널 JavaScript | 21 통과·skip 없음. 공개 키 이벤트의 단일 전송·IME 확정·붙여넣기/마우스/프로토콜 응답 분리·지연된 IME 원래 대상 보존, 기존 렌더/복구 회귀 포함 |
| Android 빌드·lint | `:androidApp:assembleDebug :androidApp:lintDebug` 통과. 네 ABI Rust/UniFFI와 원본 Debug APK 갱신. 기존 lint 진단 12개는 검증기의 명시적 목록과 일치 |
| Python·기본 검사 | 스크립트 98 통과, 번역/자리표시자·Wrapper·원문 체크섬·diff 공백·문서 로컬 링크·대시보드 등록 검증 통과 |
| macOS 패키징 | ARM64 Debug 앱 빌드·로컬 ad-hoc 서명/검증. Developer ID·공증 없음 |

네이티브 GUI 도구가 격리 실행한 macOS 앱을 식별하지 못해 실제 화면 조작을 확인하지 못했습니다.
UI 동작 결과는 GPUI 이벤트 주입 검사입니다. 연결된 Android 기기가 없어 Android 실제 GUI/IME·계측은 수행하지 않았습니다.
Windows 네이티브 빌드·DPAPI·GUI, 실제 SSH/TUI, 전체 Android API 계측·장시간 반복은 이번 범위에 포함하지 않았습니다.

## 남은 한계

- macOS 실제 GUI·네이티브 선택창·IME·Keychain 저장/재기동·OS 알림 인수.
- Windows MSVC 네이티브 빌드·DPAPI·파일 선택·GUI·알림: 사용자 확인 범위.
- Android 물리 기기·실제 한국어/일본어 키보드·Wi-Fi↔셀룰러 전환·장시간 백그라운드 연결.
- 실제 에이전트 훅의 서비스별 범위는 [AGENT_STATUS.md](AGENT_STATUS.md). 전체 로그인·승인·장시간 인수와 실험 서비스 검증은 남음.
- tmux capture/format은 모든 VT 내부 상태·현재 SGR pen을 직렬화하지 않음.
  로컬 기록·선택·스크롤 위치는 기기 상태이며 원격 tmux 전체 히스토리 복원을 보장하지 않음.
- tmux prefix·키 바인딩·상태줄·팝업·copy-mode 화면 재현은 GUI 범위 밖.
- 정확한 첫 표시 프레임: 17개 unresolved, proven 0, invalid_trace.
  성능·자원 수집 도구의 존재와 일부 수집 성공은 전체 성능 인수 완료가 아님.
- 공개 배포용 서명·공증·현재 의존성 전체 감사는 미완료. [SECURITY.md](SECURITY.md) 참고.
