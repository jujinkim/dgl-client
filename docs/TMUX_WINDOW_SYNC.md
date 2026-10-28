# 실제 tmux 창과 GUI 동기화

GUI와 일반 SSH 클라이언트는 같은 세션·창·pane을 사용한다. 세션 복제나 프로세스 재실행 없이
GUI 종료는 SSH/Control Mode 연결만 닫는다. 재접속은 서버의 현재 창·활성 pane을 따르며,
저장된 오래된 lastPane으로 외부 클라이언트의 선택을 되돌리지 않는다.

## 배치와 크기

`list-windows`의 width/height, window_layout/window_visible_layout, zoomed flag와
`list-panes`의 left/top/width/height를 원격 모델에 담는다.
데스크톱은 창 컨테이너 아래 pane별 alacritty_terminal Entity를,
Android는 WebView 하나 안에 pane별 xterm을 유지한다. 확대 시 활성 pane만 표시한다.
화면에 맞춰 pane별 grid를 fit하지 않고 실제 문자 셀 크기로 배치하며 부족한 영역은 스크롤한다.

연결당 창 컨테이너 하나만 `refresh-client -C`를 요청한다. 두 플랫폼 모두 첫 변경을 즉시 제출하고
이후 최소 50ms 간격으로 최신 크기를 합치며 한 요청만 진행한다. 실패 후 재시도는 150ms 간격이다.
서버 크기를 받은 결과로 viewport 요청을 다시 만들지 않는다.
다른 클라이언트가 크기를 제한하면 tmux의 기존 window-size 정책과 서버 확정 크기를 따른다.
GUI의 픽셀 드래그는 가장 가까운 문자 셀 수로 반올림하며 셀 경계를 넘지 않은 이동은 요청하지 않는다.
경계 드래그는 셀 단위 `resize-pane -x/-y` 요청이며 응답 후 서버 배치를 표시한다.

## 출력과 입력

`State.window`가 현재 창, `State.pane`과 기존 `PaneTarget`이 현재 입력 대상이다.
`State.render_targets[pane]`와 출력 이벤트의 target은 입력 포커스와 독립적인 렌더링 식별자다.
포커스 이동은 입력 세대만 바꾸며 이미 보이는 pane의 렌더링 식별자를 바꾸지 않는다.
모든 보이는 pane의 라이브 출력을 하나의 연결 구독으로 전달한다.

신규·실제 크기/모드 변경·연결 복구·새 surface·pause-after 복원이 필요한 pane만
pause → 모드/alternate/main/미완성 시퀀스 캡처 → continue 경계로 복원한다.
다른 pane은 그동안 라이브 출력을 이어간다. 이름 변경이나 단순 클릭은 캡처하지 않는다.
삭제된 pane의 렌더러는 해제하고 숨겨진 기존 pane의 로컬 에뮬레이터는 유지한다.
재표시 시 놓친 원격 출력을 캡처한다. 같은 pane 재캡처는 로컬 primary 기록을 유지하며,
로컬 스크롤 위치·텍스트 선택은 기기별 상태다. 서버 히스토리를 지우는 명령은 보내지 않는다.

연결 전체 출력 예산 256 KiB, 16 KiB 배치, 미확인 이벤트 256개와 소비 후 ACK를 유지한다.
캡처의 행별 커서 이동과 내용은 일반·alternate 버퍼, 모드·미완성 시퀀스까지 순서대로 16 KiB씩 묶는다.
Android 탭 재부착·tmux 창 전환은 이전 화면을 노출하지 않고 로딩 표시 뒤에서 처리한다.
현재 창의 모든 보이는 pane의 최신 target 렌더 ACK와 snapshot 요청·코어 복원이 완료되면 창 전체를 공개한다.
Rust·UniFFI·Kotlin은 오래된 연결/렌더링 세대의 출력을 적용하지 않고 ACK로 예산을 반환한다.
GUI 입력은 캡처한 대상 pane과 연결/포커스 세대를 제출·실행 시 검사하며 자동 재전송하지 않는다.
크기 세대만 바뀐 경우 같은 pane의 입력 목적지는 유지한다. 크기만 바뀌는 복원 중 키·텍스트 입력은
기존 제한된 큐로 받아 순서대로 처리한다. 실제 연결 확인·복구·대상 변경은 계속 차단하며,
좌표 입력은 크기 세대까지 검사한다.
Android bridge는 출력 pane과 입력 target을 따로 전달하고, 비활성/오래된 DOM 입력을 거절한다.

## 범위와 한계

GUI 조작은 기존 버튼·메뉴를 사용한다. 사용자 정의 tmux prefix/키 바인딩과 tmux 상태줄·팝업·
copy-mode 화면 재현은 범위 밖이다. 외부 일반 SSH 클라이언트에서는 tmux 기능을 그대로 사용한다.
format/capture만으로 VT의 모든 내부 상태를 직렬화할 수는 없다. 펜 속성 등 기존 복원 한계는
[ADR 002](decisions/002-terminal-restoration.md)에 기록한다.
Windows MSVC x64 빌드·ZIP·수동 CI 경로는 유지하며 실제 Windows 검증은 사용자 담당이다.

근거: [tmux 3.3a 매뉴얼](https://github.com/tmux/tmux/blob/3.3a/tmux.1)의 Control Mode,
window/pane formats, refresh-client와 resize-pane. 검사 결과는 [VALIDATION.md](VALIDATION.md)를 따른다.
