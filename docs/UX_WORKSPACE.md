# flat UX와 데스크톱 탭 그룹

2026-10-24, `main`에서 구현했다. Android와 Windows·Mac 공통 GPUI 코드에 적용하며 기존 설정·호스트 ID·플랫폼별 자격 증명 저장소와 Windows 빌드·패키징 진입점을 유지한다.

## 조작과 상태

- 회색 바탕·파란 강조색의 평평한 목록과 얇은 구분선을 사용한다. 시스템·밝게·어둡게 테마는 유지하며 데스크톱 입력창·선택·포커스도 테마를 따른다.
- 데스크톱 왼쪽 탐색기는 호스트와 활성 그룹의 세션 → 창 → 패널 트리를 함께 보여준다. Android는 호스트 목록과 tmux 관리 시트를 유지한다. Android 터치 항목은 최소 48dp를 확보한다.
- 펼침 화살표는 탐색만 하고 행 이름은 대상 선택을 수행한다. 헤더 `+`와 행 `…` 메뉴에 생성·이름 변경·삭제를 배치한다. 삭제 확인은 유지한다.
- `↻`는 세션 새로고침이며 완료 요청 ID로 중복 요청을 차단한다. Android는 `세션 새로고침` 접근성 설명과 툴팁, GPUI는 같은 컨트롤 ID와 툴팁을 사용한다.
- `포커스 ← → ↑ ↓`는 원격 패널을 선택한다. `경계← / 경계→ / 경계↑ / 경계↓`는 3셀씩 크기를 바꾼다. `화면 분할`은 앱 그룹, `tmux 패널 분할`은 원격 작업이다.
- 데스크톱 각 그룹은 독립된 탭 순서와 선택 탭을 가진다. 새 분할은 빈 그룹이며 새 연결은 활성 그룹에 열린다. 동일 호스트 연결도 별도 SSH 핸들을 사용한다.
- 탭 드래그 또는 `…` 메뉴로 순서·소속 그룹을 바꾼다. 연결과 터미널 Entity를 재생성하지 않는다. 같은 패널의 크기 변경 snapshot은 로컬 스크롤 기록과 위치를 유지한다.
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
