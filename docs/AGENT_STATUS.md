# Claude Code·Codex·AGY·Grok Build 관측과 알림

2026-10-25 구현. Mac·Windows·Android는 같은 Rust 관측 모델과 알림 큐를 사용한다.
SSH로 연결한 tmux 세션의 모든 창·패널을 조회한다. 일반 SSH 셸에는 세부 관측을 제공하지 않는다.

## CLI 실행 메뉴

하단 `…` 메뉴가 현재 원격 터미널에 보내는 명령입니다. 명령 뒤에 Enter를 한 번 전송합니다.
CLI 설치·로그인은 원격 환경에서 준비하며 DGL은 기존 프로세스를 종료하거나 다시 로그인하지 않습니다.

| 서비스 | 일반 실행 | Bypass 실행 |
|---|---|---|
| Claude Code | `claude` | `claude --dangerously-skip-permissions` |
| Codex | `codex` | `codex --yolo` |
| AGY `test` | `agy` | `agy --dangerously-skip-permissions` |
| Grok Build `test` | `grok` | `grok --always-approve` |

입력 불가 상태에서는 실행하지 않습니다. 메뉴를 연 뒤 연결·탭·pane이 바뀌면 닫으며 실패한 입력을 재전송하지 않습니다.
Bypass는 각 CLI의 권한 확인을 건너뛰는 옵션이며 `test`는 실험 지원을 뜻합니다.

## 사용 방법

1. 호스트에 연결한 뒤 호스트 메뉴의 **에이전트 상태 연동**을 연다. 창 행 오른쪽 상태 버튼은 그 창의 모든 pane 관측 상세를 연다.
2. **설치 전 검사 · 변경 내용 보기**에서 Python, tmux, CLI 버전, 수정할 파일과 추가 훅을 확인한다.
3. **연동 설치**를 누른다. 현재 버전 검증 대상은 Codex **0.163.0 / 0.164.0**, Claude Code **2.1.310**이다. 이 두 CLI의 다른 버전은 설치에서 제외한다. AGY·Grok Build는 설치된 CLI에 실험 지원으로 설치를 허용하며 실행 메뉴의 작은 `test` 배지를 유지한다. 미설치·설정 오류는 CLI별로 표시하며 다른 정상 CLI의 설치를 막지 않는다.
4. Codex에서는 `/hooks`에서 추가된 도우미 명령을 직접 검토·신뢰한다. 앱은 훅 신뢰를 우회하거나 CLI를 재시작하지 않는다.
5. **설치됨 / 활성화 확인 필요**와 **수신 확인 사건**을 구분한다. 후자는 해당 실행에서 실제 받은 사건 종류만 표시한다. 새 실행 또는 다음 실제 훅 수신 전에는 기존 실행의 상태를 복원하지 않는다.
6. 앱 설정에서 종류별 알림과 **OS 알림**을 설정한다. 종류별 알림은 기본 켜짐, OS 알림은 기본 꺼짐이다.

**DaGaLi 훅만 해제**는 기존 사용자 훅·설정을 보존한다. 도우미·관측 DB와 설정 백업은 자동 삭제하지 않는다.
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

패널의 대표 사건은 질문·승인 → 오류 → 응답 확인 → 활동 순이다. 읽지 않은 사건과 관측은 별개다.
다른 작업이 진행되어도 읽지 않은 질문은 알림에 남는다. 읽음 처리나 알림 클릭은 원격 입력·승인을 전송하지 않는다.
서브에이전트 사건은 별도 실행 주체로 표시하며 `SubagentStop`을 부모의 Stop으로 올리지 않는다.
같은 CLI 프로세스의 다른 대화도 별도 주체로 구분한다. AGY는 새 실행의 `SessionStart`로 주 대화를 연결하므로 이미 실행 중인 CLI에 설치한 것만으로 상태를 복원하지 않는다.
창 버튼은 합쳐진 세션 행에도 표시한다. 상태 미확인·읽지 않은 사건 수·상세의 실행 목록은 같은 연결과 창으로 필터링한다. 탭·서버 선택 집계 배지는 표시하지 않는다.
배지 상세와 알림에서 세션·창·패널로 이동할 수 있다. 종료·재생성된 대상은 이동을 거부하고 재접속하지 않는다.

## 서비스별 수집 계약

아래는 공식 계약에 맞춰 **수집 가능한 사건**이다. 설치 검사의 `capabilities`와 훅 목록 `events`는 실제 수신 증거가 아니다. 관측의 `supported`는 해당 실행 주체가 실제 받은 종류만 포함한다.

| 서비스 | 활동 | 질문·승인 | 오류 | 응답 종료·중단 |
|---|---|---|---|---|
| Claude Code | 세션 시작·요청·도구 전후 | `AskUserQuestion`, `PermissionRequest` | `PostToolUseFailure`, `StopFailure` | `Stop`은 응답, `SessionEnd`·`SubagentStop`은 종료 |
| Codex | 세션 시작·요청·도구 전후 | `PermissionRequest`만 지원; 질문 미지원 | 일반 오류 미지원 | `Stop`은 응답, `Interrupt`·`SessionEnd`·`SubagentStop`은 종료 |
| AGY (`test`) | 세션 시작·모델 호출 전후·도구 전후 | `ask_question`, 명시적 `ask_permission` | `PostToolUse.error`, `Stop.error` 또는 오류 종료 사유 | `Stop.terminationReason=model_stop`만 응답; 다른 사유는 종료 |
| Grok Build (`test`) | 세션 시작·요청·도구 전후·권한 거절 | `ask_user_question`, `Notification.permission_prompt` | `PostToolUseFailure`, `StopFailure` | 정상 `Stop`만 응답; `StopCancelled`·`SessionEnd`·`SubagentStop`은 종료 |

AGY의 일반 권한 팝업 감지는 지원하지 않는다. Grok의 `idle_prompt`는 오류·중단 뒤에도 발생하므로 무시하고, `task_complete`도 Stop과 중복 알림을 만들지 않는다.
Grok의 `subagentType`·대화 ID와 AGY의 `conversationId`를 분리하며, 다른 CLI 안에서 실행된 호환 훅을 가까운 CLI 프로세스와 대조해 거부한다. AGY App·IDE의 공유 훅은 CLI 경로와 맞지 않으면 수집하지 않는다. 대화 경로는 확인에만 사용하고 파일을 읽거나 경로·본문을 저장하지 않는다.

관측 도우미는 CLI별 설정·사건 어댑터를 사용한다. 출력은 기본 `{}`이며 AGY PreToolUse의 `allow`는 다음 훅/기본 정책에 판단을 맡긴다 (`auto_approve`가 아님). AGY Stop의 빈 decision은 원래 종료를 변경하지 않는다. 승인·거절·계속 실행을 요청하는 출력을 만들지 않는다.

## 수집·저장

- 기본 식별은 읽기 전용이다. `node`라는 이름, 화면·스피너·CPU·출력 중단으로 식별하거나 상태를 추정하지 않는다.
- Python이 없으면 POSIX `ps`와 tmux의 보수적인 실행 파일 식별만 제공한다. 연동 설치에는 Python **3.9 이상**이 필요하다.
- 선택 설치 위치: `~/.local/share/dgl-agent/v1/observer.py`, 관측 DB: 같은 폴더의 `events.sqlite`.
- 사용자 설정: `$CODEX_HOME/hooks.json`(기본 `~/.codex/hooks.json`), `$CLAUDE_CONFIG_DIR/settings.json`(기본 `~/.claude/settings.json`). 기존 inline Codex 설정을 덮어쓰지 않는다.
  Codex의 같은 계층에 inline `[hooks]`가 있으면 CLI가 두 소스를 병합하고 시작 시 경고할 수 있다.
- AGY: `~/.gemini/config/hooks.json`의 `dgl-observer-v1` 이름 아래에만 추가한다. 같은 이름의 사용자 훅과 충돌하면 덮어쓰지 않는다. 도구 사건은 matcher 그룹, 나머지는 직접 handler 형식을 사용하고 사건명 없는 입력을 위해 `record --event`를 사용한다. 기존 사건명이 포함된 `record` 명령 형식도 유지한다.
- Grok: `$GROK_HOME/hooks/dgl-observer-v1.json`(기본 `~/.grok/hooks/dgl-observer-v1.json`). 다른 JSON·TOML·Claude 호환 설정은 변경하지 않는다. Claude 호환 소스의 dgl 훅은 Grok 프로세스에서 Claude 사건으로 수집하지 않는다.
- 설치 잠금, 설정 내용 토큰 재검사, 같은 폴더 임시 파일 교체, 권한 0600의 `.dgl-backup-*` 백업을 사용한다. 기존 훅과 함께 들어 있는 사용자 handler도 해제 시 보존한다.
- SQLite는 최대 2,048개 사건과 실행 주체별 최신 관측을 보관한다. 프롬프트·응답·도구 입력·명령·대화 파일은 기록하거나 읽지 않는다. 프로세스 명령줄은 CLI 진입점 식별 중 메모리에서만 사용한다.
- 서버 소켓·서버 프로세스 시작 정보·패널·CLI PID/시작 정보로 실행을 구분한다. macOS `ps`의 경로 열 잘림을 피한다.
- 기존 SSH의 독립 exec 채널에서 약 2초마다 순차 조회한다. 출력 구독이 없어도 수집하고 조회를 겹치지 않는다. 10초 이상 오래된 관측, 단절, 사건 간격 누락은 현재 관측으로 표시하지 않는다. 누락 이전의 관측은 새 훅이 올 때까지 미확인으로 유지한다.
- 최초 연결·재접속·DB 세대 변경은 기준선만 동기화한다. 과거 사건을 새 알림으로 재발송하지 않는다.

## 알림

Rust `Inbox` 하나가 앱의 모든 연결을 집계한다. 같은 사용자/호스트·서버 사건의 여러 탭 수신을 중복 제거한다.
앱 내 최근 사건은 실행 중 최대 100건이다. OS 전송은 같은 패널에서 2초 내 수신한 알림을 최신 사건으로 묶는다.
이미 화면에서 읽은 사건은 이후 탭을 바꾸어도 OS에 뒤늦게 알리지 않는다.
앱이 전면이고 대상 패널이 보이면 OS 요청을 생략한다. 데스크톱과 Android 분할 그룹에서 실제 보이는 모든 연결의 패널도 포함한다. 좁은 창·최소 공간 부족 시 숨겨진 그룹은 제외한다.

- macOS: UserNotifications, 번들 ID `kim.jujin.dglc`. 패키지 `.app`에서 사용한다.
- Windows: 자체 AUMID·사용자 Start Menu 바로가기·COM 알림 활성화. ZIP 배포를 유지한다. [등록·실기 절차](WINDOWS.md#6-에이전트-알림).
- Android: `agent-observations-v1` 알림 채널과 `POST_NOTIFICATIONS`. 알림 클릭 Intent에는 사건 ID만 전달한다. 포그라운드 서비스·푸시는 추가하지 않는다.

OS 권한 거부·집중 모드·프로세스 종료 때문에 표시가 보장되지는 않는다. 앱 내 사건은 유지된다.
Android는 앱 프로세스와 SSH 연결이 유지되는 동안만 수신한다.

## 확인한 범위와 제한

| 검사 | 결과 |
|---|---|
| Python 도우미 10개 검사 | 기존 5개 회귀와 공식 형식 24개 fixture, 중립 출력·실패 출력, AGY/Grok 설정·사용자 훅 보존, 반복 설치/해제, CLI별 오류 격리, 버전·호환 훅 오인·대화/자식 분리·중복·본문 미저장 통과 |
| Rust·UniFFI | 관측 3개 검사: 기존 2개 및 네 서비스 wire ID·질문/승인/응답/오류의 공통 알림 경로. 전체 Rust 코어·UniFFI 회귀 통과 |
| GPUI·Kotlin 실행/이동 | GPUI 29개 검사와 Kotlin 전체 JVM 회귀 통과. 실행 명령 8개 바이트·한 번 전송·대상/재연결 변경 차단, 메뉴 폭·60:40 비율·새 서비스만 작은 test 배지·닫힘·포커스 복귀, 종료 알림 대상 이동 거부 확인 |
| 격리 SSH/tmux 3.3a·3.7c | 네 합성 CLI 각각 숨겨진 창, 출력 미구독 수집, 최초 DB 동기화, 새 사건, 패널 이동, 재연결 재알림 방지, 같은 패널의 프로세스 재시작·종료 확인. 생산자는 C로 만든 합성 CLI이며 실제 CLI 검사는 아래와 별개 |
| 실제 Codex 0.163.0 | 임시 HOME·독립 tmux에서 정상 신뢰 UI로 훅을 검토하고 SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, SubagentStart/Stop 수신 확인 |
| 실제 Claude Code 2.1.310 | 같은 격리 방식으로 SessionStart, UserPromptSubmit, 도구 전후, AskUserQuestion, PermissionRequest, PostToolUseFailure, Stop, SubagentStart/Stop, SessionEnd 수신 확인 |
| Android API 36 | 네 ABI Debug APK, JVM 회귀, lint, 알림 계측 2개 통과: 네 서비스의 네이티브 Inbox·중복·읽음/관측 분리, 권한 거부/허용·채널, AGY/Grok OS 알림 제목 갱신, 실제 PendingIntent 클릭의 종료 대상 처리 |
| macOS | GPUI 빌드·ad-hoc 패키징·단위 UI 검사 통과. 테스트 알림 요청은 발생했으나 CUA가 `cgWindowNotFound`로 창을 찾지 못해 OS 배너·클릭 실기 확인은 미완료 |
| Windows | 토스트 어댑터의 Windows GNU 대상 독립 타입 검사와 ZIP 패키저 단위 검사 통과. Windows MSVC 전체 빌드·실행·COM/토스트·DPAPI 실기는 사용자 담당이며 미수행 |

2026-10-25 AGY·Grok 확장 검증은 `fixtures/agent-hooks/contracts.json`의 합성 입력과 격리 SSH/tmux 생산자를 사용한다. 로컬 `--version` 조회는 AGY **1.4.7**, Codex **0.164.0**이며 로그인·모델 호출을 실행하지 않았다. Codex 0.164.0은 설치 버전 처리·공식 형식 fixture 회귀 범위이며, 위의 실제 수신 기록은 0.163.0에 대한 기존 기록이다. AGY·Grok의 인증된 실제 훅 수신은 미검증이다.

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
./gradlew :androidApp:connectedDebugAndroidTest -Pandroid.testInstrumentationRunnerArguments.class=kim.jujin.dglc.AgentNotificationTest
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

창 상태 버튼은 해당 창의 모든 pane 관측과 미확인 사건을 집계한다. 합쳐진 단일 창 세션 행도 같은 필터를 쓴다. 상세의 이동 버튼은 관측의 실제 pane을 선택한다.
