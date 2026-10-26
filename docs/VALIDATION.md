# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

활성 pane 출력의 OSC 0/2를 감지해 tmux 제목만 조회하는 빠른 경로를 추가했습니다.
첫 조회는 즉시, 연속 신호는 연결당 최소 500ms 간격·동시 1건으로 묶고 숨김·백그라운드는 기존 1초 구독을 사용합니다.
표시 신호는 출력 구독과 분리하며 연결·pane·표시 revision이 바뀐 오래된 응답은 버립니다.
기존 탭 제목 순서·20글자 제한·호스트 배지와 모바일 상단 세션명은 유지합니다.
이전 변경의 검증 이력은 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust 코어·UniFFI | `cargo test --locked --workspace` 중 41 통과. 조각난 OSC 0/2·BEL/ST·한글/이모지/빈 제목·다른 OSC/제어 문자열·잘못된 문자열 복구, tmux 인용, 가상 시간 499/500ms·연속 신호 병합·동시 조회 차단·유휴/일반/숨김 출력의 대기 조회 없음, 오래된 응답 거부·제목만 변경 검사 포함. SSH 검사 6개는 기본 ignore |
| Rust 데스크톱 | 같은 workspace 실행에서 65 통과·Keychain 1 ignore. 전면/백그라운드·각 그룹의 선택 연결 표시 판정과 기존 GPUI·탭 제목/입력/화면 회귀 검사 포함 |
| 격리 SSH/tmux 3.7c | `hierarchy_titles_all_sessions_without_screen_reset` 1 통과. fixture 클라이언트의 기존 구독을 제거한 상태에서 빠른 제목 조회·빈 제목·숨김 차단·전면 복귀·OSC 원문 전달·입력 대상/화면 보존을 확인. 구독 복원 뒤 숨긴 창/다른 세션 제목·재접속 갱신도 확인 |
| Kotlin/JVM | `:app:desktopTest` 78 통과·예상 환경 skip 9. 앱 전면/복귀·분할별 표시 집합 전달·출력 surface 없이 표시 상태 변경 검사 포함. `verify_test_results.py --mode linux`의 명시적 목록·skip 검증 통과 |
| Android 빌드·lint | Debug APK·계측 APK 컴파일·`lintDebug` 통과. 생성 UniFFI/Kotlin·네 ABI Rust 엔진을 다시 빌드. lint 오류 0·기존 진단 12를 `verify_lint.py`로 확인 |
| Android APK | 원본 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 갱신. `collect_rust_android.py`의 ID·버전·SDK·Rust 네 ABI·서명 검사 통과. APK 복사본 생성 없음 |
| 스크립트·정적 검사 | Python 98 통과. 명시적 JVM 목록 갱신, 변경 문서의 로컬 링크·diff 공백·원문 요구사항 체크섬·대시보드 스키마 검사 통과. 문서/출력 경로·앱 버전·배포 방식이 같아 등록 정보 변경 없음 |

연결된 Android 기기가 없어 계측은 실행하지 않았습니다. 네이티브 연결 재생성 시 표시 값 재적용은 구현·컴파일로 확인했으며 Android 수명 중 실제 동작 검증은 남아 있습니다.
macOS 실제 GUI·IME·Keychain, Windows 네이티브 빌드·DPAPI·GUI와 전체 Android API 계측·장시간 반복은 수행하지 않았습니다.
삼성 키보드의 한글 조합 위치·최종 문자열과 FAB의 실제 키보드 유지 확인은 남아 있습니다.
전력 실측을 수행하지 않았으며 배터리 절감률을 주장하지 않습니다.

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
