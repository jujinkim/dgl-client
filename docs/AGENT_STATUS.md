# Claude Code·Codex·AGY·Grok Build 관측·상태·알림

Mac·Windows·Linux·Android는 같은 Rust 관측 모델과 알림 큐를 사용한다.
이벤트(활동·질문·승인·응답·오류)와 별도로 계정·남은 사용량·컨텍스트·프롬프트 캐시·모델·추론 수준을 요약한 [에이전트 상태 HUD](#에이전트-상태-hud)를 제공한다.
SSH로 연결한 tmux 서버의 모든 세션·창·패널을 조회한다. 선택되지 않은 탭이나 어느 탭에도 열려 있지 않은 세션의 CLI도 사이드바에 계속 표시한다. 일반 SSH 셸과 데스크톱 Local 연결(로컬 tmux/psmux)에는 관측을 제공하지 않는다.

## CLI 실행 메뉴

하단 `…` 메뉴가 현재 원격 터미널에 보내는 명령입니다. 명령 뒤에 Enter를 한 번 전송합니다.
CLI 설치·로그인은 원격 환경에서 준비하며 DGL은 기존 프로세스를 종료하거나 다시 로그인하지 않습니다.

| 서비스 | 일반 실행 | Bypass 실행 |
|---|---|---|
| Claude Code | `claude` | `claude --dangerously-skip-permissions` |
| Codex | `codex` | `codex --yolo` |
| AGY | `agy` | `agy --dangerously-skip-permissions` |
| Grok Build | `grok` | `grok --always-approve` |

입력 불가 상태에서는 실행하지 않습니다. 메뉴를 연 뒤 연결·탭·pane이 바뀌면 닫으며 실패한 입력을 재전송하지 않습니다.
입력 대상 pane 아래에서 네 CLI 중 하나의 프로세스가 최근 10초 안의 관측으로 확인되면 실행 항목을 비활성화하고 메뉴 위에 실행 중인 CLI를 안내합니다.
명령이 그 CLI의 프롬프트로 제출되는 것을 막기 위한 것이며, 표시할 때와 누를 때 모두 확인합니다. 같은 세션의 다른 pane에서 실행 중인 CLI는 막지 않습니다.
관측이 오래됐거나 없는 경우(일반 SSH 셸 포함)는 상태를 알 수 없으므로 막지 않습니다. 관측은 약 2초 간격이라 CLI를 직접 시작한 직후에는 감지되지 않을 수 있습니다.
서비스명은 일반 실행, 바로 뒤의 번개 아이콘은 Bypass 실행입니다. Bypass는 각 CLI의 권한 확인을 건너뛰는 옵션입니다.
메뉴는 가장 긴 행에 맞는 폭으로 표시하고 외곽 테두리만 유지합니다.
실행 메뉴에는 AGY·Grok Build 배지를 표시하지 않습니다. 두 CLI의 에이전트 관측은 실험 지원이며 실제 CLI 훅 수신은 미검증입니다.

## 사용 방법

1. 호스트에 연결한 뒤 호스트 메뉴의 **에이전트 상태 연동**을 연다. 창 행 오른쪽 상태 버튼은 그 창의 모든 pane 관측 상세를 연다.
2. **설치 전 검사 · 변경 내용 보기**에서 Python, tmux, CLI 버전, 수정할 파일과 추가 훅을 확인한다.
3. 계정·사용량·컨텍스트 구성 등 자세한 상태가 필요하면 **상세 상태 수집 포함**을 선택한다(기본 꺼짐, [상세 상태 수집](#상세-상태-수집-선택)).
   **연동 설치**를 누른다. 설치 대상은 Codex **0.163.0 이상**, Claude Code **2.1.310 이상**의 정식 릴리스다(실제 수신 검증은 0.163.0·2.1.310, 아래 표). 그보다 낮은 버전, pre-release·비정상 버전 문자열은 설치에서 제외한다. AGY·Grok Build는 설치된 CLI에 실험 지원으로 설치를 허용한다. 미설치·설정 오류는 CLI별로 표시하며 다른 정상 CLI의 설치를 막지 않는다.
4. Codex에서는 `/hooks`에서 추가된 도우미 명령을 직접 검토·신뢰한다. 앱은 훅 신뢰를 우회하거나 CLI를 재시작하지 않는다.
5. **설치됨 / 활성화 확인 필요**와 **수신 확인 이벤트**을 구분한다. 후자는 해당 실행에서 실제 받은 이벤트 종류만 표시한다. 데스크톱에서는 관측 항목에 마우스를 올려 수신 확인 이벤트와 진단 ID를 확인한다. 새 실행 또는 다음 실제 훅 수신 전에는 기존 실행의 상태를 복원하지 않는다.
6. 앱 설정에서 종류별 알림과 **OS 알림**을 설정한다. 종류별 알림은 기본 켜짐, OS 알림은 기본 꺼짐이다.
   **에이전트 상태** 카드에서 HUD 표시 위치와 항목을 고른다.

**DaGaLi 훅만 해제**는 기존 사용자 훅·설정을 보존한다. 상세 상태 수집도 함께 해제하고 저장해 둔 원래 값을 복원한다. 도우미·관측 DB와 설정 백업은 자동 삭제하지 않는다.
해제 시 최신 관측을 비우고 DB 세대를 바꿔, 이전 이벤트를 현재 연동 상태나 새 알림으로 되살리지 않는다.
설정 변경 충돌 또는 비정상 설정은 덮어쓰지 않고 재검사를 요구한다. 여러 설정 파일을 바꾸는 도중
실패했다면 일부 파일이 적용되었을 수 있으므로 다시 검사한다. 파일별 백업과 원자적 교체를 사용하며,
다른 프로그램이 같은 파일을 교체 직전에 쓰는 극소한 경쟁 조건까지 POSIX 파일 API로 완전히 잠그지는 못한다.

## 표시가 뜻하는 것

| 표시 | 의미 |
|---|---|
| ⊖ 미감지 | 최근 10초 관측에서 그 창의 CLI 프로세스를 찾지 못함(관측 없음·오래됨 포함) |
| 서비스 아이콘 | 패널 프로세스의 실행 파일 또는 확인된 Node CLI 진입점으로 식별. 상태 훅을 받지 못했으면 서비스 아이콘만 표시 |
| ↻ 활동 | 요청·도구 등 작업 이벤트가 관측됨 |
| ? 질문 / ◇ 승인 | 질문·승인 요청이 발생했음. 현재도 대기 중이라는 뜻은 아님 |
| ○ 응답 확인 | Stop 신호를 받았음. 작업 성공·검토 통과·영구 입력 대기를 뜻하지 않음 |
| ! 오류 | 공식 오류 훅을 받았음. 종료 코드나 화면 문구로 추정하지 않음 |
| 상태 아이콘 없음 / 상태 미확인 | 미연동, Python/조회 문제, 단절, 누락 또는 오래된 관측 |

패널의 대표 이벤트는 질문·승인 → 오류 → 응답 확인 → 활동 순이다. 읽지 않은 이벤트와 관측은 별개다.
다른 작업이 진행되어도 읽지 않은 질문은 알림에 남는다. 읽음 처리나 알림 클릭은 원격 입력·승인을 전송하지 않는다.
서브에이전트 이벤트는 별도 실행 주체로 표시하며 `SubagentStop`을 부모의 Stop으로 올리지 않는다.
같은 CLI 프로세스의 다른 대화도 별도 주체로 구분한다. AGY는 새 실행의 `SessionStart`로 주 대화를 연결하므로 이미 실행 중인 CLI에 설치한 것만으로 상태를 복원하지 않는다.
사이드바 창 버튼은 테두리 없이 서비스 아이콘 → 상태 아이콘 → 읽지 않은 수 순으로 표시한다. 창 버튼은 합쳐진 세션 행에도 표시한다. 상태 미확인·읽지 않은 이벤트 수·상세의 실행 목록은 같은 연결과 창으로 필터링한다. 연결 탭 집계 배지는 표시하지 않는다.
배지 상세와 알림에서 세션·창·패널로 이동할 수 있다. 종료·재생성된 대상은 이동을 거부하고 재접속하지 않는다.

## 서비스별 수집 계약

아래는 공식 계약에 맞춰 **수집 가능한 이벤트**이다. 설치 검사의 `capabilities`와 훅 목록 `events`는 실제 수신 증거가 아니다. 관측의 `supported`는 해당 실행 주체가 실제 받은 종류만 포함한다.

| 서비스 | 활동 | 질문·승인 | 오류 | 응답 종료·중단 |
|---|---|---|---|---|
| Claude Code | 세션 시작·요청·도구 전후 | `AskUserQuestion`, `PermissionRequest` | `PostToolUseFailure`, `StopFailure` | `Stop`은 응답, `SessionEnd`·`SubagentStop`은 종료 |
| Codex | 세션 시작·요청·도구 전후 | `PermissionRequest`만 지원; 질문 미지원 | 일반 오류 미지원 | `Stop`은 응답, `Interrupt`·`SessionEnd`·`SubagentStop`은 종료 |
| AGY (`test`) | 세션 시작·모델 호출 전후·도구 전후 | `ask_question`, 명시적 `ask_permission` | `PostToolUse.error`, `Stop.error` 또는 오류 종료 사유 | `Stop.terminationReason=model_stop`만 응답; 다른 사유는 종료 |
| Grok Build (`test`) | 세션 시작·요청·도구 전후·권한 거절 | `ask_user_question`, `Notification.permission_prompt` | `PostToolUseFailure`, `StopFailure` | 정상 `Stop`만 응답; `StopCancelled`·`SessionEnd`·`SubagentStop`은 종료 |

AGY의 일반 권한 팝업 감지는 지원하지 않는다. Grok의 `idle_prompt`는 오류·중단 뒤에도 발생하므로 무시하고, `task_complete`도 Stop과 중복 알림을 만들지 않는다.
Grok의 `subagentType`·대화 ID와 AGY의 `conversationId`를 분리하며, 다른 CLI 안에서 실행된 호환 훅을 가까운 CLI 프로세스와 대조해 거부한다. AGY App·IDE의 공유 훅은 CLI 경로와 맞지 않으면 수집하지 않는다. 대화 경로는 확인에만 사용하고 파일을 읽거나 경로·본문을 저장하지 않는다.

관측 도우미는 CLI별 설정·이벤트 어댑터를 사용한다. 출력은 기본 `{}`이며 AGY PreToolUse의 `allow`는 다음 훅/기본 정책에 판단을 맡긴다 (`auto_approve`가 아님). AGY Stop의 빈 decision은 원래 종료를 변경하지 않는다. 승인·거절·계속 실행을 요청하는 출력을 만들지 않는다.

## 에이전트 상태 HUD

실행 중인 AI CLI의 계정·남은 사용량·컨텍스트·프롬프트 캐시·모델·추론 수준을 요약한다. 각 CLI 자체 statusline과는 별개이며 원격 호스트의 관측 도우미가 모은 값을 기존 poll(약 2초)로 읽는다.

| 위치 | 내용 |
|---|---|
| 데스크톱 하단 상태바 | 연결 상태 다음에 포커스 pane 에이전트의 칩 `아이콘 모델 · 추론 수준 · ctx 42% · 5h 63%`. 포커스 pane에 상태가 없으면 같은 연결에서 상태가 있는 첫 관측을 쓴다. 켜진 항목 중 값을 아는 것만 표시하고, 폭이 좁으면 비용 → 캐시 → 추론 수준 → 한도 → 모델 → 컨텍스트 순으로 뺀다. 칩을 누르면 활성 연결의 모든 에이전트 상세 팝오버를 연다 |
| 데스크톱 사이드바 하단 | 접을 수 있는 **에이전트 상태** 패널. 활성 연결의 에이전트마다 한 줄과 컨텍스트 %·가장 높은 한도 % 미니 게이지. 접힘 상태는 기기 로컬 |
| Android 넓은 화면(840dp 이상) | 상태 행의 칩과 사이드바 하단 패널(데스크톱과 같은 내용) |
| Android 폰 | 헤더 둘째 줄의 칩. 누르면 바텀시트에 같은 상세를 연다 |
| 설정 → **에이전트 상태** | 상태바 표시·사이드바 표시와 항목(계정·남은 사용량·컨텍스트·컨텍스트 구성·프롬프트 캐시·모델·추론 수준·예상 비용) 선택. 예상 비용만 기본 꺼짐. 기기 로컬 설정(`Preferences.agentStatus`)이며 설정 동기화·내보내기 대상이 아니다 |

상세(팝오버·패널·바텀시트)에는 켜진 항목만 표시한다.

- 계정 종류(구독·API 키·클라우드 공급자·기업·기타)와 플랜.
- 한도별 사용 % 게이지와 초기화까지 남은 시간. 남은 시간은 `resets_at`으로 앱이 계산한다.
- 컨텍스트 사용 토큰/창 크기 게이지, 자동 압축 기준 %, 컨텍스트 구성 누적 막대와 범례(구성 창 크기 대비 %).
- 프롬프트 캐시의 미캐시 입력·캐시 읽기·캐시 쓰기·출력 토큰, 적중률, 마지막 요청 경과 시간, Claude TTL과 추정 만료.
- 모델·추론 수준·사고 모드, 예상 비용(켰을 때), 관측 시각.

값은 툴팁이 아니라 보이는 텍스트로 표시한다. 모르는 값을 0으로 채우지 않는다.
**실행 중인 에이전트 없음**, **관측이 오래됨**(마지막 표본이 1시간 이상 전), **상세 상태 수집이 설치되지 않았습니다**, **이 CLI는 제공하지 않습니다**, **이 CLI는 컨텍스트 구성을 제공하지 않습니다**를 구분해 표시한다.
설치할 상세 수집기가 없는 Codex·Grok Build는 "설치되지 않았습니다" 대신 제공하지 않음 또는 알 수 없음으로 표시한다. 컨텍스트 구성은 컨텍스트 항목을 꺼도 따로 표시한다.
사용량 한도는 계정 단위라 같은 계정의 여러 pane이 같은 값을 공유한다. 앱은 계정 키가 같은 관측 중 한도 표본 시각(`limits_at`, 없으면 `at`)이 가장 최근인 한도를 쓴다. 다른 훅이 관측 시각만 갱신해도 오래된 한도가 이기지 않는다.
계정 키는 조직 ID(있을 때) 또는 CLI·계정 종류·플랜의 SHA-256이며 표본 선택에만 쓴다.

### CLI별 제공 항목

| 항목 | Claude Code | Codex | AGY (실험) | Grok Build (실험) |
|---|---|---|---|---|
| 수집 경로 | 훅 + 상세 수집 mod | 훅 + 읽기 전용 캐시 파일·state DB | 훅 + 상세 수집 statusline 래퍼 | 훅 수신 때 세션 파일 읽기 |
| 계정 종류·플랜 | mod `$.session.identity()`. 거부되거나 계정 종류를 모르면 한도 정보가 있을 때만 구독으로 추정 | 수집 안 함 | statusline `plan_tier`(플랜만, 구독/API 키 구분 없음) | 수집 안 함 |
| 남은 사용량 | mod `rateLimits`: 5시간·7일·지출 한도. API 키 계정은 한도 없음 | 수집 안 함(4단계) | statusline `quota` 버킷: 남은 비율 → 사용 %, 라벨은 버킷 ID | 수집 안 함 |
| 컨텍스트 | mod 토큰·창 크기·% | `PostModelRequest` `total_tokens` + `models_cache.json` 창 크기 | statusline `context_window` | `signals.json` |
| 컨텍스트 구성 | mod `$.session.usage({breakdown: 'summary'})`(로컬 추정, 네트워크 없음) | 제공 안 함 | 제공 안 함 | 제공 안 함 |
| 프롬프트 캐시 | mod 마지막 요청 토큰 + TTL(5m/1h) | `PostModelRequest`(`sampling` 요청만) | statusline `current_usage` | `usage.json` 마지막 turn |
| 모델 | mod, `PostModelSwitch` 훅 | 훅 공통 `model` | 훅 `modelName`, statusline `model` | `signals.json` `primaryModelId` |
| 추론 수준·사고 모드 | 도구 훅 `effort.level` | state DB `reasoning_effort` | statusline `model.effort`, `thinking` | 제공 안 함 |
| 예상 비용 | mod `cost` | 제공 안 함 | statusline `cost.total_usd` | `usage.json` `costUsdTicks` |
| 경보(한도·결제·인증) | `StopFailure.error` | 제공 안 함 | 제공 안 함 | `StopFailure`의 `errorType` 또는 `error` |

- **Claude Code**(2.1.310 이상): mod는 Claude Code 프로세스 안에서 실행되는 플러그인 훅 모듈이다. `session.measure`·`turn.complete`·`session.end`만 구독한다.
  1초에 최대 한 번 `~/.local/share/dgl-agent/v1/claude/<CLAUDE_PID>.json`에 화이트리스트 값만 쓰고, 모든 단계를 예외 처리해 세션을 방해하지 않는다. 세션 ID는 SHA-256으로만 남긴다.
  도우미 poll은 관측된 Claude 프로세스 PID로 파일을 찾는다. PID는 앱으로 보내지 않는다. 6시간보다 오래된 파일, 심볼릭 링크, 불완전한 JSON, PID 불일치, 프로세스 시작 전 표본은 무시하고, 24시간 지난 파일은 Claude `SessionStart` 훅 때 정리한다.
  구성 행은 범주·이름·토큰·종류만 남기고 메모리 파일 경로·MCP 도구·에이전트 목록은 버린다. `claude auth status`는 호출하지 않는다.
  상세 수집 없이도 훅으로 모델(`PostModelSwitch`)·추론 수준·경보는 수집한다.
- **Codex**(0.163.0 이상): 설치된 버전이 **0.171.0 이상**일 때만 `PostModelRequest` 훅을 추가한다. 이후 업데이트했다면 연동을 다시 설치하고 `/hooks`에서 추가된 훅을 검토·신뢰한다.
  0.171.0부터 Codex는 같은 `CODEX_HOME`의 모든 TUI를 `codex app-server` 데몬 하나로 처리하고 훅도 데몬이 실행한다. 데몬 환경의 `TMUX_PANE`은 데몬을 처음 띄운 pane이므로 귀속에 쓰지 않는다.
  TUI 공용 데몬(`--managed-daemon`)이 실행한 훅만 귀속하며, IDE 등 다른 클라이언트의 `app-server` 훅은 버린다. 스레드(`session_id`, SHA-256만 저장)는 처음 귀속된 TUI에 고정하고,
  Codex는 스레드와 클라이언트 연결의 대응을 외부에 공개하지 않으므로(훅 환경·payload·데몬 파일 모두 확인), 처음 보는 스레드는 아래 순서로 연결한다.
  1. `codex resume <id>`로 시작한 TUI가 그 스레드 ID를 인자로 가진 경우 그 TUI.
  2. payload `cwd`와 작업 디렉터리가 같은 Codex TUI가 정확히 하나면 그 TUI.
  3. 그 외에는 데몬 제어 소켓의 연결 끝점(macOS `lsof -U`, Linux `ss -xp`)으로 이 데몬의 클라이언트인 TUI만 남긴다. 그중 2번 조건을 만족하는 TUI가 하나면 그 TUI다.
     아니면 **그 TUI가 시작하며 만든 스레드인지**로 정한다. TUI는 시작 직후(로컬 1~5초, 첫 프롬프트 전) 첫 스레드를 만들고, 스레드 ID(UUIDv7)에 생성 시각이 들어 있다.
     TUI 시작 후 20초 안에 생성됐고 그 사이에 알려진 다른 스레드가 없는 TUI가 정확히 하나일 때만 연결한다.
     알려진 스레드는 이름과 ID만으로 모은다(로드 중 잠금 파일, TUI 표식 파일, state DB `threads.id`). 출처가 놓친 스레드는 판정을 엄격하게 만들 뿐이다.
     같은 디렉터리에 TUI가 여럿이면 그중 하나여야 하고, 없으면(`codex --cd`) 이 데몬의 어떤 클라이언트든 된다.
  소켓 표를 읽지 못하거나, 시작이 겹치거나(같은 초 포함), `resume`·`fork`로 연 TUI이거나, 같은 디렉터리 여러 TUI 중 한 곳의 `/new`·`/resume` 스레드이거나, 스레드 ID가 없으면 다른 pane에 잘못 붙이지 않고 버린다.
  데몬 프로세스는 Codex CLI 관측에서 제외한다(도우미와 Python 없는 기본 조회 모두). 기본 추론 수준을 쓰면 state DB 값이 비어 있어 추론 수준은 표시하지 않는다.
  `input_tokens`는 캐시 포함이므로 미캐시 입력은 input − cached − write(0 미만은 0)로 계산하고, 압축 요청은 제외한다.
  창 크기는 `$CODEX_HOME/models_cache.json`의 `context_window` × `effective_context_window_percent`/100이다.
  추론 수준은 가장 높은 번호의 `$CODEX_HOME/state_*.sqlite`를 읽기 전용·0.2초 제한으로 열어 해당 스레드의 `reasoning_effort`·`model` 두 열만 읽는다. 비공개 형식이라 실패하면 생략한다.
  한도·계정은 대화 파일(rollout)이나 app-server 조회가 필요해 이번 범위에서 제외했다. 화면에 한도·계정 값을 표시하지 않으며 0으로 채우지 않는다.
- **AGY**: statusline 입력 JSON에서 화이트리스트만 남기고 이메일·대화 제목·transcript 경로·cwd·workspace·세션 ID는 버린다. `current_usage.input_tokens`는 미캐시 입력만 센다(로컬 데이터로 확인).
  quota는 AGY 내부 캐시 값이라 분 단위로 늦을 수 있다. 상세 수집 없이는 훅의 모델만 수집한다.
- **Grok Build**: `SessionStart`·`Stop`·`StopFailure` 훅 수신 때 `$GROK_HOME/sessions/*/<sessionId>/signals.json`(256 KB 이하)과 같은 폴더의 `usage.json`만 읽는다.
  검색은 64개까지, 세션 ID는 안전한 파일 이름일 때만 사용한다. 측정 전 기본값 0은 알 수 없음으로 처리한다. Grok 설정은 바꾸지 않는다.

### 프롬프트 캐시

여기서 캐시는 앱·원격 호스트의 캐시가 아니라 **LLM 공급자 서버의 프롬프트 캐시**다. 토큰 수는 마지막 요청 기준이며 적중률은 읽기 ÷ (미캐시 입력 + 읽기 + 쓰기)다.
TTL은 Claude만 제공한다. 마지막 turn의 `ephemeral_5m_input_tokens`·`ephemeral_1h_input_tokens` 중 0보다 큰 쪽을 쓰고 둘 다 있으면 1h로 본다.
만료 시각은 마지막 요청 시각 + TTL로 계산한 **추정**이다. 공급자가 더 일찍 비우거나, DGL이 관측하지 못한 요청이 캐시를 갱신할 수 있으므로 실제 캐시 상태를 보장하지 않는다. Codex·AGY·Grok은 TTL을 표시하지 않는다.

### 갱신 주기

| 데이터 | 갱신 계기 | 앱 반영 |
|---|---|---|
| Claude 컨텍스트·한도·비용·캐시·구성·계정 | mod의 `session.measure`·`turn.complete`(최소 1초 간격 기록) | 기록 후 다음 poll(약 2초) |
| Claude 모델·추론 수준·경보 | mod, `PostModelSwitch`·도구 훅·`StopFailure` | 같음 |
| Codex 모델·토큰·컨텍스트·캐시 | `PostModelRequest`(요청 완료마다) | 같음 |
| Codex 추론 수준 | `SessionStart`·`UserPromptSubmit`·`PostModelRequest`·`Stop` 훅 때 state DB 조회 | 같음 |
| AGY 모델·컨텍스트·캐시·비용 | AGY가 statusline을 다시 그릴 때 | 같음 |
| AGY quota | AGY 내부 캐시 갱신 시(분 단위로 조사됨, 미검증) | 분 단위 |
| Grok 컨텍스트·모델·캐시·비용 | `SessionStart`·`Stop`·`StopFailure` 훅 | turn 종료 후 다음 poll |
| 초기화까지 남은 시간·캐시 경과 시간 | 앱이 `resets_at`·`last_at`으로 계산 | 화면 갱신(약 2초) |

별도 원격 폴링 루프나 타이머는 없다. 유휴 중에는 새 표본이 없지만 값도 바뀌지 않는다.
도우미는 값마다 관측 시각을 두고 더 새로운 값을 쓴다(같은 시각이면 mod 우선). 24시간 지난 상태 행은 정리한다.
CLI 프로세스 자체의 관측 유효 시간(10초)은 기존 규칙을 따른다.

## 상세 상태 수집 (선택)

연동 다이얼로그에서 **상세 상태 수집 포함**을 선택해 설치할 때만 원격 CLI 설정을 추가로 바꾼다. 기본은 꺼짐이다.
선택하지 않고 다시 설치하면 이미 있는 상세 수집을 해제하고, **DaGaLi 훅만 해제**는 모두 해제한다.

| CLI | 설치 시 변경 | 해제 시 |
|---|---|---|
| Claude Code 2.1.310 이상 | `~/.local/share/dgl-agent/v1/claude-mod/`(`.claude-plugin/plugin.json`·`hooks/hooks.json`·`hooks/register.ts`)를 만들고, `$CLAUDE_CONFIG_DIR/settings.json`(기본 `~/.claude/settings.json`)의 `env.CLAUDE_CODE_PLUGIN_DIRS`에 그 절대 경로를 `:`로 덧붙인다. 기존 항목은 유지한다 | DGL 경로만 뺀다. 원래 없던 키나 `env` 객체는 다시 없앤다(원래 상태는 `claude-mod.json`에 저장) |
| AGY | `~/.gemini/antigravity-cli/settings.json`의 `statusLine`을 도우미 래퍼(`observer.py statusline --agent Agy --tmux …`)로 바꾼다. 원래 값은 `statusline-Agy.json`(0600)에 저장한다. 원래 statusline이 없으면 `stack_with_default: true`로 AGY 기본 줄을 유지한다 | 저장한 원래 값을 그대로 복원한다(없던 키는 제거) |
| Codex·Grok Build | 추가 설정 없음. 훅과 읽기 전용 파일로 수집한다 | — |

AGY 래퍼는 stdin(최대 1 MB)을 한 번 읽어 화이트리스트를 기록한다(SQLite 대기 0.2초, 실패는 드러내지 않음).
저장한 원래 명령은 같은 stdin 바이트·환경·작업 폴더로 `sh -c` 실행하고 stdout 바이트와 종료 코드를 그대로 전달한다. 원래 명령은 기록과 동시에 시작한다.
원래 명령은 별도 프로세스 그룹에서 실행한다. 4.5초를 넘기면 그 그룹 전체(자식·백그라운드 작업 포함)를 종료하고 받은 출력까지 전달한 뒤 종료 코드 124를 낸다.
원래 명령의 `sh`가 끝났는데 백그라운드 작업이 stdout을 열어 두면 0.2초만 더 기다린 뒤 원래 종료 코드를 전달한다. 원래 명령이 없거나 꺼져 있으면 아무것도 출력하지 않고 0으로 끝난다.

저장한 원래 값이 있는데 DGL 래퍼나 mod 경로가 사라졌으면 사용자가 설정을 바꾼 것으로 보고 **사용자가 설정을 바꿔 상세 수집이 해제됨**으로 표시한다. 자동으로 다시 감싸지 않는다.
상세 수집 없이 재설치하거나 해제하면 저장 값을 지운다. 그 뒤 다시 선택하면 사용자의 새 설정을 감싼다.
설정 변경은 이미 실행 중인 CLI에 적용되지 않으므로 설치 후 CLI를 다시 시작한다. 앱은 CLI를 재시작하지 않는다.
검사 결과는 CLI별 `status_collection`(지원·설치됨·교체됨·종류: mod/statusline/hooks/files)으로 설치 상태를 표시한다. 지원 대상은 Claude Code 2.1.310 이상과 설치된 AGY다.
변경에는 기존 설치 잠금·내용 토큰 재검사·백업·원자 교체를 그대로 쓰며 AGY 설정과 저장 파일도 토큰에 포함한다.

## 수집·저장

- 기본 식별은 읽기 전용이다. `node`라는 이름, 화면·스피너·CPU·출력 중단으로 식별하거나 상태를 추정하지 않는다.
- Python이 없으면 POSIX `ps`와 tmux의 보수적인 실행 파일 식별만 제공한다. 연동 설치에는 Python **3.9 이상**이 필요하다.
- 선택 설치 위치: `~/.local/share/dgl-agent/v1/observer.py`, 관측 DB: 같은 폴더의 `events.sqlite`.
- 사용자 설정: `$CODEX_HOME/hooks.json`(기본 `~/.codex/hooks.json`), `$CLAUDE_CONFIG_DIR/settings.json`(기본 `~/.claude/settings.json`). 기존 inline Codex 설정을 덮어쓰지 않는다.
  Codex의 같은 계층에 inline `[hooks]`가 있으면 CLI가 두 소스를 병합하고 시작 시 경고할 수 있다.
- AGY: `~/.gemini/config/hooks.json`의 `dgl-observer-v1` 이름 아래에만 추가한다. 같은 이름의 사용자 훅과 충돌하면 덮어쓰지 않는다. 도구 이벤트는 matcher 그룹, 나머지는 직접 handler 형식을 사용하고 이벤트명 없는 입력을 위해 `record --event`를 사용한다. 기존 이벤트명이 포함된 `record` 명령 형식도 유지한다.
- Grok: `$GROK_HOME/hooks/dgl-observer-v1.json`(기본 `~/.grok/hooks/dgl-observer-v1.json`). 다른 JSON·TOML·Claude 호환 설정은 변경하지 않는다. Claude 호환 소스의 dgl 훅은 Grok 프로세스에서 Claude 이벤트로 수집하지 않는다.
- 설치 잠금, 설정 내용 토큰 재검사, 같은 폴더 임시 파일 교체, 권한 0600의 `.dgl-backup-*` 백업을 사용한다. 기존 훅과 함께 들어 있는 사용자 handler도 해제 시 보존한다.
- 상세 상태 수집을 선택한 경우에만 `$CLAUDE_CONFIG_DIR/settings.json`의 `env.CLAUDE_CODE_PLUGIN_DIRS`와 `~/.gemini/antigravity-cli/settings.json`의 `statusLine`을 바꾼다([상세 상태 수집](#상세-상태-수집-선택)).
- SQLite는 최대 2,048개 이벤트와 실행 주체별 최신 관측을 보관한다. 프롬프트·응답·도구 입력·명령·대화 파일은 기록하거나 읽지 않는다. 프로세스 명령줄은 CLI 진입점 식별 중 메모리에서만 사용한다.
- 상태 값은 같은 DB의 `status` 표에 실행 주체별로 보관하고 24시간 지나면 정리한다. 모델 선택·요청 완료처럼 상태만 바꾸는 훅(`PostModelSwitch`·`PostModelRequest`)은 이벤트·알림·수신 확인 이벤트를 만들지 않는다.
- 상태는 화이트리스트 필드만 저장한다. 이메일, 세션·대화 제목, transcript 경로, 프롬프트, 마지막 응답, 압축 요약, cwd·workspace 경로, 원시 세션·조직 ID, 메모리 파일 경로는 저장·전송하지 않는다.
  문자열은 128자(추론 수준 32자, 이름 64자), 한도 16개, 구성 24행으로 제한하고 범위를 벗어난 수치는 버린다. 앱도 같은 경계로 다시 검사해 잘못된 상태만 버리고 나머지 관측은 유지한다.
- transcript·Codex rollout·Grok `unified.jsonl` 같은 대화 파일과 대화 DB, 인증 정보 파일은 상태 수집에도 읽지 않는다.
  읽는 파일은 Codex `models_cache.json`·state DB의 세 열(`reasoning_effort`·`model`, 귀속 판정용 `id`), Grok 세션의 `signals.json`·`usage.json`, DGL mod가 쓴 파일뿐이다.
  Codex 귀속 판정은 `thread-writer-locks`·`tui-thread-reference-capabilities` 폴더의 파일 이름과 데몬 소켓의 연결 끝점만 보고, 연결 내용은 읽지 않는다.
- 서버 소켓·서버 프로세스 시작 정보·패널·CLI PID/시작 정보로 실행을 구분한다. macOS `ps`의 경로 열 잘림을 피한다.
- 연결마다 기존 SSH의 독립 exec 채널에 로그인 셸로 도우미를 한 번 실행해 두고, 약 2초마다 요청 한 줄·응답 한 줄로 순차 조회한다. Python이 없거나 도우미가 응답하지 않으면 30초 동안 tmux·`ps` 기본 조회로 대신한 뒤 다시 시작한다. 출력 구독이 없어도 수집하고 조회를 겹치지 않는다. 10초 이상 오래된 관측, 단절, 이벤트 간격 누락은 현재 관측으로 표시하지 않는다. 누락 이전의 관측은 새 훅이 올 때까지 미확인으로 유지한다.
- 최초 연결·재접속·DB 세대 변경은 기준선만 동기화한다. 과거 이벤트를 새 알림으로 재발송하지 않는다.
- 도우미는 오류·제한을 코드로만 보고하고, 화면 문구와 번역은 앱 문구 목록(`locales/*.json`)이 맡는다.

## 알림

Rust `Inbox::tick` 하나가 데스크톱과 Android의 모든 연결을 같은 규칙으로 집계한다. 같은 사용자/호스트·서버 이벤트의 여러 탭 수신을 중복 제거한다.
코어가 관측 중인 연결(Ready·Restoring·Selecting)의 스냅샷만 수집하며, 묶인 알림은 전송 시점에도 그 실행이 관측 중일 때만 OS에 보낸다.
앱 내 최근 이벤트는 실행 중 최대 100건이다. OS 전송은 같은 패널에서 2초 내 수신한 알림을 최신 이벤트로 묶는다.
이미 화면에서 읽은 이벤트는 이후 탭을 바꾸어도 OS에 뒤늦게 알리지 않는다.
앱이 전면이고 대상 패널이 보이면 OS 요청을 생략한다. 데스크톱과 Android 분할 그룹에서 실제 보이는 모든 연결의 패널도 포함한다. 좁은 창·최소 공간 부족 시 숨겨진 그룹은 제외한다.

- macOS: UserNotifications, 번들 ID `kim.jujin.dglc`. 패키지 `.app`에서 사용한다.
- Windows: ZIP판은 자체 AUMID·사용자 Start Menu 바로가기·COM 알림 활성화, Store MSIX판은 패키지 ID와 매니페스트 선언 활성화. [등록·실기 절차](WINDOWS.md#7-에이전트-알림).
- Linux: 세션 D-Bus의 freedesktop `org.freedesktop.Notifications`. `DBUS_SESSION_BUS_ADDRESS`가 없으면 OS 알림을 보내지 않는다. 실기 표시·클릭은 미검증([Linux 안내](LINUX.md)).
- Android: `agent-observations-v1` 알림 채널과 `POST_NOTIFICATIONS`. 알림 클릭 Intent에는 이벤트 ID만 전달한다. 알림 전용 포그라운드 서비스·푸시는 없다.

OS 권한 거부·집중 모드·프로세스 종료 때문에 표시가 보장되지는 않는다. 앱 내 이벤트는 유지된다.
Android는 앱 프로세스와 SSH 연결이 유지되는 동안만 수신한다. 백그라운드·화면 잠금 중에는 `keepConnectionsInBackground`(기본 켬)의 연결 유지 서비스가 SSH 연결을 지키는 동안만 받는다.

## 확인한 범위와 제한

| 검사 | 결과 |
|---|---|
| Python 도우미 상태 수집 16개 검사 | 민감 필드가 섞인 입력의 화이트리스트, 경계·병합 규칙, mod의 모델·한도 표본 시각 유지, 상태 전용 훅의 이벤트 미생성, 24시간 정리와 `status` 표 없는 이전 DB, AGY 래퍼의 바이트 투명성·종료 코드·원래 명령 없음/꺼짐/자기 참조·큰 입력·기록 실패·4.5초 기한과 프로세스 그룹 종료·백그라운드 작업이 있어도 원래 종료 코드 유지, 설정 쓰기 실패 시 저장 원본 폐기와 재설치, `CLAUDE_CODE_PLUGIN_DIRS` 기존 항목·없던 키 복원, 교체 감지, Codex 0.171.0 게이트, Grok 파일 해석, PID별 Claude mod 파일(오래됨·불완전·다른 PID·심볼릭 링크·재사용 PID) 통과. 모두 합성 입력이며 기존 13개와 합쳐 29개. 생성한 mod를 Node(형식 제거 실행, 23.6 이상)로 돌려 신원 거부·로그아웃 때 한도가 있으면 구독으로 추정하는지 확인하며, Node가 없으면 skip으로 남긴다 |
| Claude mod 형식 | 생성한 mod를 `claude plugin validate`(Claude Code 2.1.330)로 검사해 통과. 단위 검사는 `claude`가 PATH에 있을 때만 이 검사를 실행하고 없으면 skip으로 남긴다. 작업 중 TypeScript 5.6.3 strict 형식 검사도 한 번 통과했으나 반복 실행 진입점은 없다 |
| Rust·Kotlin 상태 모델 | `dgl-core` 상태 6개 검사(파싱, 잘못된 상태만 폐기, 창·pane 선택과 계정별 최신 한도, 토큰·시간 표시, `--status` 설치 인자, 설정 기본값)와 계약 fixture의 `agentStatus` 기본값. Kotlin `AgentStatusTest` 3개와 `PreferencesTest` 설정 기본값·부분 입력·재시작 유지 통과 |
| 상태 HUD 화면 | 상태바·팝오버·사이드바 패널·설정 카드·Android 칩/바텀시트의 실제 GUI 표시는 확인하지 않았다(GUI 미실행) |
| 실제 CLI 상태 수집 (2026-11-22, 로컬 macOS, 임시 HOME·전용 tmux) | **Claude Code 2.1.330 확인**: `install --status`로 등록한 mod가 대화형 TUI에 로드되어 `$.fs.write`로 PID 파일을 썼고, 모델·컨텍스트·카테고리 분해·캐시(TTL 1h)·비용·5시간/7일 한도·구독 추정이 `serve` poll에서 pane에 붙었다(Rust 역직렬화·validate 통과). **AGY 1.5.14 확인**: statusline 래퍼 호출과 `TMUX_PANE` 상속, 모델·effort·컨텍스트·캐시·`plan_tier`·quota 버킷 수신. **Codex 0.171.0 확인(데몬 귀속 수정 후)**: 두 pane의 TUI가 데몬 하나를 공유해도 각 스레드의 이벤트·모델·컨텍스트(창 475k)·캐시가 자기 pane에 붙었고, 데몬이 고아(부모 launchd)가 된 뒤에도 유지됐다. 같은 디렉터리의 TUI 3개를 순서를 섞어 프롬프트해도 각자 자기 pane에 붙었고, 그중 한 TUI의 `/new` 스레드는 버려졌다(소켓 대응은 macOS `lsof`로 확인, Linux `ss`는 합성 출력 단위 검사만). 수정 전에는 데몬이 첫 TUI 아래면 다른 pane 이벤트가 첫 pane으로 가고, 고아면 모두 버려졌다. Grok·원격 SSH 호스트·API 키 계정은 미검증 |
| Python 도우미 기존 13개 검사 | 기존 5개 회귀, 연결당 도우미의 줄 단위 응답·오류 코드, 화면 문구 미포함과 공식 형식 24개 fixture, 중립 출력·실패 출력, AGY/Grok 설정·사용자 훅 보존, 반복 설치/해제, CLI별 오류 격리, 버전·호환 훅 오인·대화/자식 분리·중복·본문 미저장 통과 |
| Rust·UniFFI | 관측 3개 검사: 기존 2개 및 네 서비스 wire ID·질문/승인/응답/오류의 공통 알림 경로. 전체 Rust 코어·UniFFI 회귀 통과 |
| GPUI·Kotlin 실행/이동 | 실행 명령 8개 바이트·한 번 전송·대상/재연결 변경 차단·입력 pane의 CLI 실행 중 차단(코어·Kotlin 단위 검사)·닫힘·포커스 복귀·종료 알림 대상 이동 거부를 회귀 검사한다 |
| 격리 SSH/tmux 3.3a·3.7c | 네 합성 CLI 각각 숨겨진 창, 출력 미구독 수집, 최초 DB 동기화, 새 이벤트, 패널 이동, 재연결 재알림 방지, 같은 패널의 프로세스 재시작·종료 확인. 생산자는 C로 만든 합성 CLI이며 실제 CLI 검사는 아래와 별개 |
| 실제 Codex 0.163.0 | 임시 HOME·독립 tmux에서 정상 신뢰 UI로 훅을 검토하고 SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, SubagentStart/Stop 수신 확인 |
| 실제 Claude Code 2.1.310 | 같은 격리 방식으로 SessionStart, UserPromptSubmit, 도구 전후, AskUserQuestion, PermissionRequest, PostToolUseFailure, Stop, SubagentStart/Stop, SessionEnd 수신 확인 |
| Android API 36 | 네 ABI Debug APK, JVM 회귀, lint, 알림 계측 2개 통과(`channelPermissionAndClickIntentRetainOnlyEventId`는 시작 시 알림 권한이 이미 허용돼 있으면 실패, [TASKS.md](../TASKS.md)): 네 서비스의 네이티브 Inbox·중복·읽음/관측 분리, 권한 거부/허용·채널, AGY/Grok OS 알림 제목 갱신, 실제 PendingIntent 클릭의 종료 대상 처리 |
| macOS | GPUI 빌드·ad-hoc 패키징·단위 UI 검사 통과. OS 배너 표시·클릭 실기는 미검증 |
| Windows | ZIP·MSIX 패키저 단위 검사(`scripts/tests/test_windows_package.py`) 통과. 토스트 어댑터는 Mac에서 dgl-core 대체 타입을 둔 임시 crate로 Windows GNU 대상 타입 검사를 했을 뿐 반복 실행 진입점이 없다. Windows MSVC 전체 빌드·실행·COM/토스트·DPAPI 실기는 사용자 담당이며 미검증. 상태 HUD의 Windows 빌드·표시도 미검증 |

AGY·Grok 검사는 `fixtures/agent-hooks/contracts.json`의 합성 입력과 격리 SSH/tmux 생산자를 사용한다. AGY·Grok의 인증된 실제 훅 수신은 미검증이다. 확인한 로컬 `--version`은 AGY 1.4.7·Codex 0.164.0이며 로그인·모델 호출은 하지 않았다.
Codex 0.164.0은 설치 버전 처리·공식 형식 fixture 회귀 범위이며, 실제 훅 수신 기록은 0.163.0에만 있다.

Codex PermissionRequest의 실제 수신은 미검증이다. 실행 정책의 자동 승인 검토가 테스트용 sandbox 승인 요청을 거부해 발생시키지 못했다. Codex 질문 도구와 일반 오류는 수집 지원으로 표시하지 않는다.
Claude StopFailure와 Codex Interrupt/SessionEnd 등 모든 설정 훅의 실제 발생을 재현한 것은 아니다.
설치 가능한 훅 목록과 실제 **수신 확인 이벤트**을 혼동하지 않는다. 지원하지 않는 이벤트는 터미널이나 대화 로그로 보충하지 않는다.
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

상태 수집 검사는 임시 HOME만 사용하고 실제 `~/.claude`·`~/.codex`·`~/.gemini`·`~/.grok`을 바꾸지 않는다. `verify_agent_cli.py`는 상태 HUD 값을 확인하지 않는다.
실제 CLI 도구는 임시 HOME에 인증 파일을 0600으로 복사하고 종료 시 지운다. 사용자 설정·기존 CLI를 변경하지 않는다.
대화 본문 대신 이벤트 이름·종류만 보고서에 남긴다. Codex 신뢰 검토를 건너뛰는 옵션은 없다.
Kotlin/JVM 출력 대상은 17이다. 필요한 JDK는 [개발 안내](DEVELOPMENT.md)를 따른다.

Debug 데스크톱 패키지에서 `DGL_CANDIDATE_DIR`를 임시 디렉터리로, `DGL_NOTIFICATION_PROBE=1`로 실행하면
권한 요청 후 20초 뒤 합성 알림 1건을 보낸다. 클릭 시 대상 종료 안내가 나오고 SSH에 연결하지 않아야 한다.
이 probe는 Release 바이너리에 포함되지 않는다. OS 표시·클릭 확인이 끝나면 테스트 앱을 종료한다.

공식 근거: [Codex Hooks](https://learn.chatgpt.com/docs/hooks),
[Claude Hooks](https://code.claude.com/docs/en/hooks),
[Android 알림 권한](https://developer.android.com/develop/ui/compose/notifications/notification-permission),
[Apple 알림 권한](https://developer.apple.com/documentation/usernotifications/asking-permission-to-use-notifications),
[Windows 데스크톱 알림 활성화](https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-desktop-cpp-wrl).

창 상태 버튼은 해당 창의 모든 pane 관측과 미확인 이벤트를 집계한다. 합쳐진 단일 창 세션 행도 같은 필터를 쓴다. 상세의 이동 버튼은 관측의 실제 pane을 선택한다.
