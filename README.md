# dgl client

**Keep the CLI. Simplify the workflow.**

SSH와 표준 tmux를 사용하는 Windows·Mac·Android 터미널 클라이언트입니다. 원격 데몬 없이 기존 세션을 탐색하고, 연결을 닫은 뒤 같은 프로세스로 돌아갑니다. Rust 코어를 사용하는 **개발용 후보**입니다. Windows 네이티브 빌드·실기, Mac 실제 GUI·Keychain과 전체 기능 화면 인수는 대기입니다. Codex·Claude의 일부 공식 훅은 실제 CLI에서 확인했으며 [검증 범위](docs/AGENT_STATUS.md)를 구분해 기록합니다.

## 현재 구현

- 호스트 추가·편집·삭제, 별칭·즐겨찾기·최근 접속, 독립된 여러 연결
- Google Drive·iCloud Drive 파일 기반 설정·호스트 동기화, 민감정보 제외·동시 수정 충돌 선택 ([지원 플랫폼·사용 방법](docs/CLOUD_SYNC.md))
- 비밀번호, RSA/Ed25519 및 암호화 개인키 가져오기, Ed25519 생성·공개키 복사
- 최초 호스트키 지문 확인, 저장된 키 변경 시 차단, 신뢰 키 삭제
- Android Keystore / Mac Keychain / Windows DPAPI 보호 저장. 실패 시 저장 중단 (Windows 실행 검증 대기)
- flat 탐색기의 세션 → 창 → 패널 트리, 독립적인 펼침·대상 선택, ↻ 새로고침과 행별 … 메뉴
- tmux 패널 분할, 방향별 포커스 이동, 3셀 경계 조절, 생성·이름 변경·삭제 확인
- 데스크톱 그룹별 탭, 좌우·상하 중첩 화면 분할, 경계 드래그, 탭 재배치·그룹 이동·합치기
- 그룹 배치만 workspace-layout.json에 복원하며 SSH 연결은 사용자가 다시 열기
- 외부 tmux 변경 이벤트 반영, 연결 복구 중 입력 차단, 입력 재전송 금지
- 네트워크 단절 시 자동 재시도 최대 7회, 접속 정보를 유지하는 `새로고침 재시도`·`재시도 정지` 버튼
- 화면 캡처·실시간 출력 순서화, 연결·패널·크기 세대 검사, bounded output / scrollback
- Android xterm.js/WebView, Mac GPUI/alacritty_terminal, 단축키 바, 앱 테마·폰트 크기
- Android 키보드 크기 변경 통합 처리, 화면 캐시와 fade, 선택형 크기·크로스페이드·블러 전환
- 하단 `…` 메뉴에서 Claude Code·Codex·AGY·Grok Build 일반 실행 또는 Bypass 실행 (AGY·Grok Build는 `test`)
- 모든 tmux 창·패널의 네 CLI 프로세스 아이콘, 선택적 공식 훅 연동, 읽지 않은 사건 배지·앱 내 알림·OS 알림·대상 패널 이동
- Android 볼륨 위·아래 키별 단축키·텍스트/키 매크로 설정 (기본 꺼짐)
- tmux 미설치/버전 미지원 시 일반 SSH 셸. **일반 셸의 작업은 연결 종료 후 보존되지 않을 수 있습니다.**

## 플랫폼 범위

| 대상 | 설정 | 실제 확인 |
|---|---|---|
| Android 14 / 15 / 16 | `minSdk=34`, `compileSdk=36`, `targetSdk=36` | ARM64 에뮬레이터에서 APK·Keystore·SSH·합성 IME·키보드·단축키 바·회전·복귀 검증 |
| Windows x64 | GPUI + alacritty_terminal, DPAPI, MSVC 빌드·Release ZIP·수동 CI 구성 | 네이티브 빌드·실기 검증 대기; [Windows 안내](docs/WINDOWS.md) |
| Linux | 후속 Rust 데스크톱 대상 | 현재 실행·배포 경로 없음 |
| macOS ARM64 | GPUI + alacritty_terminal, macOS Keychain | 빌드·코어 SSH/tmux 통과; 실제 GUI·Keychain 저장 인수 **대기** |
| tmux | 3.3–3.7 범위만 Control Mode 사용 | **3.3a, 3.7c** 실제 서버 검증 |

앱 식별자(Android applicationId / macOS 번들 ID / Windows AUMID): `kim.jujin.dglc`.
추가 플랫폼과 WSL 제품 연동은 후속 단계입니다.

## 빌드·실행

Android는 Rust 코어만 사용합니다. 엔진 선택 옵션과 이전 JVM 앱 실행·설치물 빌드는 제거했습니다.
`dglEngine` 속성을 넘기면 오류로 중단합니다. 기존 엔진은 회귀 검사와 격리 SSH 관리에 필요한
`app/src/referenceTest`에만 남으며 앱 APK에는 포함하지 않습니다.

JDK 17, Android SDK 36 / Build Tools 36.0.0 / NDK 29.0.14206865,
Rust 1.98.1, Node.js/npm이 필요합니다. `JAVA_HOME`과 `ANDROID_HOME`을 설정하세요.
Gradle 9.7.0, Kotlin 2.4.21, AGP 9.3.3을 사용합니다. Mac은 Xcode 26.6과 Metal Toolchain이 필요합니다.

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
./gradlew :app:desktopTest :androidApp:assembleDebug :androidApp:assembleRelease :androidApp:lintDebug
python3 scripts/collect_rust_android.py
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer python3 scripts/package_rust_mac.py
```

- Android: `androidApp/build/outputs/apk/debug/androidApp-debug.apk` (debug 서명, 네 ABI).
  전달용 복사본: `artifacts/rust-candidate/android/dgl-rust-debug.apk`.
- Mac: `artifacts/rust-candidate/mac/dgl Rust Candidate.app` (ARM64, 로컬 ad-hoc 서명).
- Release APK는 미서명이며 공개 배포용 서명·공증은 하지 않았습니다.

Android 앱 ID는 `kim.jujin.dglc`입니다. 이전 `kim.jujin.rovessh` 앱과 별도로 설치되며,
설정·자격 증명은 새 앱의 `noBackupFilesDir/rust-candidate-v1`에 저장합니다.
이전 사용자 데이터는 자동으로 이전하거나 삭제하지 않습니다.

`crates/dgl-core`는 Rust 연결·tmux 코어, `crates/dgl-uniffi`/`:coreBindings`는 바인딩,
`:mobileUi`는 Rust 어댑터, `:app`은 공통 모델·Compose/xterm 컴포넌트,
`:androidApp`은 Android 진입점, `desktop`은 GPUI 앱입니다.

Windows는 VS 2022 Build Tools(C++·Windows SDK·CMake), Rustup, Python 3.9 이상을 준비하고
저장소 루트의 PowerShell에서 실행합니다. JDK·Android SDK는 Windows 앱 빌드에 필요하지 않습니다.

Windows 메뉴: `.\scripts\run_windows.cmd` (탐색기에서 더블클릭 가능).
빌드·실행, ZIP만 빌드, 테스트 포함 실행, 전체 도구·업데이트 확인, 도구 설치·업데이트를 선택합니다.
도구별 문제가 발생해도 나머지를 모두 확인한 뒤 결과를 표시합니다.
메뉴 없이 빌드·실행: `.\scripts\run_windows.cmd -Run`, 테스트 포함: `.\scripts\run_windows.cmd -Test`.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\build_windows.ps1
```

미서명 Release ZIP: `artifacts/rust-candidate/windows/dgl-rust-windows-x64.zip`.
기본은 빌드만 수행하며 `-Test`로 단위 검사를 선택할 수 있습니다.
설치할 도구·수동 CI·실기 검사 방법은 [Windows 빌드·테스트 안내](docs/WINDOWS.md)를 따릅니다.

## 연결 방법

1. 원격 서버에 SSH와 tmux를 설치하고 UTF-8 로케일을 구성합니다.
2. 앱에서 호스트·포트·사용자와 인증 정보를 저장합니다. 개인키는 파일에서 가져올 수 있습니다.
3. 처음 연결할 때 표시된 SHA-256 지문을 서버에서 독립적으로 확인하고 신뢰합니다.
4. 기존 세션을 선택하거나 새 세션을 만듭니다. 창·패널을 GUI로 관리합니다.
5. `연결 종료`는 SSH와 해당 tmux 클라이언트만 닫습니다. `삭제`는 이름을 확인한 뒤 원격 대상을 종료합니다.

네트워크 오류는 2·4·8·16·30·30·30초 간격으로 최대 7회 자동 재시도합니다. 7회를 소진하면 추가 자동 접속을 멈추고 탭·접속 정보·인증 정보·신뢰 지문·마지막 tmux 대상을 유지합니다. 데스크톱은 각 터미널 위, Android는 연결 상태 아래에 `새로고침 재시도`와 `재시도 정지`를 표시합니다. `새로고침 재시도`는 대기 중에도 즉시 접속하고 자동 재시도 횟수를 새로 시작합니다. `재시도 정지`는 진행 중인 접속과 예약된 재시도를 멈추며, 다시 버튼을 누르기 전까지 자동 접속하지 않습니다. 연결 성공 후에도 횟수를 초기화하므로 다음 단절은 새로운 복구 회차입니다.

기존 tmux 세션이 서버에 남아 있으면 이어 붙습니다. 일반 SSH 셸은 기존 프로세스 복원을 보장하지 않습니다. 호스트키 변경·인증 실패는 자동 재시도하지 않으며, 수동 재시도도 서버 지문 검증을 생략하지 않습니다. 이 동작은 앱 프로세스와 연결 탭이 유지되는 동안 적용됩니다.

호스트 입력창의 `비밀번호 표시` 체크로 비밀번호·키 암호를 확인할 수 있습니다. 새 입력창은 숨김 상태로 시작합니다. 데스크톱은 `연결 정보`와 `인증` 영역에서 비밀번호·개인키를 선택합니다. 개인키의 `파일 선택…` 또는 빈 경로의 `경로에서 가져오기`는 macOS의 `~/.ssh`, Windows의 `%USERPROFILE%\.ssh`에서 파일 선택창을 엽니다. 해당 폴더가 없으면 사용자 폴더에서 엽니다. 폴더 경로를 입력하면 그 안에서 선택하고, 파일 경로를 입력하면 직접 가져옵니다. [개인키 가져오기·오류 안내·Windows 실기 검사](docs/HOST_AUTH.md)를 참고하세요.

데스크톱은 왼쪽 탐색기에 호스트와 활성 그룹의 세션 트리를 함께 표시합니다. Android는 `tmux 관리` 시트에서 같은 트리를 사용합니다. `▸`는 펼침만 수행하며 이름을 누르면 해당 세션·창·패널을 선택합니다. 헤더의 `+`는 세션 생성, `↻`는 세션 새로고침입니다. 창 생성·이름 변경·삭제는 각 행의 `…` 메뉴에 있습니다. 삭제에는 확인이 필요합니다.

세션은 실제 세션명, 창은 활성 패널의 작업 제목, 패널은 자신의 작업 제목을 강조합니다. 창 번호·실제 창 이름·프로세스·개수·크기는 작은 보조 정보로 표시하며, 접속 중 제목 변경은 터미널 화면을 초기화하지 않고 자동 반영합니다. [표시 규칙과 접근성 지원 범위](docs/SESSION_HIERARCHY.md)를 참고하세요.

tmux 패널 도구는 세션 목록과 분리하여 각 터미널 상단에 표시합니다. `패널 분할`의 ↔ / ↕는 좌우·상하 분할, `이동`의 ← → ↑ ↓는 선택 패널과 입력 대상 이동, `크기 조절`의 화살표는 경계를 3셀씩 조절합니다. `닫기 ×`는 현재 패널을 확인 후 종료합니다. 데스크톱에서는 각 탭 그룹에 도구 모음이 있으며 해당 그룹의 터미널에 적용합니다. Android에서는 도구를 가로로 스크롤할 수 있고 닫기 버튼은 오른쪽에 고정됩니다. 일반 SSH·재연결 등 tmux 패널을 조작할 수 없는 상태에서는 버튼을 비활성화합니다.

데스크톱 탭 바 오른쪽 끝의 `↔️ / ↕️` 버튼은 좌우·상하에 빈 탭 그룹을 만듭니다. 버튼은 탭이 많아도 오른쪽에 고정됩니다. 탭 제목을 다른 탭 위로 끌면 순서를 바꾸고, 그룹 중앙에 놓으면 해당 그룹으로 이동합니다. 그룹 본문의 왼쪽·오른쪽·위·아래 가장자리로 끌면 반투명 예상 배치가 표시되며, 놓으면 그 방향으로 분할하고 탭을 옮깁니다. 빈 그룹이나 자기 그룹의 유일한 탭, 최소 공간 또는 최대 16개 그룹 제한으로 분할할 수 없는 경우에는 분할 미리보기를 표시하지 않습니다. Esc로 드래그를 취소할 수 있습니다. 중첩 분할·경계 드래그와 탭 `…` 메뉴의 순서 변경·그룹 이동도 지원합니다. `🔗`는 인접 그룹과 합칩니다. 연결·터미널·로컬 스크롤 기록은 탭과 함께 이동합니다. 새 호스트 연결은 활성 그룹에 열리며 동일 호스트도 독립된 연결로 열립니다. `그룹 합치기`는 인접 그룹으로 탭을 옮기고 분할을 없앱니다. 마지막 탭을 닫아도 빈 그룹은 남습니다.

재실행 시 설정 디렉터리의 `workspace-layout.json`에서 분할 구조·비율만 읽습니다. 모든 그룹은 빈 상태로 시작하고 SSH는 자동 연결하지 않습니다. 파일이 없거나 잘못되면 빈 단일 그룹으로 시작합니다. 기존 `settings.json` 형식은 그대로 유지합니다.

데스크톱 대화상자는 첫 입력 항목으로 포커스를 이동하고 `Tab`·`Shift+Tab`으로 입력과 버튼을 순환합니다. 버튼은 Enter/Space, 이름 입력은 Enter로 실행하며 IME 조합 중에는 제출하지 않습니다. `Esc`로 닫으면 이전 포커스로 돌아갑니다. 일반 터미널의 Tab 전송은 유지됩니다. [UX 검증 결과와 범위](docs/UX_WORKSPACE.md)를 참고하세요.

데스크톱 터미널은 휠·트랙패드 이동량, 원격 앱이 요청한 마우스 클릭·드래그·호버, Shift/Ctrl/Alt와 방향키·기능키의 조합을 처리합니다. Shift+마우스는 로컬 선택·스크롤에 사용합니다. Android는 xterm.js의 텍스트·바이너리 입력을 전달합니다. [입력 지원 범위와 검증 결과](docs/TERMINAL_INPUT.md)를 참고하세요.

서버에서 `tmux new-session`, `tmux attach`, `tmux list-sessions`를 그대로 사용할 수 있습니다. 앱은 tmux 서버의 전역 설정·사용자 셸·키 바인딩을 변경하지 않습니다.

tmux는 SSH 기본 PATH, 로그인 셸 PATH, 대표 설치 경로 순서로 탐지합니다. 대표 경로에는 Homebrew·Linuxbrew·MacPorts, `/usr/bin`·`/bin`·`/usr/local/bin`, 사용자 `~/.local/bin`·`~/bin`, Nix·Snap 위치가 포함됩니다. 찾은 절대 경로를 버전 확인·세션 조회·생성·Control Mode 연결에 함께 사용합니다.

탐지나 버전 확인이 실패하면 상태 라벨 옆의 ⓘ를 눌러 호스트·탐지 시도와 결과·검사한 경로·확인 명령을 볼 수 있습니다. 경로 미발견, 실행 실패, 지원 범위 밖 버전, 버전 응답 오류를 구분하며 미설치로 단정하지 않습니다. 상세 대화상자는 관리 영역을 접어도 열 수 있습니다.

Android 키보드의 연속 크기 변경은 마지막 변경 이후 150ms에 한 번 전달합니다. 화면 복원 중에는 이전 화면과 옅은 상단 그라디언트를 유지하고, 새 화면을 그린 뒤 짧게 fade합니다. 설정의 `부드러운 터미널 화면 전환`을 켜면 약 240ms의 크기 보간·크로스페이드·블러 전환을 사용합니다. 기본은 꺼짐이며 기기에 저장됩니다. 시스템의 동작 줄이기 설정을 따르고, SSH 연결과 실제 터미널 행·열은 시각 효과와 독립적으로 처리합니다.

Android는 `dgl` 옆 서버 드롭다운으로 연결을 전환합니다. 그 아래 한 행에 현재 세션·`tmux 관리`와 연결 상태·종료/재연결 버튼을 표시합니다. 오른쪽 분할 아이콘은 패널 도구 전체를 접거나 펼치며, 모든 모바일 탭에 즉시 적용하고 앱 재실행 후에도 유지합니다.

하단에는 키보드 토글, `텍스트 입력`, 가로 스크롤 단축키, CLI `…` 메뉴가 있습니다. 고정 컨트롤은 단축키를 모두 숨겨도 유지됩니다. `텍스트 입력` 다이얼로그에서 한글·이모지·여러 줄을 편집하고 `전송`으로 보냅니다. Enter 키는 줄바꿈이며 `Enter 함께 보내기`는 기본 꺼짐입니다. 초안은 연결별 메모리에만 유지하고 전송 성공·연결 제거·앱 종료 시 비웁니다. [전송 방식·초안·검증 범위](docs/TERMINAL_INPUT.md#android-텍스트-작성)를 참고하세요.

Android 설정에서 `볼륨 키로 단축키 실행`을 켜고 위·아래 키를 각각 설정할 수 있습니다.
`기본 음량 조절`, `단축키`, `매크로` 중 선택하며 편집기의 `저장`으로 적용합니다.
예를 들어 `Ctrl+B → 텍스트 n` 또는 `텍스트 git status → Enter`를 구성할 수 있습니다.
텍스트 끝에 Enter를 붙이지 않으며 실제 줄바꿈만 Enter로 변환합니다. `\n`처럼 입력한 문자는 그대로 보냅니다.
빈 매크로와 UTF-8 변환 후 64 KiB를 넘는 입력은 저장할 수 없습니다.
선택한 터미널이 보이고 입력 가능한 동안, 키보드 표시 여부와 관계없이 누를 때 한 번 실행합니다.
설정·입력·확인 대화상자, 호스트 목록만 표시한 화면, 연결 해제·복구 중에는 기본 음량 조절을 사용합니다.

하단 단축키는 개별 테두리·배경과 버튼 사이 간격으로 구분됩니다. `Esc`, `Tab`, `Enter`, `Shift+Tab`, `Shift+←`는 모바일·데스크톱 공통 기본 설정에 포함됩니다. 설정의 `하단 단축키`에서 각 키를 켜거나 끄고 기본값으로 복원할 수 있으며, 선택한 목록은 재시작 후에도 유지됩니다. 버튼을 누르면 선택한 터미널에 해당 키를 한 번 보내고 입력 포커스를 돌려줍니다. 좁은 화면에서는 단축키를 가로로 스크롤할 수 있습니다.

Mac·Windows·Android의 하단 단축키 바 오른쪽 `…`는 위쪽으로 CLI 실행 메뉴를 엽니다.
각 행의 왼쪽 60%는 서비스 일반 실행, 오른쪽 40%의 번개 `Bypass`는 권한 확인을 건너뛰는 실행입니다.

| 서비스 | 일반 실행 | Bypass 실행 |
|---|---|---|
| Claude Code | `claude` | `claude --dangerously-skip-permissions` |
| Codex | `codex` | `codex --yolo` |
| AGY `test` | `agy` | `agy --dangerously-skip-permissions` |
| Grok Build `test` | `grok` | `grok --always-approve` |

선택한 명령과 Enter를 현재 활성 터미널에 한 번 보내고 메뉴를 닫아 터미널 입력으로 돌아갑니다.
현재 원격 셸의 작업 디렉터리·환경을 그대로 사용하며, CLI는 원격 환경에 미리 설치·로그인되어 있어야 합니다.
앱은 설치·로그인·기존 프로세스 종료를 수행하지 않으며 추가 확인창도 표시하지 않습니다.
입력할 수 없는 연결 상태에서는 항목이 비활성화됩니다. 연결·탭·패널 변경, 바깥 클릭,
Esc 또는 Android 뒤로가기로 메뉴가 닫힙니다.
Bypass의 의미는 [Claude Code CLI 문서](https://code.claude.com/docs/en/cli-reference)와
[Codex CLI 문서](https://learn.chatgpt.com/docs/developer-commands?surface=cli)를 따릅니다.
AGY는 [권한 문서](https://antigravity.google/docs/permissions/), Grok Build는
[권한 문서](https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/22-permissions-and-safety.md)를 따릅니다.
새 두 서비스의 실행명 오른쪽 `test` 배지는 실험 지원을 뜻합니다. 실제 로그인·유료 호출은 미검증입니다.
서비스·번개 아이콘은 앱에 번들되며 네트워크 다운로드 없이 테마 색상으로 표시됩니다.
데스크톱 패키지는 아이콘의 `simple-icons-LICENSE.md`도 포함합니다.

호스트 메뉴의 `에이전트 상태 연동`에서 변경 내용을 검사한 뒤 선택적으로 도우미와 사용자 훅을 설치합니다.
Python 3.9 이상, Codex 0.163.0/0.164.0·Claude Code 2.1.310이 현재 설치 검사 대상입니다.
설치된 AGY·Grok Build는 실험 지원으로 훅 설치를 허용합니다. 각 CLI의 미설치·비정상 설정은 개별 표시하고 정상인 다른 CLI는 설치할 수 있습니다. 기존 훅은 보존하며
Codex의 `/hooks` 신뢰 검토는 사용자가 수행합니다. 별도 설치 없이도 확인된 프로세스 아이콘을 표시합니다.
AGY 승인은 명시적 `ask_permission`만 수집하며 일반 권한 팝업은 감지하지 않습니다.
Grok의 `idle_prompt`·`task_complete`는 응답 종료로 알리지 않습니다. 설치 검사의 수집 가능 사건과 실행별 실제 **수신 확인 사건**은 구분합니다.
질문·승인 요청·응답 종료·오류는 **발생 사실**이며 현재 대기나 작업 성공을 뜻하지 않습니다.
OS 알림은 설정에서 켜며 Android는 앱 프로세스와 SSH 연결이 유지되는 동안만 수신합니다.
설치·해제·저장 범위와 플랫폼별 검증은 [에이전트 상태·알림 안내](docs/AGENT_STATUS.md)를 따릅니다.

## 검증

```sh
./gradlew :app:desktopTest
cargo test --locked -p dgl-core -p dgl-uniffi
python3 scripts/integration.py --tmux /path/to/tmux-3.3a
python3 scripts/build_test_tmux.py 3.3a
python3 scripts/integration.py --tmux .tools/tmux-3.3a/tmux-3.3a/tmux
python3 scripts/integration.py --tmux-login-path --command ./gradlew :app:desktopTest --tests kim.jujin.rovessh.IntegrationTest.tmuxDiscoveryUsesLoginPathForCreateListAttachAndReconnect
./gradlew :androidApp:connectedDebugAndroidTest
python3 scripts/integration.py --command python3 scripts/rust_android_integration.py
python3 scripts/check_prompt.py
python3 scripts/verify_wrapper.py
python3 scripts/scan_secrets.py
./gradlew writeDependencyInventory
python3 -m unittest discover -s scripts/tests -v
python3 scripts/audit_dependencies.py
python3 scripts/verify_licenses.py
python3 scripts/verify_lint.py
python3 scripts/collect_artifacts.py
```

통합 테스트는 로컬 OpenSSH와 임시 키·독립 tmux 소켓을 사용합니다. 기존 `authorized_keys`, 사용자 tmux 서버와 호스트 설정을 사용하지 않습니다. Android 통합 실행에는 실행 중인 에뮬레이터가 필요합니다(`ANDROID_SERIAL` 기본값 `emulator-5554`). 기본 JVM 테스트는 외부 SSH 환경이 없으면 해당 테스트를 **skip**합니다.

테스트용 tmux 빌드는 Python 3.9 이상, C 컴파일러, make, libevent·ncurses 개발 헤더가 필요합니다. `.tools/`에만 빌드하며 사용자 tmux를 덮어쓰지 않습니다.

`main` push와 `main` 대상 PR에서는 Linux 기본 검증 **1개 작업**만 자동 실행합니다. JVM 단위 테스트, Debug APK 빌드, lint, Python 검사, Wrapper·비밀정보·터미널 번들 검사를 포함합니다. `docs/**`, `README.md`, `TASKS.md`, `AGENTS.md`만 바뀌면 실행하지 않습니다. Debug APK는 `android-debug` artifact에서 받을 수 있습니다.

GitHub Actions의 **Build and verify → Run workflow**에서 아래 검사를 각각 추가할 수 있습니다. 기본값은 모두 꺼짐이며, 기본 검증은 수동 실행에도 포함됩니다.

| 수동 입력 | 추가 검사 |
|---|---|
| `verify_packaging` | Release·계측 APK 빌드, 의존성 취약점·라이선스·배포 파일 검증 |
| `verify_ssh` | 실제 SSH와 tmux 3.3a/3.7c 통합 테스트 |
| `verify_android` | Android API 34/35/36 에뮬레이터 테스트 |

Windows의 이전 JVM 설치물 CI는 제거했습니다. Rust 앱은 별도 수동 workflow인
**Build Windows Rust candidate**에서 실행·실기 검증 없이 빌드하고 ZIP을 수집합니다.
이번 작업에서 원격 CI나 전체 에뮬레이터 matrix를 실행하지 않았습니다.
`collect_artifacts.py`는 APK ID·SDK·번들·네 ABI와 Rust 엔진을 검사합니다.
기존 연결 엔진·SSHJ·Bouncy Castle이 앱 DEX에 있으면 실패합니다. 이 의존성들은 테스트 APK에만 허용합니다.

검증 결과·실기 체크리스트는 [docs/VALIDATION.md](docs/VALIDATION.md), 단계별 상태는 [TASKS.md](TASKS.md)에 기록합니다.

다음 구현 순서와 종료 조건은 [NEXT_WORK.md](NEXT_WORK.md)를 따릅니다. API36의 테스트 전용
화면 표시 계측의 미해결 결과는 보존합니다. R1b 성능 작업은 후속이며 기능 구현을 막지 않습니다.

공통 코어·모바일 UI·Windows/macOS/Linux 데스크톱 UI 분리 전환은 [아키텍처 전환 작업 계획](docs/ARCHITECTURE_MIGRATION.md)으로 추적합니다. R1에서 공통 fixture 120건, 코어 계약, 성능 수집·판정 도구를 추가했습니다. 이번 검증 범위는 현재 Mac(macOS 26.6.2)과 Android API 36이며, [기준선 보고서](docs/migration/R1_BASELINE.md)에 결과와 미완료 항목을 기록합니다. Rust 코어·UniFFI·Mac GPUI 후보와 Android 후보를 구현했습니다. Android 기본 실행은 Rust이며, Mac 실제 화면·Keychain 인수가 남아 후보 완료와 툴킷 최종 채택을 선언하지 않습니다.

## 남은 한계

- Windows/Linux 새 앱과 물리 Android 기기·실제 한글/일본어 키보드, 실제 Wi-Fi↔셀룰러 전환은 별도 검증이 필요합니다.
- 실제 Codex·Claude의 요청·도구·Stop·서브에이전트 훅과 Claude 질문·승인·도구 오류를 확인했습니다. 전체 로그인 흐름·장시간 재접속, Codex 승인 훅, macOS 알림 배너·클릭, Windows 네이티브 토스트는 미검증입니다.
- 스냅샷은 화면·alternate screen·커서·주요 모드를 복원합니다. tmux가 제공하지 않는 현재 SGR pen, 모든 VT 상태와 이전 로컬 scrollback의 완전 복원은 보장하지 않습니다. 필요하면 터미널에서 다시 그리기(Ctrl+L)를 사용합니다.
- 패널 전환·재접속 시 로컬 scrollback은 초기화됩니다. tmux Copy Mode는 별도 상태로 표시하며 해당 화면을 일반 pane 출력으로 오인하지 않습니다.
- 자동 재접속은 최대 8회 시도합니다. 네트워크 오류만 재시도하며 인증·호스트키·프로토콜 오류는 멈춥니다. Android의 지속적인 백그라운드 연결은 보장하지 않습니다.
- 원격 pane 크기는 최대 500×250셀입니다. Windows는 빌드·ZIP 생성 경로를 마련했으며 실제 실행 검증은 대기입니다. Linux 실행·패키징은 후속입니다.
- 2026-10-23 선택 의존성 371개 버전의 OSV 런타임·빌드·테스트 경고는 모두 0건입니다. 공개 릴리스 전 [보안 검토](docs/SECURITY.md)를 완료해야 합니다.

원문 요구사항은 [KICKOFF_PROMPT.md](KICKOFF_PROMPT.md)와 SHA-256으로 보존합니다. 기술 결정: [ADR 001](docs/decisions/001-architecture.md), [ADR 002](docs/decisions/002-terminal-restoration.md). 라이선스: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Mac·Android 후보 전환 — 2026-10-24

성능 계측을 기능 구현의 선행 게이트에서 분리하고 기존 MVP 기능·기본 디자인을 먼저 이식합니다.
후속 사용자 지시인 “legacy는 불필요”에 따라 Android는 Rust만 빌드하고 Mac은 GPUI 앱으로 실행합니다.
현재 검증 결과와 남은 화면 인수는 [후보 구현 기록](docs/migration/CANDIDATE.md),
[다음 작업](NEXT_WORK.md)을 따릅니다.
