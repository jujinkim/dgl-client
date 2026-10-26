# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

기존 tmux 메타데이터 구독을 GUI 탭·Android 연결 선택 이름에 연결했습니다.
세션·활성 창·활성 pane 제목 변경과 전환을 반영하며 빈 값·중복 이름·제어 문자를 정리합니다.
데스크톱에는 긴 이름의 말줄임·툴팁을 추가하고, 재접속·세션 소실 중 마지막 이름 보존을 검사했습니다.
이전 변경의 검증 이력은 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust 데스크톱 | 64 통과, 대화형 Keychain 1개는 기본 ignore. 제목·창/pane 전환·다른 세션 분리·빈 값/중복/Unicode 정리, 제목 변경·세션 소실 시 탭/터미널/연결 동일성과 기존 GPUI 회귀 확인 |
| JVM·결과 검증기 | 77 통과, 외부 SSH/Windows 환경 9개는 명시적 예상 skip. Rust와 같은 제목·전환·분리·정리 사례와 기존 회귀 확인. 명시적 검사 이름 목록 갱신 |
| 격리 SSH/tmux 3.7c | `hierarchy_titles_all_sessions_without_screen_reset` 1개 통과. 실제 OSC 제목·tmux 이름·활성 창/pane 변경을 메타데이터 구독으로 수신하고 화면 재캡처·입력 대상 변경 없이 처리하는 기존 코어 경로 확인 |
| Android Debug APK | `:androidApp:assembleDebug` 통과. 원본 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 갱신. `collect_rust_android.py`로 앱 ID·버전·SDK·Rust 네 ABI·서명 검증. lint·계측은 수행하지 않음 |
| Python·기본 검사 | 스크립트 98 통과. JVM 이름 목록·예상 skip 결과 검증과 diff 공백·대시보드 등록 검증 통과. 문서는 기존 등록 `docs/` 경로에 포함되어 등록 정보 변경 없음 |

UI 동작 결과는 GPUI 이벤트 주입 검사입니다.
Android 실제 GUI/IME·계측은 수행하지 않았습니다.
삼성 키보드를 열자마자 한글을 입력할 때의 조합 위치·최종 문자열, 영문/숫자 비교와 FAB의 실제 키보드 유지 확인은 남아 있습니다.
Windows 네이티브 빌드·DPAPI·GUI, 실제 사용자 서버/TUI, 전체 Android API 계측·장시간 반복은 이번 범위에 포함하지 않았습니다.

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
