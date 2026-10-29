# 검증 범위와 한계

기본 검증은 기능 검사·유닛 테스트입니다. 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
이 문서는 현재 범위와 알려진 한계만 다룹니다. 변경별 실행 결과는 [HISTORY.md](HISTORY.md)에 요약하고 상세는 Git 이력에서 찾습니다.
실행하지 않았거나 skip·ignore된 검사를 통과로 기록하지 않습니다.

## 기본 검증

| 영역 | 내용 | 예상 제외 |
|---|---|---|
| 원문·도구 | `check_prompt.py`·`verify_wrapper.py`, 번역·UI 아이콘 생성물 `--check`, `scripts/tests` 단위 검사 | 없음 |
| Rust | `cargo test --locked -p dgl-core -p dgl-uniffi`. 공통 계약 fixture 포함. 데스크톱 변경은 macOS에서 `dgl-desktop`까지 | `ssh_integration`(격리 SSH/tmux 필요)과 macOS Keychain 왕복은 기본 ignore |
| JVM | `:app:desktopTest` 후 `verify_test_results.py --mode linux`로 명시적 이름 목록 대조 | 외부 SSH가 필요한 `IntegrationTest`·`RecoveryTest`·`PerformanceTest`만 예상 skip |
| 웹 | `npm test --prefix web`(Android 터미널 assets) | 없음 |
| Android 빌드 | CI 기본 job의 Debug APK·`lintDebug`와 `verify_lint.py` 검토 목록 대조 | 계측 실행은 선택 |

Android Debug APK는 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 원본만 검증하며 복사본을 만들지 않습니다.
`main` push/PR의 **Build and verify**가 위 검사를 실행합니다. GPUI 데스크톱 검사는 macOS 로컬,
Windows는 `build_windows.ps1 -Test`로 별도 실행합니다.

## 기본 범위 밖

| 항목 | 실행 조건 |
|---|---|
| 격리 SSH/tmux 통합 | `scripts/integration.py`로 Rust `ssh_integration` ignore 검사 실행. 에이전트 관측은 `--agent-home` 필수. CI 수동 `verify_ssh`(tmux 3.3a·3.7c) |
| JVM 참조 엔진 SSH 회귀 | `integration.py --command ./gradlew :app:desktopTest --rerun-tasks` 후 `verify_test_results.py --mode ssh` |
| Android 계측 | 전용 에뮬레이터·명시적 `ANDROID_SERIAL`에서 필요한 `Class#method`만 선택. `verify_android_results.py`는 skip을 허용하지 않음. 전체 API 34·35·36은 CI 수동 `verify_android` |
| 배포 검사 | Release·계측 APK, Gradle·Cargo·npm 의존성 감사, 라이선스·산출물 검사. CI 수동 `verify_packaging` |
| 성능·자원 | [PERFORMANCE.md](PERFORMANCE.md)의 표본·종료 조건을 먼저 정한 뒤 실행. 계측 코드·의존성은 계측 APK(androidTest)에만 포함 |
| 실제 CLI 훅 | 설치·로그인·유료 호출이 필요할 수 있어 [AGENT_STATUS.md](AGENT_STATUS.md) 범위를 따름 |

전체 Android API matrix·장시간 반복 계측은 기본 작업에 자동으로 추가하지 않습니다.
CI 성공은 서명·공개 릴리스·실기 인수 완료를 뜻하지 않습니다.

## 남은 한계

- 실기 인수(macOS GUI·IME·Keychain·OS 알림, Windows MSVC 네이티브 빌드·DPAPI·GUI, Android 물리 기기·실제 키보드·네트워크 전환)는
  미수행입니다. 항목은 [TASKS.md](../TASKS.md)의 기능 인수, Windows 절차는 [WINDOWS.md](WINDOWS.md)를 따릅니다.
- Android 성능·자원 계측 하네스(`ResourceScenario`·`ResourceInstrumentation`·`MigrationPresentationProbe`)는
  제품 WebView를 SSHJ 참조 엔진으로 구동합니다. 결과는 Rust 엔진 성능 근거가 아닙니다.
- 정확한 첫 표시 프레임은 `proven 0`, `unresolved 17`, `invalid_trace`로 미해결입니다.
  수집 도구의 `collected`나 일부 계측 성공은 성능 인수 통과가 아닙니다.
- tmux format/capture는 모든 VT 내부 상태·현재 pen 속성을 직렬화하지 않습니다([ADR 002](decisions/002-terminal-restoration.md)).
  로컬 기록·선택·스크롤 위치는 기기 상태이며 원격 tmux 전체 히스토리 복원을 보장하지 않습니다.
- 사용자 정의 tmux prefix·키 바인딩과 상태줄·팝업·copy-mode 화면 재현은 GUI 범위 밖입니다([TMUX_WINDOW_SYNC.md](TMUX_WINDOW_SYNC.md)).
- 실제 에이전트 훅의 서비스별 수신 범위는 [AGENT_STATUS.md](AGENT_STATUS.md)에 있습니다. 전체 로그인·승인·장시간 인수와 AGY·Grok 실제 수신은 남아 있습니다.
- `AgentNotificationTest#channelPermissionAndClickIntentRetainOnlyEventId`는 시작 시 알림 권한이 허용돼 있으면 실패합니다([TASKS.md](../TASKS.md)의 검사 보완).
- 전체 `cargo fmt --all -- --check`는 `crates/dgl-core/src/command.rs`·`crates/dgl-core/tests/password_shell.rs`의
  형식 차이로 통과하지 않습니다. 기본 검사·CI에는 포함되지 않습니다.
- 공개 배포용 서명·공증과 현재 의존성 전체 감사는 미완료입니다. [SECURITY.md](SECURITY.md)를 따릅니다.
