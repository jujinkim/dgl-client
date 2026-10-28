# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-28)

Android 키보드 대응에 기기 로컬 `androidKeyboardMode` 설정을 추가했습니다. 기본 자동은 짧은 일반 출력만
리사이즈하며 긴 출력·전체 화면 앱·이력 열람·복원 중에는 이동합니다. 이동은 하단 빈 행을 제외한 마지막 콘텐츠와
커서를 기준으로 하고, 명시적인 리사이즈는 전체 화면 앱도 가시 높이에 맞춥니다. IME 종료 후 최종 행 수를 제출하고
닫을 때 전체 크기를 복원합니다. 자체 resize의 크기 세대 갱신으로 자동 선택을 반복하지 않습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust | core·UniFFI 46 통과·환경 ignore 6. 설정 누락·저장 호환과 동기화 제외, 기존 계약 fixture 포함 |
| JVM | `:app:desktopTest`: 96 통과·환경별 예상 skip 9. 추가 설정 검사와 명시적 이름 목록 대조 통과 |
| 웹 | 55 통과. 빈 행·커서·경계 높이·표시 pane 위치·모드 전환·TUI 격자 복원·최종 resize·빠른 반전·오래된 프레임·스크롤바/수동 위치 포함 |
| Python | 99 통과. JVM/Android 명시 목록과 관련 검증기 회귀 포함 |
| Android 빌드 | Debug·계측 APK·`lintDebug` 통과. lint 기존 검토 진단 12개 목록 검증 통과 |
| 정적·산출물 | 번역 목록·로컬 문서 링크·diff 공백·대시보드 메타데이터·원본 Debug APK/4 ABI 검사 통과 |

JVM 결과는 `artifacts/android-keyboard/jvm-tests.json`, 빌드·Python 원문은 `.tools/android-keyboard-*.log`에 있습니다.
첫 JVM/Rust 실행은 새 기본 필드가 빠진 설정 fixture 예상값 때문에 실패했습니다. 기존 입력 fixture는 유지하고
인코딩 예상값에 추가 기본값을 반영한 뒤 최종 실행을 통과했습니다. 계측 소스의 기존 resize 없음 검사는 이동 모드를 명시했습니다.

APK는 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 원본입니다. Android 계측은 컴파일만 했으며
기기·에뮬레이터에서 이번 키보드 동작을 실행하지 않았습니다. 실제 SSH·물리 키보드·macOS GUI/Keychain·
데스크톱 재패키징·Windows 네이티브·전체 API matrix·장시간 반복 검사는 수행하지 않았습니다.

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
