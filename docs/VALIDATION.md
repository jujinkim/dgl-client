# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

데스크톱 타이틀바 `DGL` 정리와 데스크톱·Android 한국어/영어 UI, 시스템 언어 선택과 기기별 저장,
세션·창 행의 실시간 터미널 제목 연동 검사입니다.
이전 호스트 순서·세션 탭 재사용 검사는 [HISTORY.md](HISTORY.md)에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| 번역 목록 | 공통 JSON·Kotlin 생성 파일 일치, 호출부 키 누락과 번호 자리표시자 일치 검사 통과 |
| 언어 동작 | 한국어/영어 강제 선택, 시스템 한국어와 지원하지 않는 언어의 영어 fallback, 예전 설정 기본값·재실행 저장·클라우드 제외, 인수 순서 변경·사용자 입력의 중괄호/Unicode 보존 검사 통과 |
| 타이틀 표시 | 세션별 활성 창·pane, 비활성 창·다른 세션, 제목 변경·활성 전환·빈 제목·같은 이름·Unicode/제어 문자·행 ID와 포커스/펼침 유지 검사 통과. Rust/Kotlin 공통 fixture 사용 |
| Rust workspace | 코어·계약·SSH fixture·UniFFI 30 통과, 데스크톱 60 통과. 기본 실행의 SSH/tmux 5 ignore·대화형 Keychain 1 ignore는 통과에 포함하지 않음 |
| 실제 SSH/tmux 제목 | 격리 OpenSSH와 tmux 3.3a·3.7c에서 `hierarchy_titles_all_sessions_without_screen_reset` 각각 1개 통과. 프로그램 OSC 제목·숨은 창/다른 세션·재연결·출력 구독 제거 후 제목 갱신, 입력 대상/프로세스와 화면 상태 보존 확인. 기본 ignore인 다른 통합 검사는 실행하지 않음 |
| JVM | 51 통과·외부 SSH/Windows 환경의 예상 skip 9개, 명시적 결과 검증기 통과 |
| Python | 98개 통과. JVM 검사 이름 목록 일치 포함 |
| Android | 4개 ABI Rust payload와 Kotlin 컴파일·Debug APK·lint 통과. 수집 도구가 앱 ID/버전/SDK·Rust 제품 경로·v2 서명을 확인. lint 기존 검토 진단 12개 일치 |
| Android 검사 코드 | UI 검사의 한국어 설정 고정·실시간 제목 기대값과 언어별 알림 비교 갱신. 계측 Kotlin 컴파일 통과, 계측 실행은 하지 않음 |
| macOS 패키징 | Debug 앱 번들·로컬 ad-hoc 서명과 strict 서명 검사 통과. 실제 GUI 인수·Developer ID·공증을 의미하지 않음 |
| 문서·메타데이터 | 언어 선택·번역 유지 절차 갱신. 기존 로컬 대시보드 등록 검증기 통과·Git 제외 유지 |
| 무결성 | `git diff --check`·원문 요구사항 SHA-256·Gradle wrapper checksum 검사 통과 |

GPUI 검사는 합성 상태와 실제 이벤트 디스패치를 사용합니다. 실제 서버의 창·pane 전환이나 네트워크 복구 인수를 대신하지 않습니다.
Windows MSVC 네이티브 빌드·DPAPI·GUI와 macOS 실제 GUI·Keychain·OS 알림,
전체 SSH/tmux 통합·Android 계측은 이번에 실행하지 않았습니다. Debug APK는 커밋 후에도 다시 빌드·수집합니다.
Windows 기본 빌드·ZIP·수동 CI 경로는 유지합니다.
기존 Rust 의존성 `block`/`proc-macro-error2`의 향후 호환성 경고는 남아 있습니다.

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
