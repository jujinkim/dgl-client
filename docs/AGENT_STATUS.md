# Codex·Claude 관측과 알림

2026-10-25 구현. Mac·Windows·Android는 같은 Rust 관측 모델과 알림 큐를 사용한다.
SSH로 연결한 tmux 세션의 모든 창·패널을 조회한다. 일반 SSH 셸에는 세부 관측을 제공하지 않는다.

## 사용 방법

1. 호스트에 연결한 뒤 호스트 메뉴의 **에이전트 상태 연동** 또는 탭/연결의 상태 배지를 연다.
2. **설치 전 검사 · 변경 내용 보기**에서 Python, tmux, CLI 버전, 수정할 파일과 추가 훅을 확인한다.
3. **연동 설치**를 누른다. 현재 설치 검사를 통과하는 버전은 Codex **0.163.0**, Claude Code **2.1.310**이다. 다른 버전은 미검증으로 표시하고 설치에서 제외한다.
4. Codex에서는 `/hooks`에서 추가된 도우미 명령을 직접 검토·신뢰한다. 앱은 훅 신뢰를 우회하거나 CLI를 재시작하지 않는다.
5. **설치됨 / 활성화 확인 필요**와 **수신 확인 사건**을 구분한다. 후자는 해당 실행에서 실제 받은 사건 종류만 표시한다. 새 실행 또는 다음 실제 훅 수신 전에는 기존 실행의 상태를 복원하지 않는다.
6. 앱 설정에서 종류별 알림과 **OS 알림**을 설정한다. 종류별 알림은 기본 켜짐, OS 알림은 기본 꺼짐이다.

**dgl 훅만 해제**는 기존 사용자 훅·설정을 보존한다. 도우미·관측 DB와 설정 백업은 자동 삭제하지 않는다.
해제 시 최신 관측을 비우고 DB 세대를 바꿔, 이전 사건을 현재 연동 상태나 새 알림으로 되살리지 않는다.
설정 변경 충돌 또는 비정상 설정은 덮어쓰지 않고 재검사를 요구한다. 여러 설정 파일을 바꾸는 도중
실패했다면 일부 파일이 적용되었을 수 있으므로 다시 검사한다. 파일별 백업과 원자적 교체를 사용하며,
다른 프로그램이 같은 파일을 교체 직전에 쓰는 극소한 경쟁 조건까지 POSIX 파일 API로 완전히 잠그지는 못한다.

## 표시가 뜻하는 것

| 표시 | 의미 |
|---|---|
| 서비스 아이콘 | 패널 프로세스의 실행 파일 또는 확인된 Node CLI 진입점으로 식별 |
| ↻ 활동 | 요청·도구 등 작업 사건이 관측됨 |
| ? 질문 / ◇ 승인 | 질문·승인 요청이 발생했음. 현재도 대기 중이라는 뜻은 아님 |
| ○ 응답 확인 | Stop 신호를 받았음. 작업 성공·검토 통과·영구 입력 대기를 뜻하지 않음 |
| ! 오류 | 공식 오류 훅을 받았음. 종료 코드나 화면 문구로 추정하지 않음 |
| 중립 / 상태 미확인 | 미연동, Python/조회 문제, 단절, 누락 또는 오래된 관측 |

탭의 대표 사건은 질문·승인 → 오류 → 응답 확인 → 활동 순이다. 읽지 않은 사건과 관측은 별개다.
다른 작업이 진행되어도 읽지 않은 질문은 알림에 남는다. 읽음 처리나 알림 클릭은 원격 입력·승인을 전송하지 않는다.
서브에이전트 사건은 별도 실행 주체로 표시하며 `SubagentStop`을 부모의 Stop으로 올리지 않는다.
배지 상세와 알림에서 세션·창·패널로 이동할 수 있다. 종료·재생성된 대상은 이동을 거부하고 재접속하지 않는다.

## 수집·저장

- 기본 식별은 읽기 전용이다. `node`라는 이름, 화면·스피너·CPU·출력 중단으로 식별하거나 상태를 추정하지 않는다.
- Python이 없으면 POSIX `ps`와 tmux의 보수적인 실행 파일 식별만 제공한다. 연동 설치에는 Python **3.9 이상**이 필요하다.
- 선택 설치 위치: `~/.local/share/dgl-agent/v1/observer.py`, 관측 DB: 같은 폴더의 `events.sqlite`.
- 사용자 설정: `$CODEX_HOME/hooks.json`(기본 `~/.codex/hooks.json`), `$CLAUDE_CONFIG_DIR/settings.json`(기본 `~/.claude/settings.json`). 기존 inline Codex 설정을 덮어쓰지 않는다.
  Codex의 같은 계층에 inline `[hooks]`가 있으면 CLI가 두 소스를 병합하고 시작 시 경고할 수 있다.
- 설치 잠금, 설정 내용 토큰 재검사, 같은 폴더 임시 파일 교체, 권한 0600의 `.dgl-backup-*` 백업을 사용한다. 기존 훅과 함께 들어 있는 사용자 handler도 해제 시 보존한다.
- SQLite는 최대 2,048개 사건과 실행 주체별 최신 관측을 보관한다. 프롬프트·응답·도구 입력·명령·대화 파일은 기록하거나 읽지 않는다. 프로세스 명령줄은 CLI 진입점 식별 중 메모리에서만 사용한다.
- 서버 소켓·서버 프로세스 시작 정보·패널·CLI PID/시작 정보로 실행을 구분한다. macOS `ps`의 경로 열 잘림을 피한다.
- 기존 SSH의 독립 exec 채널에서 약 2초마다 순차 조회한다. 출력 구독이 없어도 수집하고 조회를 겹치지 않는다. 10초 이상 오래된 관측, 단절, 사건 간격 누락은 현재 관측으로 표시하지 않는다. 누락 이전의 관측은 새 훅이 올 때까지 미확인으로 유지한다.
- 최초 연결·재접속·DB 세대 변경은 기준선만 동기화한다. 과거 사건을 새 알림으로 재발송하지 않는다.

## 알림

Rust `Inbox` 하나가 앱의 모든 연결을 집계한다. 같은 사용자/호스트·서버 사건의 여러 탭 수신을 중복 제거한다.
앱 내 최근 사건은 실행 중 최대 100건이다. OS 전송은 같은 패널에서 2초 내 수신한 알림을 최신 사건으로 묶는다.
이미 화면에서 읽은 사건은 이후 탭을 바꾸어도 OS에 뒤늦게 알리지 않는다.
앱이 전면이고 대상 패널이 보이면 OS 요청을 생략한다. 데스크톱 분할 그룹의 보이는 패널도 포함한다.

- macOS: UserNotifications, 번들 ID `kim.jujin.dgl.rust-candidate`. 패키지 `.app`에서 사용한다.
- Windows: 자체 AUMID·사용자 Start Menu 바로가기·COM 알림 활성화. ZIP 배포를 유지한다. [등록·실기 절차](WINDOWS.md#6-에이전트-알림).
- Android: `agent-observations-v1` 알림 채널과 `POST_NOTIFICATIONS`. 알림 클릭 Intent에는 사건 ID만 전달한다. 포그라운드 서비스·푸시는 추가하지 않는다.

OS 권한 거부·집중 모드·프로세스 종료 때문에 표시가 보장되지는 않는다. 앱 내 사건은 유지된다.
Android는 앱 프로세스와 SSH 연결이 유지되는 동안만 수신한다.

## 확인한 범위와 제한

| 검사 | 결과 |
|---|---|
| Python 도우미 5개 검사 | 기존 훅 보존, 반복 설치/해제, 외부 변경, 백업, 지원하지 않는 버전, symlink 거부, Node 오인 방지, 본문 미저장, 실행 재시작, 제한 기록/누락 통과 |
| Rust·UniFFI | 관측/읽음 분리, 중복, 묶음, 가시성, 종료 대상, 설정 fixture 회귀 통과 |
| GPUI | 최근 알림 이동 실패 시 읽음 처리·재접속/입력 미전송, 기존 UI 회귀 통과 |
| 격리 SSH/tmux | 숨겨진 창, 출력 미구독 수집, 최초 DB 동기화, 새 사건, 패널 이동, 재연결 재알림 방지, 프로세스 종료 확인. 이 검사의 생산자는 C로 만든 합성 CLI이며 실제 CLI 검사는 아래와 별개 |
| 실제 Codex 0.163.0 | 임시 HOME·독립 tmux에서 정상 신뢰 UI로 훅을 검토하고 SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, SubagentStart/Stop 수신 확인 |
| 실제 Claude Code 2.1.310 | 같은 격리 방식으로 SessionStart, UserPromptSubmit, 도구 전후, AskUserQuestion, PermissionRequest, PostToolUseFailure, Stop, SubagentStart/Stop, SessionEnd 수신 확인 |
| Android API 36 | 네 ABI Debug APK, JVM 회귀, lint, 알림 계측 2개 통과: 권한 거부/허용·채널·중복, 실제 PendingIntent 클릭의 종료 대상 처리, 네이티브 공통 Inbox·읽음/관측 분리 |
| macOS | GPUI 빌드·ad-hoc 패키징·단위 UI 검사 통과. 테스트 알림 요청은 발생했으나 CUA가 `cgWindowNotFound`로 창을 찾지 못해 OS 배너·클릭 실기 확인은 미완료 |
| Windows | 토스트 어댑터의 Windows GNU 대상 독립 타입 검사와 ZIP 패키저 단위 검사 통과. Windows MSVC 전체 빌드·실행·COM/토스트·DPAPI 실기는 사용자 담당이며 미수행 |

Codex PermissionRequest 실검증은 현재 실행 정책의 자동 승인 검토가 테스트용 sandbox 승인 요청을 거부하여
완료하지 못했다. Codex 질문 도구와 일반 오류는 수집 지원으로 표시하지 않는다.
Claude StopFailure와 Codex Interrupt/SessionEnd 등 모든 설정 훅의 실제 발생을 재현한 것은 아니다.
설치 가능한 훅 목록과 실제 **수신 확인 사건**을 혼동하지 않는다. 지원하지 않는 사건은 터미널이나 대화 로그로 보충하지 않는다.
실제 CLI 로그인 전 과정, 장시간 실행·네트워크 전환·macOS 네이티브 알림 클릭은 이 결과에 포함되지 않는다.

## 재현

```sh
cargo test --locked -p dgl-core -p dgl-uniffi -p dgl-desktop
python3 -m unittest scripts.tests.test_agent_observer scripts.tests.test_windows_package -v
python3 scripts/integration.py --agent-home --tmux .tools/tmux-3.3a/tmux-3.3a/tmux --command sh -c 'cargo test --locked -p dgl-core --test ssh_integration -- --ignored --test-threads=1'
python3 scripts/integration.py --agent-home --tmux .tools/tmux-3.7c/tmux-3.7c/tmux --command sh -c 'cargo test --locked -p dgl-core --test ssh_integration -- --ignored --test-threads=1'
./gradlew :app:desktopTest :androidApp:assembleDebug :androidApp:lintDebug
./gradlew :androidApp:connectedDebugAndroidTest -Pandroid.testInstrumentationRunnerArguments.class=kim.jujin.rovessh.AgentNotificationTest
python3 scripts/verify_agent_cli.py --agent Claude --interactive --report artifacts/agent-status/claude-cli.json
python3 scripts/verify_agent_cli.py --agent Codex --interactive --report artifacts/agent-status/codex-cli.json
```

실제 CLI 도구는 임시 HOME에 인증 파일을 0600으로 복사하고 종료 시 지운다. 사용자 설정·기존 CLI를 변경하지 않는다.
대화 본문 대신 사건 이름·종류만 보고서에 남긴다. Codex 신뢰 검토를 건너뛰는 옵션은 없다.
검증 실행 환경의 Java는 Android Studio 내장 JBR 25였으며 Kotlin/JVM 출력 대상 17은 유지했다.

Debug 데스크톱 패키지에서 `DGL_CANDIDATE_DIR`를 임시 디렉터리로, `DGL_NOTIFICATION_PROBE=1`로 실행하면
권한 요청 후 20초 뒤 합성 알림 1건을 보낸다. 클릭 시 대상 종료 안내가 나오고 SSH에 연결하지 않아야 한다.
이 probe는 Release 바이너리에 포함되지 않는다. OS 표시·클릭 확인이 끝나면 테스트 앱을 종료한다.

공식 근거: [Codex Hooks](https://learn.chatgpt.com/docs/hooks),
[Claude Hooks](https://code.claude.com/docs/en/hooks),
[Android 알림 권한](https://developer.android.com/develop/ui/compose/notifications/notification-permission),
[Apple 알림 권한](https://developer.apple.com/documentation/usernotifications/asking-permission-to-use-notifications),
[Windows 데스크톱 알림 활성화](https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-desktop-cpp-wrl).
