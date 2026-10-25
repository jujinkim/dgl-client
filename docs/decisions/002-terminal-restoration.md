# ADR 002 — Control Mode, 캡처 경계와 렌더링

상태: tmux 3.3a·3.7c에서 구현·통합 검증.

## 프로토콜

SSH exec 채널에서 PTY 없이 `tmux -C attach-session -t '$ID'`를 연다. `-CC`의 tty 제어와 일반 셸 출력을 섞지 않는다. 다른 클라이언트를 떼어내는 `attach -d`를 사용하지 않는다.

바이트 기준 LF 프레이밍 이후 메시지 종류를 판별한다. UTF-8 문자가 여러 `%output`에 걸쳐도 바이트를 보존한다. 응답 본문은 같은 timestamp/number/flags의 `%end`/`%error`만 닫는다. 본문 안의 `%output` 같은 텍스트는 알림이 아니다. `%extended-output`, octal escape, 부분 읽기, 알 수 없는 알림을 처리한다. 1 MiB line / 4 MiB response 상한을 넘거나 EOF에서 프레임이 잘리면 중단한다.

라이브 `%output`의 `\ooo`와 `capture-pane -C`의 `\\` escaping은 다르다. 실제 서버 테스트로 차이를 확인해 별도 디코딩한다. 상태 행은 `q:`로 인용된 필드를 escape-aware `|` 구분자로 읽는다. SSH ForceCommand 환경에서 제어 문자가 변형될 수 있어 구분자에 literal tab을 사용하지 않는다.

명령은 검증된 숫자 ID와 인용된 이름으로만 구성한다. 터미널 입력은 256바이트씩 `send-keys -H`에 hex로 보낸다. 이름의 `#`는 `##`로 인코딩해 tmux의 format expansion(`#{...}`, `#(...)`)을 막는다. 최초 세션 생성의 shell argv 경로는 trailing semicolon의 tmux 재해석도 별도로 escape한다.

## 화면 복원

다음 **동기 명령 목록을 한 번의 write로** 전송한다.

1. `refresh-client -A %pane:pause`
2. `capture-pane -p -e -C -N`
3. 커서·alternate·주요 입력 모드·scroll region·pane 크기 조회
4. `capture-pane -p -P -C` (미완성 입력 시퀀스)
5. `capture-pane -p -a -q -e -C -N` (alternate 진입 전 화면)
6. `refresh-client -A %pane:continue`

tmux continue는 현재 offset으로 이동한다. 캡처와 재개를 여러 SSH round trip으로 나누면 사이의 출력이 누락된다. 명령 목록에 `run-shell` 같은 비동기 명령을 넣지 않는다. 응답을 처리하는 reader에서 pane 크기와 화면을 복원한 뒤 다음 라이브 이벤트를 전달한다. 명령 목록 중 실패하면 후속 명령이 실행되지 않는 tmux 규칙도 응답 처리에 반영한다.

connection/pane/size generation이 달라진 응답은 화면에 적용하지 않는다. 입력은 복원 후에만 허용하며 이전 세대의 대기 입력은 버린다. 삭제된 세션은 자동 대체하지 않는다. pane의 실제 열/행을 렌더러에 전달해 분할 pane의 줄바꿈 폭을 유지한다.

## 흐름과 한계

- SSH read: 16 KiB, JediTerm pipe: 256 KiB, input: 최대 32×64 KiB, scrollback: 기본 5,000줄.
- WebView output은 16 KiB write의 xterm 완료 ack를 기다린다. 읽기와 렌더링 사이에 무제한 큐를 두지 않는다.
- tmux `pause-after=2`가 알리는 지연은 화면 재캡처로 회복한다. 로컬 기록 전체를 보존한다고 주장하지 않는다.
- renderer surface는 연결보다 짧게 살 수 있다. 연결 탭을 바꾸면 새 surface에 재캡처하며 원격 프로세스를 종료하지 않는다.
- Android viewport 변경은 마지막 변경부터 150ms 동안 묶으며 전송 중 요청 하나와 최신 대기 크기 하나만 유지한다. Rust는 같은 요청 크기를 무시하고, tmux 크기 변경 후 목록을 갱신해 실제 패널 크기가 달라질 때만 캡처한다. 다른 클라이언트가 크기를 제한하면 실제 크기만 렌더러에 다시 전달한다.
- 크기 조정 복원은 `resizing` 표시로 연결 복구 UI와 구분하지만 내부 `Restoring` 입력 차단과 세대 검사는 유지한다. 캡처에서 확인한 실제 크기를 목록의 비교 기준에도 반영하여 후속 레이아웃 알림의 중복 캡처를 막는다.
- Android는 번들 xterm DOM 렌더러의 보이는 화면 한 장만 복제해 캡처 조각을 가린다. `ResetEnd`에서 xterm 렌더 이벤트와 paint 기회를 확인한 뒤 180ms fade로 제거한다. 선택형 `smoothTerminalTransitions`(기본 false)은 240ms 크기 보간·크로스페이드·2px 블러를 추가한다. 캐시는 입력·접근성 대상에서 제외하며 대상 변경·실패·해제와 15초 제한에서 정리한다. 입력 버퍼·스크롤백을 복제하지 않으며 기존 출력 ACK·크기 제한을 유지한다.
- 현재 pen 속성·모든 VT 내부 상태는 tmux format/capture만으로 완전히 복원할 수 없다. 완전한 emulator serialization이 아니다. 색상 화면·커서·alternate 전환과 일반 출력은 테스트하고 미확인 상태는 명시한다.
- Copy Mode 알림은 GUI 상태로 표시한다. `%output`을 Copy Mode UI로 간주하지 않는다.

## 확인한 근거

- [Control Mode 프로토콜](https://github.com/tmux/tmux/wiki/Control-Mode)
- [tmux 3.3a의 pause/continue와 출력 순서](https://github.com/tmux/tmux/blob/3.3a/control.c)
- [tmux 명령 파싱·명령 목록 오류 규칙](https://github.com/tmux/tmux/blob/3.3a/tmux.1)
- 실제 SSH 테스트: 연속 번호 출력 중 10회 snapshot 이후 행 순서·중복·누락 비교, 두 클라이언트 동시 연결, 외부 split·rename·삭제.

### 2026-10-25 Android 크기 변경·캐시 전환 검증

- Rust 코어/바인딩 19개, Kotlin 39개(환경 필요 9개 제외), 웹 6개 통과. Kotlin 기존 설정의 기본값과 새 전환 설정 저장·복원, 마지막 변경 기준 150ms 통합과 연결별 요청 분리를 확인했다.
- API 36 ARM64 에뮬레이터 8개 통과: 실제 WebView 캐시·그라디언트·크기/블러 전환·정리, 키보드 반복 표시 전후 tmux 클라이언트 PID와 WebView 유지, 설정 변경 시 WebView 유지, 회전·복귀. Android 34/35 및 실제 기기는 이번 수정에서 재검증하지 않았다.
- tmux 3.7c의 크기 변경·독립 연결, 실제 연결 단절/복구, 제목 갱신 시나리오 통과. 3.3a도 각 시나리오는 새 SSH/tmux fixture에서 통과했지만, 같은 fixture를 공유한 일괄 실행은 세션 생성/초기화 단계의 `InvalidArgument` 실패가 남아 있어 일괄 통과로 간주하지 않는다.
- Android Debug 빌드·lint와 네 ABI/서명 확인 통과. Windows 네이티브 빌드·실기는 미실시.
