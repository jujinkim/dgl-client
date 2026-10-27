# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-28)

모바일 하단 두 줄 단축키를 Compose 기본 Staggered Grid로 변경했습니다.
저장 순서대로 더 짧은 줄에 이어 붙이고 길이가 같으면 위 줄을 선택하며, 버튼별 너비와 공동 가로 스크롤을 유지합니다.

| 검사 | 이번 결과·범위 |
|---|---|
| JVM | `:app:desktopTest`: 92 통과·환경별 예상 skip 9, 명시적 이름 목록 대조 통과 |
| Android 빌드 | Debug·계측 APK·`lintDebug` 통과. lint 오류 0·기존 검토 경고 12개, 진단 목록 검증 통과 |
| API36 단축키 | 320dp 전용 에뮬레이터의 `mobileLayoutImeComposerAndSharedPreference` 1개 이름별 완료 통과. 짧은 줄 연속 배치·동률 시 위 줄·독립 너비·공동 스크롤·고정 도구·빈/단일 목록·한 줄 설정 저장/복원·IME·회전·작성창 입력 검사 포함 |
| 정적 검사 | 변경 문서의 로컬 링크와 diff 공백 검사 통과 |

JVM·API36 원문과 이름별 결과는 `artifacts/shortcut-staggered/`, 최종 계측 빌드 로그는
`.tools/shortcut-staggered-test-build.log`에 있습니다. 초기 계측 2회는 화면 밖 Lazy 항목을 기다리거나
새 목록의 프레임 반영 전에 조회하여 실패했습니다. 새 항목 반영과 시작 위치 스크롤을 기다리도록 검사를 보완한 뒤
세 번째 실행이 통과했으며 이전 실패·skip을 통과로 계산하지 않습니다. 에뮬레이터 설정은 복원하고 종료했습니다.
APK는 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 원본을 사용합니다.
Rust 검사·재패키징·실제 SSH·물리 Android·macOS GUI/Keychain·Windows 네이티브·전체 API matrix·장시간 반복 검사는 수행하지 않았습니다.

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
