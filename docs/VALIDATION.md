# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-28)

Android 하단 `…` CLI 메뉴를 별도 Popup 창에서 같은 Activity의 오버레이로 옮겼습니다. 메뉴 열기·명령 선택·
바깥 터치·Back·Escape 동안 WebView 입력 포커스와 IME 표시 상태를 보존합니다. Back은 메뉴가 열린 동안
IME보다 먼저 처리하고, 닫을 때 키/Back 콜백을 해제합니다. 입력 차단·명령 1회 전송·대상 세대 검사와
복구 시 메뉴 종료를 유지합니다. `단축키 설정` 선택 뒤에는 기존 설정 화면의 포커스 동작을 따릅니다.

| 검사 | 이번 결과·범위 |
|---|---|
| CLI 단위 검사 | `:app:desktopTest --tests kim.jujin.dglc.CliLauncherTest`: 5 통과·skip 0. 명시적 이름 목록 대조 통과 |
| Android 빌드 | Debug·계측 APK·`lintDebug` 통과. lint 기존 검토 진단 12개 목록 검증 통과 |
| Android 계측 | 전용 API36에서 CLI 메뉴 관련 2개 통과·skip 0. 결과 검증기의 명시적 이름 목록 대조 통과 |
| IME·포커스 | 키보드 표시/숨김 각각 열기·선택·닫기의 매 프레임과 포커스 전환 감시. 일시적인 IME 숨김/재표시·WebView/창 포커스 상실 없음 |
| 메뉴 회귀 | 같은 Activity 창·버튼 위 위치·화면 경계·한 줄/두 줄 전환·크기 복원·가로 내부 스크롤·키/볼륨/터치 입력 차단·복구/대상 변경·설정 진입 확인 |
| 정적 | 변경 문서의 로컬 링크·diff 공백·대시보드 메타데이터 검증 통과 |

결과·원문·프레임 감시 로그는 `artifacts/cli-menu-overlay/`, 빌드는 `.tools/cli-overlay-*-build.log`에 있습니다.
최초 IME 계측은 메뉴를 열기 전 WebView 초기 포커스 대기에서 실패하여 검사 준비 단계에 포커스를 명시했습니다.
이어진 실행은 기기 로그상 통과했으나 종료 처리 중 실행 래퍼의 120초 제한에 걸려 최종 통과 수에 포함하지 않았습니다.
최종 두 검사 실행은 72.423초로 완료했고 결과 검증기를 통과했습니다.

APK는 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 원본입니다. 삼성 키보드 실기 검사는 수행하지 않았습니다.
실제 SSH·물리 키보드·macOS GUI/Keychain·데스크톱 재패키징·Windows 네이티브·전체 API matrix·장시간 반복과
관련 범위 밖 Rust·웹·JVM 전체 검사는 실행하지 않았습니다. 앞선 키보드 대응 모드 검사 기록은 [HISTORY.md](HISTORY.md)에 있습니다.

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
