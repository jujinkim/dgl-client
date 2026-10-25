# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

데스크톱 세션 목록의 복원 중 위치 유지와 상태줄 알림·설정 버튼 이동의 검사입니다.
Android는 제품 UI를 유지하고 기존 계층 fixture의 상태 전환 검사를 보강했습니다.
이전 앱 아이콘·강조 색상·동기화 검사 요약은 [HISTORY.md](HISTORY.md)에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| 세션 목록 GPUI fixture | `Ready → Restoring → Ready`와 재접속·연결·인증 전환에서 행 좌표·펼침·비영점 스크롤·포커스 유지. 전체/간략 계층과 빈 목록 대기 안내 검사 |
| 상태줄 GPUI fixture | 폭 420/640/1200px, 밝게/어둡게, 단일/분할 그룹, 긴 상태 문구와 미확인 0/1/12/100개 전환에서 26px 높이·오른쪽 버튼 위치·배지 영역 유지 |
| 대화상자 GPUI fixture | 알림·설정 클릭, Esc·닫기 후 터미널 포커스 복원, 모달 중 키·텍스트 입력 차단, 기존 설정 단축키 검사 |
| Rust | `cargo test --locked --workspace`: 데스크톱 54 통과/Keychain 1 ignore, 코어·UniFFI 27 통과/SSH 통합 5 ignore. ignore는 미실행이며 통과에 포함하지 않음 |
| macOS 빌드 | `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer cargo build --locked -p dgl-desktop` 성공. 실제 GUI·번들 패키징은 이번에 실행하지 않음 |
| Android | `:androidApp:assembleDebugAndroidTest` 성공. 기존 `compactRowsExpansionPanelDetailsAndSelectedHostActions` 이름을 유지하고 복원 전후 펼친 행·터미널 영역 좌표 검사 추가. 계측 실행은 연결 기기가 없어 미검증 |
| JVM 참조 검사 | `:app:desktopTest`는 UP-TO-DATE. 기존 결과를 `verify_test_results.py --mode linux`로 검증: 46 통과/예상 skip 9. 신규 실행으로 계산하지 않음 |
| Python | 테스트 이름 명시 목록 검사와 Windows 패키징을 포함한 98개 단위 검사 통과 |
| 문서·메타데이터 | 작업 공간 사용 문서와 로컬 대시보드 문서 등록 갱신, 대시보드 검증기 통과·Git 제외 유지 |
| 무결성 | 변경 Rust 파일 서식·문서 로컬 링크·`git diff --check`·원문 요구사항 SHA-256 검사 통과 |

첫 GPUI 검사에서 버튼 클릭이 이전 터미널 포커스를 루트로 옮기는 문제를 발견해
상태줄 버튼 영역의 기본 마우스 포커스 이동을 막고 재검사했습니다.
GPUI가 제거된 요소의 디버그 좌표를 보관하므로 안내·배지 제거는 현재 컨테이너 높이와 아이콘 위치로 검사합니다.

Windows MSVC 네이티브 빌드·DPAPI·GUI, macOS 실제 GUI·Keychain,
Android 계측·전체 API별 검사·SSH 통합은 이번에 실행하지 않았습니다.
Windows 기본 빌드·ZIP·수동 CI 경로는 유지합니다.

Gradle 10 대비 deprecated 기능, 기존 Android 테스트의 `isChecked` deprecated 사용과
Rust 의존성 `block`/`proc-macro-error2`의 향후 호환성 경고는 남아 있습니다.
실행 로그는 Git 제외 로컬 `/tmp/dgl-session-*.log`입니다.

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
