# 터미널 마우스·조합 키 입력

## 동작

데스크톱(Mac·Windows 공통 Rust 코드)은 다음 입력을 처리한다.

| 입력 | 동작 |
| --- | --- |
| 휠·트랙패드 | 이동량만큼 원격 휠 이벤트를 보내며, 작은 픽셀 이동은 누적한다. 0 이동은 보내지 않는다. 마우스 모드가 꺼져 있으면 로컬 기록을 스크롤한다. |
| 가로 휠 | 원격 마우스 모드에서 좌우 휠 이벤트를 보낸다. |
| 클릭·드래그·호버 | 왼쪽·가운데·오른쪽 버튼을 지원한다. 원격 앱이 요청한 1000(클릭), 1002(버튼 드래그), 1003(호버 포함) 모드를 구분한다. 같은 셀의 반복 이동은 보내지 않는다. |
| 마우스 인코딩 | 기본 X11, UTF-8(1005), SGR(1006)을 지원한다. 기본 X11 좌표 범위를 넘는 이벤트는 보내지 않는다. |
| Shift+마우스 | 원격 마우스 모드에서도 로컬 텍스트 선택·기록 스크롤을 사용한다. 원격으로 누른 버튼은 영역 밖이나 Shift를 누른 상태에서도 해제한다. |
| Ctrl/Alt+마우스 | 클릭·드래그·호버·휠에 modifier 정보를 포함한다. |
| Shift/Ctrl/Alt+방향키·Home/End·Insert/Delete·PageUp/PageDown·F1–F12 | 조합 정보를 xterm 시퀀스에 포함한다. 여러 modifier를 동시에 누를 수 있다. |
| Ctrl+문자·Alt+문자 | Ctrl+A–Z, Ctrl+Space 및 제어 문자 별칭을 처리한다. Ctrl+Alt 조합은 ESC와 제어 바이트를 함께 보낸다. |
| Shift+Tab·Alt+Enter·Alt+Backspace | 각각 역방향 Tab, ESC+Enter, ESC+Backspace를 보낸다. |
| 일반 문자·IME | GPUI 입력기 경로에서 입력을 받는다. Shift로 만드는 대문자·기호는 이 경로를 사용한다. |

호버는 원격 앱이 1003 모드를 켠 경우에 전달된다. 단순 셸 프롬프트에서 포인터를 움직이는 것만으로 입력을 보내지 않는다. 대체 화면에서 마우스 모드는 꺼져 있고 1007 모드가 켜져 있으면 세로 휠을 위·아래 방향키로 보낸다.

macOS의 Command 앱 단축키와 Windows에서 실제로 지정한 Ctrl+Shift 앱 단축키는 앱 조작에 사용한다. Windows에서 지정하지 않은 Ctrl+Shift+방향키 등의 조합은 터미널로 전달한다.

Android는 xterm.js의 키보드·마우스 처리를 사용한다. `onData`는 UTF-8 텍스트로, `onBinary`는 문자당 원래 1바이트로 전달하여 기본 마우스 보고의 128–255 바이트를 보존한다.

이번 범위는 전통적인 xterm 입력이다. 데스크톱의 Kitty 키보드 프로토콜, modifyOtherKeys, 키 해제 이벤트, 픽셀 단위 마우스 보고(1016), 링크 호버 UI까지 지원한다는 의미는 아니다. OS가 먼저 사용하는 단축키와 실제 키보드 배열·IME 동작은 기기별 확인이 필요하다.

프로토콜 기준: [xterm 제어 시퀀스](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html), [xterm.js onBinary](https://xtermjs.org/docs/api/terminal/classes/terminal/#onbinary).

## 검증 (2026-10-25)

- `cargo test -p dgl-desktop --no-fail-fast`: 18개 통과, 기존 Keychain 실환경 테스트 1개 ignored. GPUI 이벤트 주입으로 조합 키, IME 커밋 경로, 모드별 클릭·드래그·호버, 영역 밖 버튼 해제, 휠 이동량·소수 누적·방향·Shift 우회를 확인했다.
- `npm test --prefix web`: 브리지 테스트 1개 통과. 텍스트·바이너리 채널의 분리와 입력 크기 제한을 확인했다. 실제 WebView 이벤트 테스트는 아니다.
- `:app:desktopTest --tests kim.jujin.rovessh.core.TerminalInputTest`: 5개 통과. 바이너리 0–255 값 보존과 잘못된 입력 거부를 포함한다.
- `:app:compileAndroidMain`: 통과. 기존 deprecated API 등의 경고는 남아 있다.
- 데스크톱 `clippy --all-targets --no-deps -- -D warnings`, 수정한 Rust 파일의 `rustfmt --check`, `git diff --check`, 대시보드 메타데이터 검증: 통과. 의존 패키지의 향후 Rust 호환성 안내는 남아 있다.
- 의존성을 포함한 Clippy는 기존 변경 사항인 `crates/dgl-core/src/connection.rs:1062`의 `unnecessary_unwrap` 경고로 실패했다. 이번 입력 수정 범위 밖이므로 해당 코드는 변경하지 않았다.

이번에는 실제 장치의 마우스·키보드 조작, SSH 원격 TUI 상호작용, Android WebView 계측, Windows 네이티브 빌드·GUI 검증을 실행하지 않았다. Windows 빌드·패키징 경로는 유지한다.
