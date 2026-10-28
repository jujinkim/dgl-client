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

- SSH read: 16 KiB, 연결 전체 출력 payload: 256 KiB, 입력 대기: 최대 32×64 KiB, scrollback: 기본 5,000줄.
- 캡처의 행별 커서 이동·내용·일반/alternate 버퍼·모드·미완성 시퀀스는 순서를 유지해 최대 16 KiB씩 묶는다. 추가 임시 버퍼도 16 KiB이며 WebView는 묶음별 xterm 완료 ack를 기다린다. 읽기와 렌더링 사이에 무제한 큐를 두지 않는다.
- tmux `pause-after=2`가 알리는 지연은 화면 재캡처로 회복한다. 로컬 기록 전체를 보존한다고 주장하지 않는다.
- renderer surface는 연결보다 짧게 살 수 있다. Android는 탭별 WebView를 유지하며 다시 붙일 때 새 출력 구독에 재캡처한다. 원격 프로세스를 종료하지 않는다.
- Android viewport 변경은 첫 요청을 즉시 제출하고 이후 최소 50ms 간격으로 전송 중 요청 하나와 최신 대기 크기 하나만 유지한다. 실패 후 재시도는 150ms 간격이다. Rust는 같은 요청 크기를 무시하고, tmux 크기 변경 후 목록을 갱신해 실제 패널 크기가 달라질 때만 캡처한다. 다른 클라이언트가 크기를 제한하면 실제 pane 크기를 유지하며, 변경이 없으면 추가 Resize 이벤트도 보내지 않는다.
- 크기 조정 복원은 `resizing` 표시로 연결 복구 UI와 구분한다. 크기만 바뀌는 `Restoring`은 기존 제한 큐로 키·텍스트를 받고 좌표 입력은 차단한다. 연결 복구·대상 전환의 입력 차단과 세대 검사는 유지한다. 캡처에서 확인한 실제 크기를 목록의 비교 기준에도 반영하여 후속 레이아웃 알림의 중복 캡처를 막는다.
- Android 연결 탭 재부착·tmux 창 전환은 불투명 로딩 표시 뒤에서 복원한다. 코어 `Ready`, 재부착 snapshot 요청 완료, 모든 보이는 pane의 현재 target 렌더 ACK를 확인한 뒤 창 전체를 한 번에 공개한다. 이전 화면을 최신 화면처럼 표시하지 않으며 입력도 그때까지 차단한다. 연결·구독·창 전환 세대가 다른 ACK는 공개 조건을 만족하지 못한다. 오류 시에는 이전 화면 대신 실패·재시도 안내를 유지한다.
- Android의 첫 캡처를 포함한 `ResetEnd`는 캐시 유무와 무관하게 xterm 렌더 이벤트와 paint 기회를 확인한다. 일반 resize·화면 동기화는 DOM 화면 한 장을 복제해 캡처 조각을 가린다. 재접속 복구에서만 기본 90ms fade를 적용하며, 선택형 `smoothTerminalTransitions`(기본 false)은 120ms 크기 보간·크로스페이드·2px 블러를 추가한다. 캐시는 입력·접근성 대상에서 제외하며 대상 변경·실패·해제와 15초 제한에서 정리한다. 입력 버퍼·스크롤백을 복제하지 않으며 기존 출력 ACK·크기 제한을 유지한다.
- 현재 pen 속성·모든 VT 내부 상태는 tmux format/capture만으로 완전히 복원할 수 없다. 완전한 emulator serialization이 아니다. 색상 화면·커서·alternate 전환과 일반 출력은 테스트하고 미확인 상태는 명시한다.
- Copy Mode 알림은 GUI 상태로 표시한다. `%output`을 Copy Mode UI로 간주하지 않는다.

## 확인한 근거

- [Control Mode 프로토콜](https://github.com/tmux/tmux/wiki/Control-Mode)
- [tmux 3.3a의 pause/continue와 출력 순서](https://github.com/tmux/tmux/blob/3.3a/control.c)
- [tmux 명령 파싱·명령 목록 오류 규칙](https://github.com/tmux/tmux/blob/3.3a/tmux.1)
- 실제 SSH 테스트: 연속 번호 출력 중 10회 snapshot 이후 행 순서·중복·누락 비교, 두 클라이언트 동시 연결, 외부 split·rename·삭제.

현재 pane별 렌더링 세대와 입력 포커스의 분리·로컬 기록 유지 계약은 [TMUX_WINDOW_SYNC.md](../TMUX_WINDOW_SYNC.md)를 따릅니다.
