# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

Android 터미널의 렌더 후 자동 맞춤을 제거하고 원격 행·열을 유지합니다. 전체 컨테이너 너비로
원격 크기 변경을 요청하며 xterm·바깥 컨테이너·WebView의 스크롤바를 숨깁니다.
세로 터치는 행 단위 휠·로컬 이력 스크롤로 전달하고, 비활성 pane의 첫 드래그는 선택만 수행합니다.
대상 변경·입력 차단·재접속·핀치·취소 시 미전송 이동량을 폐기합니다. Rust API·저장 설정·의존성 버전은 유지합니다.
이전 변경의 검사는 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| 웹 | `npm test --prefix web`: 32 통과·skip 0. 원격 열 수·분할 pane·컨테이너 측정, 방향·행 단위 누적·소수 픽셀, TUI 모드·원본 대상, IME 분리·캡처 이동·합성 클릭·취소와 기존 회귀 검사 |
| Android 빌드 | JDK 17·SDK 36의 `assembleDebug`·`assembleDebugAndroidTest`·`lintDebug` 통과. `verify_lint.py`: 기존 검토 진단 12개 일치 |
| API36 전용 에뮬레이터 | WebView 133.0.6943.137에서 터미널 관련 계측 2개·기존 핀치/가로 경계 검사 1개, 총 3개 통과·skip 0. 실제 번들 xterm의 80열·분할 마지막 열·줄바꿈, 스크롤바 숨김, SGR·128 이상 바이너리 좌표·방향키 대체·로컬 이력, 실제 손가락 세로/가로 드래그·IME 미표시, 비활성 pane 선택·대상 변경과 기존 pane 화면·복구 회귀 검사. 명시적 이름 목록 대조 통과 |
| 원본 APK | `collect_rust_android.py`: `androidApp/build/outputs/apk/debug/androidApp-debug.apk`의 ID·버전·SDK·네 ABI·Rust payload·v2 서명 검증 통과. 경로·해시는 `artifacts/rust-candidate/android/build.json` |
| 정적 검사 | Python 검사 이름 목록·결과 검증기 6개, 변경 문서 로컬 링크·diff 공백·대시보드 메타데이터 검증 통과 |

계측 결과는 `artifacts/android-terminal-scroll/`에 있으며 핀치 검사의 화면·IME 설정은 복원했습니다. 검사 작성 중 DOM 커서 공백·WebView의
스크롤바 CSS 지원 차이·주입 손가락 종류·보존된 이력 위치를 반영해 fixture를 수정했습니다.
실제 터치의 캡처가 xterm 셀에서 pane으로 이동할 때 발생하는 자식의 캡처 해제를 취소로 처리하지 않도록 보강했습니다.
Rust·JVM·데스크톱 검사·실제 SSH/원격 TUI 수신·물리 Android·Windows 네이티브·전체 API matrix·장시간 검사는 이번 변경에서 수행하지 않았습니다.

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
