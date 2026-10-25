# flat UX와 데스크톱 탭 그룹

2026-10-25 세션·창·패널 제목과 보조 정보 분리는 [SESSION_HIERARCHY.md](SESSION_HIERARCHY.md)를 따른다.

2026-10-24, `main`에서 구현했다. Android와 Windows·Mac 공통 GPUI 코드에 적용하며 기존 설정·호스트 ID·플랫폼별 자격 증명 저장소와 Windows 빌드·패키징 진입점을 유지한다.

## 조작과 상태

- 회색 바탕·파란 강조색의 평평한 목록과 얇은 구분선을 사용한다. 시스템·밝게·어둡게 테마는 유지하며 데스크톱 입력창·선택·포커스도 테마를 따른다.
- 데스크톱 왼쪽 탐색기는 호스트와 활성 그룹의 세션 → 창 → 패널 트리를 함께 보여준다. Android는 호스트 목록과 tmux 관리 시트를 유지한다. Android 터치 항목은 최소 48dp를 확보한다.
- 펼침 화살표는 탐색만 하고 행 이름은 대상 선택을 수행한다. 헤더 `+`와 행 `…` 메뉴에 생성·이름 변경·삭제를 배치한다. 삭제 확인은 유지한다.
- `↻`는 세션 새로고침이며 완료 요청 ID로 중복 요청을 차단한다. Android는 `세션 새로고침` 접근성 설명과 툴팁, GPUI는 같은 컨트롤 ID와 툴팁을 사용한다.
- tmux 도구 모음은 각 터미널 상단에 별도로 둔다. `패널 분할`, `이동`, `크기 조절`, `닫기`로 묶으며, 방향별 이동은 원격 선택 패널과 입력 대상을 함께 바꾸고 크기 조절은 3셀 단위다. 닫기는 기존 확인창을 거친다. 데스크톱의 각 도구 모음은 해당 그룹의 선택 탭에 적용하며 클릭 시 최신 연결 상태·패널을 확인한다. Android 도구는 가로 스크롤, 닫기 버튼은 오른쪽 고정이며 터치 영역은 48dp다. 탭 바의 `화면 분할`은 앱 그룹, 터미널 위 `패널 분할`은 원격 tmux 작업이다.
- 데스크톱 각 그룹은 독립된 탭 순서와 선택 탭을 가진다. 탭 바 오른쪽 끝의 `↔️ / ↕️` 버튼으로 만든 분할은 빈 그룹이며 새 연결은 활성 그룹에 열린다. 버튼은 탭 목록 스크롤 영역 밖에 고정하고 기능 이름을 툴팁으로 표시한다. 동일 호스트 연결도 별도 SSH 핸들을 사용한다.
- 탭 제목을 탭 바에 드롭하면 순서를, 다른 그룹 중앙에 드롭하면 소속 그룹을 바꾼다. 그룹 본문의 네 가장자리에 드롭하면 해당 방향에 새 그룹을 만들고 탭을 옮긴다. 드래그 중 실제로 분할 가능한 방향에만 반투명 예상 배치를 표시한다. 자기 그룹의 유일한 탭, 빈 대상 그룹, 최소 공간 부족·최대 그룹 수 도달 시에는 분할하지 않는다. Esc 또는 그룹 밖 드롭은 배치를 바꾸지 않는다. `…` 메뉴에서도 순서·소속 그룹을 바꾼다. 연결과 터미널 Entity를 재생성하지 않는다. 같은 패널의 크기 변경 snapshot은 로컬 스크롤 기록과 위치를 유지한다.
- 마지막 탭을 닫아도 그룹은 유지한다. `그룹 합치기`는 분할 트리의 인접 그룹에 탭을 옮긴 뒤 그룹을 제거한다. 닫기는 해당 연결만 종료한다.
- 경계를 드래그하여 비율을 조절한다. 그룹의 최소 분할 공간은 280×180 논리 픽셀이며 최대 16개 그룹을 지원한다. 창이 작아지면 기존 그룹은 축소 표시하고 추가 분할을 막는다.
- 키보드의 기존 연결 단축키와 하단 단축키 바는 활성 그룹의 선택 탭에 적용한다. 각 그룹의 보이는 터미널이 자체 크기를 요청한다. 크기 요청은 연결당 한 건만 진행하고 대기 중 변경은 최신 크기로 병합한다.

## 모달과 저장

호스트 입력창에는 비밀번호·키 암호의 `비밀번호 표시` 체크가 있다. 표시 상태는 저장하지 않고 새 입력창에서 숨김으로 시작한다. 데스크톱 개인키 경로의 빈 입력에는 macOS `$HOME/.ssh/`, Windows `%USERPROFILE%\.ssh\`의 실제 사용자 경로를 플레이스홀더로 보여준다.

대화상자는 공통 포커스 링과 마우스 차단 레이어를 사용한다. 첫 입력 항목으로 이동하며 Tab·Shift+Tab은 보이는 입력과 버튼을 순환한다. 입력 없는 확인/설정 창은 버튼에서 시작한다. 버튼은 Enter/Space, 이름 입력은 Enter로 실행하며 IME 조합 중 Enter는 제출하지 않는다. Esc와 닫기는 이전 포커스를 복원한다. 배경 터미널은 모달 동안 입력·마우스·스크롤을 차단하며 일반 터미널의 Tab 전송은 유지한다.

설정 디렉터리는 `desktop/src/platform.rs`의 기존 경로를 사용한다. 새 `workspace-layout.json`에는 `Group { id }`와 재귀 `Split { axis, ratio, first, second }`만 기록한다. 호스트·탭·자격 증명·선택 상태는 저장하지 않는다. 재실행 시 모든 그룹은 비고 연결은 사용자가 다시 연다. 잘못된 JSON, 중복 그룹 ID, 범위 밖 비율, 과대한 파일/깊이는 빈 단일 그룹으로 대체한다. 기존 `settings.json`은 그대로 읽는다.

코어 인터페이스는 [CORE_API.md](migration/CORE_API.md)의 `selectTarget`·`focusPane` 확장을 따른다. 방향 이동은 [tmux 3.3a의 select-pane 구현](https://raw.githubusercontent.com/tmux/tmux/3.3a/cmd-select-pane.c)의 방향 플래그를 사용한다. 이동 뒤 실제 활성 패널을 조회하고 화면·PaneTarget·패널 세대를 갱신한다. 다른 연결의 표시 패널은 유지한다.

## 실행 결과

| 검사 | 결과 |
|---|---|
| Rust workspace 단위·계약·GPUI 검사 | 17 통과, 3 ignored. 120개 공통 fixture 포함. ignored는 별도 실행하는 SSH 두 검사와 interactive Keychain 한 검사 |
| GPUI 자동화 | 초기 포커스, Tab/Shift+Tab 왕복·순환, 버튼 Enter, Esc 복원, IME Enter와 한글 뒤 조합 선택 위치, 한글·emoji 비밀번호 클릭/입력, 가려진 호스트 버튼·터미널 입력 차단 통과 |
| 그룹 자동화 | 중첩 분할, 실제 GPUI 탭·경계 드래그와 비율 저장, Entity 유지, 이동·닫기·합치기, 크기 요청 병합, snapshot 스크롤 기록 유지, 배치 저장·빈 복원·불량 파일 대체 통과 |
| 격리 OpenSSH + tmux 3.3a | 2개 SSH 통합 검사 통과 |
| 격리 OpenSSH + tmux 3.7c | 동일 2개 SSH 통합 검사 통과 |
| SSH 검사 범위 | 네 방향 이동 뒤 실제 패널·입력·snapshot 대상 일치, 다른 세션 패널 선택, 동일 호스트 독립 연결, 크기 전달, 기존 인증·복구·삭제 회귀 |
| JVM 회귀 | 36개 중 27 통과, 9 예상 skip; 격리 SSH 환경 없는 기본 실행 |
| Android APK·lint | debug, unsigned release, instrumentation APK 빌드 통과. lint 오류 0, 경고 13 |
| Android API 36 | 전용 AVD와 격리 tmux 3.7c에서 7개 계측 검사 통과. 트리 펼침과 선택 분리, 포커스 이동, 새로고침, 메뉴 창 생성, 시트 복귀, IME 위 단축키·저장 버튼, 테마, 회전·복귀 포함 |
| Mac 패키지 | ARM64 debug .app 생성, 로컬 ad-hoc 서명 검증 통과 |
| Windows | MSVC x64 스크립트·ZIP 패키징·수동 CI 유지. 이번 작업에서 네이티브 빌드·DPAPI·GUI 실행하지 않음 |

비밀번호 표시·SSH 폴더 플레이스홀더 후속 변경은 기존 데스크톱 검사 8개 통과와 Android debug 빌드·lint로 확인했다. Android 계측 7개 및 화면 기록은 이 후속 변경 전 결과다.

Android 화면은 `artifacts/rust-candidate/android/rust-light.png`, `rust-dark.png`, `rust-editor.png`와 펼쳐진 트리의 `rust-tree.png`를 확인했다. 계측 원문은 같은 디렉터리의 `api36-instrumentation.txt`에 있다. APK 전달용 복사본은 `dgl-rust-debug.apk`이며 v2 debug 서명을 확인했다. Mac 출력은 `artifacts/rust-candidate/mac/dgl Rust Candidate.app`이다.

검사 중 발견하여 수정한 오류:

- 초기 목록 조회에서 마지막 세션 삭제 직후 빈 세션 목록에 대해 하위 조회를 실행하던 오류. 세션이 없으면 빈 트리를 반환한다.
- 단일 행 GPUI 입력창에 IME Enter의 줄바꿈이 삽입되던 오류. 조합 제출과 줄바꿈을 분리했다.
- 비밀번호의 마스킹 문자 위치를 UTF-8 바이트 위치로 사용하던 오류. 한글·emoji를 클릭/선택한 뒤 입력해도 문자 경계를 유지하며 입력기 좌표 조회도 마스킹/placeholder를 처리한다.
- Android 트리가 길어지면 부분 확장 시트 아래로 복귀 버튼이 가려지던 문제. 시트를 완전히 열고 트리만 남은 높이 안에서 스크롤하도록 제한했다.
- 탭 이동/크기 변경 snapshot이 로컬 기록을 초기화하던 문제. 같은 패널이면 primary grid의 기록과 스크롤 위치를 보존한다.

최종 재검사 중 한 번은 에뮬레이터 System UI ANR 창이 앱을 가려 7개 중 3개가 실패했다. `ux-system-ui-failure.png`와 `ux-system-ui-failure.txt`로 보존했다. 앱 연결은 Ready였지만 접근성 활성 패키지는 android였다. 빌드 종료 뒤 전용 AVD를 다시 시작했다. 다음 실행에서는 새 tmux UI 검사가 통과했지만 기존 볼륨 키 회귀 검사 한 건이 시트 종료 후 입력 소비 assertion에서 실패했다(`ux-volume-focus-failure.txt`). 테스트의 고정 지연 가정을 실제 Activity 포커스·resumed 조건 대기로 교체하고 진단 값을 추가했다. 대기 실패에 화면 이름·접근성 트리·스크린샷 진단을 추가했다. 개별 볼륨 키 검사와 전체 7개 검사가 재실행에서 통과했다. 테스트 timeout은 늘리지 않았다. 이후 트리 화면 기록을 추가한 전체 재검사도 7개 모두 통과했다.

## 남은 인수 범위

Mac의 짧은 실제 화면 확인은 임시 `DGL_CANDIDATE_DIR`로 시도했지만 Computer Use가 앱 표시명과 bundle ID 모두에서 `cgWindowNotFound`를 반환했다. 임시 앱 프로세스를 종료했으며 실제 Mac 화면·실제 한국어/일본어 IME·Keychain 인수를 통과 처리하지 않는다. GPUI TestPlatform 자동화는 네이티브 화면 검사를 대체하지 않는다. Windows 실기 확인은 사용자 담당이며 [Windows 안내](WINDOWS.md)를 따른다. 이번 Android 실행은 API 36 에뮬레이터에 한정한다.

GPUI 0.2.2는 앱이 사용할 OS 접근성 이름/트리 API를 제공하지 않는다. 따라서 데스크톱 `↻`의 OS 스크린리더 노출은 구현·검증 완료로 표시하지 않는다. 현재 같은 이름의 툴팁과 컨트롤 ID까지 제공하며, OS 접근성 브리지 지원은 별도 과제다. Android의 접근성 설명은 실제 계측에서 사용했다.

## CLI 실행 메뉴 후속 검증 — 2026-10-24

Mac·Windows 공통 GPUI와 Android Compose의 단축키 바 오른쪽에 고정 `…`를 추가했다.
위로 열리는 두 행은 Claude Code / Codex 각각 일반 실행 60%, 번개 Bypass 40%로 나뉜다.
명령과 CR을 기존 입력 경로로 한 번 보내며 별도 확인이나 CLI 설치·로그인을 수행하지 않는다.
연결·탭·그룹·패널 변경과 입력 불가 상태를 실행 직전 재검사한다.
상태 변경, 바깥 클릭, Esc/Android 뒤로가기로 닫히며 실행 후 터미널 포커스를 복원한다.
공개 연결 API·저장 형식은 바꾸지 않았다.

- Rust 데스크톱: 10 통과, interactive Keychain 1 ignored. 네 명령의 정확한 CR·1회 처리,
  대상·연결/패널 세대 변경 차단, Shell/Ready 허용, 복구·종료 차단을 검사했다.
  GPUI TestPlatform에서 640×480 창의 고정 버튼, 위쪽 메뉴, 6:4 비율,
  비활성 항목 무전송, Esc·바깥 클릭·탭 변경 닫기와 포커스 복귀를 검사했다.
  데스크톱 all-targets Clippy `-D warnings`와 Rust fmt 검사도 통과했다.
- JVM: 38개 중 29 통과, 외부 SSH·Windows 전용 9 예상 skip.
  두 연결 사이의 대상 선택, 네 명령과 CR, 중복 실행·상태/패널 변경 차단을 검사했다.
- Android API 36: 새 실행 메뉴 계측 1개와 기존 볼륨 키·WebView/IME 회귀 2개 통과.
  실제 ClientConnection 입력 경로를 테스트 SSH transport로 받아 네 명령의 단일 전송과
  다른 연결 무전송, 실행 후 WebView/xterm 포커스를 확인했다. 두 테마의 아이콘,
  메뉴 위치와 비율, Esc·Back·바깥 터치, 복구/종료 중 비활성화·패널/연결 변경 닫기를 확인했다.
  처음 바깥 클릭 검사는 source가 없는 합성 MotionEvent로 실패했다. 실제 터치스크린
  source와 앱 내부 좌표를 지정한 재실행에서 통과했다. 제품의 Popup 동작은 변경하지 않았다.
- Android debug·계측 APK 빌드, lint 오류 0/기존 검토 경고 13, 네 ABI·Rust 엔진·v2 서명 검사 통과.
  Mac debug .app 생성과 로컬 ad-hoc 서명 검사, Windows ZIP 패키징 단위 검사 3개 통과.
- README·아이콘 출처/CC0 라이선스와 양쪽 데스크톱 패키지 라이선스를 갱신했다.
  대시보드 메타데이터 검증 및 Git 제외 유지 확인을 완료했다.

새 Android 화면·계측 원문은 `artifacts/rust-candidate/android/cli-launcher-{light,dark}.png`,
`cli-launcher-api36.txt`, `cli-launcher-regression-api36.txt`에 보존했다.
이번 세션의 Computer Use는 `CUA_REPL_ENABLED_SURFACES is required`로 사용할 수 없어
Mac 네이티브 메뉴 화면 검사를 완료하지 않았다. 임시 앱 프로세스는 종료했다.
Windows 네이티브 빌드·실기 검증 및 실제 Claude/Codex CLI 실행은 이번 검증에 포함하지 않는다.

## 탭 드래그 분할 후속 검증 — 2026-10-25

- 각 그룹의 탭 바 오른쪽에 `↔️ / ↕️` 분할 버튼과 `🔗` 합치기 버튼을 고정했다.
- 네 방향 드래그 분할, 그룹 중앙 이동, 탭 바 순서 변경, 반투명 예상 배치와 실제 그룹 좌표 일치,
  Esc·바깥 드롭 취소, 좁은 창의 버튼 위치·분할 차단을 GPUI TestPlatform으로 검사했다.
- 새 그룹에서도 기존 터미널 Entity를 유지하고 드래그·Esc를 원격 터미널 입력으로 보내지 않는 것을 확인했다.
- Rust 데스크톱 검사 24개 통과, interactive Keychain 검사 1개 ignored.
- 데스크톱 자체 Clippy(`--no-deps --all-targets -- -D warnings`), 변경 파일 Rust fmt,
  대시보드 메타데이터 검증을 통과했다. 의존성까지 검사하는 Clippy는 기존
  `crates/dgl-core/src/connection.rs`의 `unnecessary_unwrap` 경고로 실패했다.
- Windows 공통 코드·빌드 경로는 유지했다. Windows 네이티브 빌드와 Mac·Windows 실제 GUI 검증은 수행하지 않았다.

## tmux 패널 도구 모음 후속 검증 — 2026-10-25

- 세션 목록 아래의 패널 분할·이동·크기 조절을 터미널 상단으로 옮기고 확인 후 닫기를 추가했다.
- Rust 데스크톱 검사 25개 통과, interactive Keychain 검사 1개 ignored. 좁은 두 그룹에서
  각 도구 모음이 탭 바와 터미널 사이에 배치되고 카테고리가 영역 밖으로 넘치지 않는 것을 확인했다.
  연결 미준비 상태의 분할·이동·크기 조절·닫기는 오류·삭제 대화상자·터미널 입력을 만들지 않는다.
- 공통 Compose UI와 Android 앱·계측 테스트 코드를 컴파일했다. Android 계측 시나리오는
  새 도구 위치와 닫기 취소에 맞춰 갱신했으며 이번 작업에서 기기 실행은 하지 않았다.
- Windows 빌드 경로는 유지했고 Mac·Windows 실제 GUI 검증은 수행하지 않았다.

## 하단 단축키 후속 검증 — 2026-10-25

- 하단 키와 CLI 메뉴 버튼에 테두리·배경·둥근 모서리·간격을 적용했다.
- `Esc`·`Tab`·`Enter`·`Shift+Tab`·`Shift+←`를 모바일·데스크톱 공통 기본 단축키 설정에 포함했다.
  고정 목록 대신 저장된 `terminalShortcuts`를 렌더링하며, 설정에서 키별 표시 선택과 기본값 복원이 가능하다.
  필드가 없는 기존 설정에는 기본값을 적용하고, 사용자 지정 순서와 빈 목록은 저장·재시작 후에도 유지한다.
  Android의 48dp 터치 영역·가로 스크롤·고정 CLI 메뉴 버튼은 유지한다.
- Shift 조합키는 각각 `ESC [ Z`, `ESC [ 1 ; 2 D`를 보내며 Rust/Kotlin 계약 fixture는 124건이다.
- 커밋 대상만 분리한 Rust 데스크톱 25개 통과·interactive Keychain 1개 ignored, 코어 계약 검사 3개와 JVM 입력·계약·저장 검사 12개 통과.
  GPUI에서 양 테마의 버튼 간격·클릭당 1회 전송·다른 터미널 무전송·포커스 복귀·물리 키보드와의 일치를 확인했다.
  설정에서 키를 끈 뒤 하단 목록에 반영되는 동작, 저장 파일 복원과 기본값 복원도 확인했다.
- Android 앱·계측 테스트 컴파일을 통과했다. 새 키의 전송 검사 시나리오를 기존 계측에 추가했지만
  Android 기기 실행과 Mac·Windows 실제 GUI 검증은 이번 작업에서 수행하지 않았다.

## 재현

```sh
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer cargo test --workspace
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
python3 scripts/integration.py --tmux .tools/tmux-3.3a/tmux-3.3a/tmux --command sh -c 'cargo test -p dgl-core --test ssh_integration -- --ignored --test-threads=1'
python3 scripts/integration.py --tmux .tools/tmux-3.7c/tmux-3.7c/tmux --command sh -c 'cargo test -p dgl-core --test ssh_integration -- --ignored --test-threads=1'
./gradlew :app:desktopTest :androidApp:assembleDebug :androidApp:assembleRelease :androidApp:assembleDebugAndroidTest :androidApp:lintDebug
# 전용 API36 AVD를 띄우고 해당 serial을 지정한다. JAVA_HOME/ANDROID_HOME 필요.
ANDROID_SERIAL=emulator-5580 python3 scripts/integration.py --tmux .tools/tmux-3.7c/tmux-3.7c/tmux --command python3 scripts/rust_android_integration.py
python3 scripts/package_rust_mac.py
python3 scripts/collect_rust_android.py
```
