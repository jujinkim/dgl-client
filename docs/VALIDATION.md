# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-25)

하단 입력 도구 배치와 데스크톱 텍스트 작성창, Android 명칭·버튼 상태 스타일 변경의 검사입니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust 데스크톱 | 48 통과, Keychain 환경 필요 1 ignore. 작은 창·분할의 입력 바, 고정 버튼·단축키 스크롤, 여러 줄 편집·IME 이벤트·선택·clipboard·스크롤, 포커스·초안 복원, 전송 결과·중복 차단·대상 변경·크기 제한 |
| Rust 코어·UniFFI | 23 통과, 격리 SSH/tmux 환경 필요 5 ignore. 입력·출력 세대와 공통 계약 회귀 포함 |
| macOS 개발 빌드 | `cargo build --locked -p dgl-desktop` 성공. 패키징·서명·실제 GUI 검사는 수행하지 않음 |
| Kotlin/JVM | `:app:desktopTest` 성공. 이름별 검증기에서 45 통과·예상 skip 9 확인 |
| Android Debug·lint | `:androidApp:assembleDebug :androidApp:lintDebug` 성공. lint 오류 0·기존 검토 경고 12, 진단 검증기 통과 |
| 검사 결과 검증기 | Python evidence 검사 6 통과. JVM/Android 테스트 이름 목록과 누락·실패·skip 거절 회귀 |
| 문서·구문 | 변경 Rust 파일 rustfmt·로컬 Markdown 링크·`git diff --check` 통과 |

초안은 연결별 메모리에만 두고 코어·UniFFI 공개 API와 저장 설정 스키마는 바꾸지 않았습니다.
GPUI의 IME 이벤트 주입은 실제 OS 키보드·IME 인수 결과가 아닙니다. 실제 SSH/TUI 전송·Android 계측·Windows
네이티브 빌드·DPAPI·GUI는 이번에 실행하지 않았으며 Windows 공통 빌드·패키징 경로는 유지합니다.
이전 전체 인수 범위와 문서 정리 검사는 [HISTORY.md](HISTORY.md)와 Git 이력에서 확인합니다.

Gradle 10 대비 deprecated 기능과 Rust 의존성 `block`/`proc-macro-error2`의 향후 호환성 경고는 남아 있습니다.
로컬 로그는 Git 제외 `.tools/text-compose-*.log`, JVM 결과는 `artifacts/text-compose-jvm-tests.json`입니다.

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
