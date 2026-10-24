# 다음 작업 — Mac·Android 기능·기본 디자인 우선

2026-10-24. `main`에서 기존 미커밋 변경을 보존하며 진행한다.
이번 완료 기준은 기존 MVP 필수 기능을 실제로 사용할 수 있는 Mac GPUI 후보와
Android Rust 후보, 그리고 두 후보의 기본 화면 검증이다.

후보 구현·개발용 빌드와 Rust 120 fixture, 두 tmux 버전, API36 검사를 진행했다.
전체 완료로 판정하지 않으며 [후보 기록](docs/migration/CANDIDATE.md)의 미검증 항목을 따른다.

1. 실제 창을 표시할 수 있고 Keychain이 잠금 해제된 Mac 세션에서 GPUI 연결 전 과정,
   IME·clipboard·TUI·탭/resize·저장 후 재기동을 확인한다. 현재 세션의 창 조회는
   `cgWindowNotFound`, Keychain 쓰기는 `-25308`로 끝났다. 툴킷 실패로 해석하지 않는다.
2. 두 후보의 남은 호스트/키/독립 연결, 모든 tmux 관리·삭제 취소, 빈/오류/복구 화면을
   기능 대조표에 따라 인수한다. Android 기본 화면·큰 글자·키보드 편집 검증은 기록했다.
3. 필수 기능과 양쪽 화면 검증이 끝나야 후보 완료다. 성능/실제 에이전트/사용자 데이터 이전은 후속이다.

R1 기능·계약 검증과 성능 계측을 분리한다. 미완료 성능 계측은 R2–R5 기능 구현을
막지 않는다. 정식 성능 수집·최적화, 실제 Codex/Claude 인수, 장시간 soak, 전체 플랫폼
matrix·추가 원격 CI와 데이터 이전은 후속이다.
사용자 후속 지시 “legacy는 불필요”에 따라 Android는 Rust만 빌드·실행하며
`dglEngine` 옵션은 제거했다. 앱 ID·네 ABI·별도 저장 영역은 유지한다.
이전 엔진/SSHJ/BC는 테스트 소스에만 남고 JVM 앱 진입점·설치물 CI는 제거했다.
Mac 빌드는 명령별 `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`를 쓴다.
상태와 실행 증거는 [후보 구현 기록](docs/migration/CANDIDATE.md),
[기능 대조표](docs/migration/FEATURE_MATRIX.md), [전환 계획](docs/ARCHITECTURE_MIGRATION.md)에 기록한다.

## Windows 빌드 환경 유지 — 2026-10-24 후속 지시

Windows는 이번 실기 검증 대상에서 제외할 뿐, 빌드 환경은 제공한다.
Rust GPUI 앱의 OS 의존성·경로·폰트·단축키와 DPAPI 저장소를 분리하고,
`scripts/build_windows.ps1` 및 수동 `Build Windows Rust candidate` workflow로
MSVC x64 Release 실행 파일·미서명 ZIP을 생성하도록 구성했다.
Windows 네이티브 빌드·DPAPI·GUI 검증 결과는 아직 없으며 자동 통과로 처리하지 않는다.
사용자 재현 명령과 검증 항목은 [Windows 안내](docs/WINDOWS.md)를 따른다.

## 이번 첫 표시 프레임 진단 결과

- 수집기와 오프라인 분석기에 `--mode upper-bound|first-presentation`을 추가했다. 기본값은
  기존 상한 측정이다. 실제 문자 변경을 검사하는 테스트 훅과 입력별 증거/공백 보고서를 추가했다.
- API36·WebView 133.0.6943.137·software GPU를 유지해 정상 입력 8회, 100ms 지연 8회,
  무변화 입력 1회를 수집했다. Chromium track_event와 시스템 FrameTimeline을 함께 보존했다.
- 입력→xterm 셀 변경→렌더 ancestor→Chromium main-frame ID까지 확인했다. 그러나 WebView
  수신 프레임 406개의 `surface_frame_trace_id`가 모두 `-1`이며 compositor→Android surface
  token 연결과 앞선 모든 프레임의 내용 증거가 없다. `misplaced_end_event=1` 손실 경고도
  있어 **17개 전부 unresolved, proven 0, invalid_trace**로 남겼다.
- 무변화 입력의 불변 표식은 관찰됐지만 손실 trace에서 `no-content-change`로 확정하지 않았다.
  지연 실험의 key→write 순서는 확인했으며, 이를 표시 지연이나 성능 합격으로 사용하지 않는다.
- 현재 고정 환경에서 정확한 첫 표시를 증명할 수 없다는 것이 이번 단계의 종료 결과다.
  원시 trace·재분석·정리 확인은 [R1 보고서](docs/migration/R1_BASELINE.md), 명령과 결과
  인터페이스는 [성능 문서](docs/migration/PERFORMANCE.md)에 기록한다.

Python 전체 82개(성능·표시 34개), API36 기본 계측 8개, JVM 27개(예상 skip 9개)가
통과했다. 기본 upper-bound도 입력 8개·표시 간격 101개로 재검증했다. WebView 플래그·
전용 AVD·임시 SSH/tmux·reverse를 정리하고 기본 test runner 복구를 확인했다.
