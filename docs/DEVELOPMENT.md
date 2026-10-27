# 개발·빌드·검사

저장소 루트에서 실행합니다. Windows 전용 환경은 [WINDOWS.md](WINDOWS.md)를 따릅니다.
현재 모듈은 [ARCHITECTURE.md](ARCHITECTURE.md), 도구별 역할은 [scripts/README.md](../scripts/README.md)에 있습니다.

## 도구

| 용도 | 저장소 기준 |
|---|---|
| 공통 스크립트 | Python 3.9 이상 |
| Rust | 1.98.1, `Cargo.lock` 사용 |
| Android | JDK 17, SDK 36, Build Tools 36.0.0, NDK 29.0.14206865 |
| Gradle | Wrapper 9.7.0, AGP 9.3.3, Kotlin 2.4.21, Compose 1.9.3 |
| 터미널 assets | Node.js/npm, CI Node.js 22.23.4, `web/package-lock.json` |
| macOS | Xcode 26.6 + Metal Toolchain, ARM64 검증 경로 |
| Windows | VS 2022 C++·Windows SDK·CMake, Rust MSVC x64, Python·Git |

Android 작업 전 `JAVA_HOME`을 JDK 17, `ANDROID_HOME`을 SDK 경로로 설정합니다.
macOS Rust GUI 빌드는 명령별 `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`를 사용합니다.
Windows 앱만 빌드할 때 JDK·Android SDK는 필요하지 않습니다.

## 기본 검사

```sh
python3 scripts/check_prompt.py
python3 scripts/verify_wrapper.py
python3 scripts/generate_locales.py --check
python3 -m unittest discover -s scripts/tests -v
cargo test --locked -p dgl-core -p dgl-uniffi
./gradlew :app:desktopTest
python3 scripts/verify_test_results.py --mode linux --output artifacts/jvm-tests.json
npm test --prefix web
```

단축키 회귀 검사는 Rust 공통 인코더·UniFFI, Kotlin 저장 호환/조합키 수명, 웹 키·IME 출처/붙여넣기·마우스 비간섭/지연된 대상 검사를 포함합니다.
크기·폰트 회귀 검사는 즉시 제출·50ms 제한·실패 후 150ms 재시도·최신 크기 병합, 렌더러 소유 IME 세대의 취소·재부착,
지연된 폰트 측정·숨김 후 표시, 조합 중 스냅샷 교체·커서 좌표와 입력 보조 요소 캐시 제외를 포함합니다.
모바일 웹 검사는 원격 열 수 유지·전체 컨테이너 측정, 세로 드래그 방향·행 단위 누적,
비활성 pane 선택·대상 세대·취소·핀치·IME 분리와 합성 클릭 차단도 확인합니다.

검증기의 `linux` 모드는 macOS에서도 사용하는 비 Windows·비 SSH JVM 검사 모드입니다.
외부 SSH/Windows 환경이 필요한 JVM 검사는 예상 skip, Rust 통합/Keychain 검사는 기본 ignore입니다.
이 결과를 실기 검사로 보고하지 않습니다.

macOS 데스크톱까지 확인할 때:

```sh
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer cargo test --locked --workspace
```

데스크톱 변경만 검사할 때는 같은 환경에서 `cargo test --locked -p dgl-desktop`을 실행합니다.
GPUI 검사는 실제 Ctrl+휠의 확대·축소/원격 전송 차단과 좁은 창·분할 화면의 우측 상단 초기화 FAB 위치·클릭·포커스를 포함합니다.
제목 빠른 갱신 검사는 조각난 OSC·BEL/ST·다른 제어 문자열, 가상 시간 499/500ms·연속 신호 병합·동시 조회 차단,
숨김/연결/pane 세대의 오래된 응답 거부와 전면·그룹별 표시 상태 전달을 포함합니다.
호스트·하단 단축키 순서 모델, 설정 상세 화면 왕복·포커스, GPUI 드래그 확정/취소·조합키 1회 소비·세션 탭 재사용 검사를 포함하며 대화형 Keychain 검사는 기본 ignore입니다.
알림 패널은 버튼 기준 위치·좁은 창·빈/긴 목록·내부 스크롤·닫기·입력 차단·포커스 복원을 검사합니다.
네이티브 툴팁은 영역 클리핑·스크롤·크기 갱신·가림·제거·창 수명을 GPUI로 검사하며 실제 OS 표시와 Windows MSVC 빌드는 별도 확인합니다.
스낵바는 Rust·Kotlin에서 프리셋·기본값·사용자 지정 ms의 만료 경계, 같은 문구 재호출과 오래된 닫기/만료 거부를 검사합니다.
GPUI 검사는 하단 3px 게이지의 남은 시간·위치, 영구 노출 시 게이지 없음, 닫기·포커스를 포함합니다.

기본은 위 기능·단위 검사이며 전체 Android API 계측과 장시간 자원 측정은 별도 선택입니다.
검사 결과·남은 한계는 [VALIDATION.md](VALIDATION.md)에 기록합니다.

### 스낵바 API

Rust 데스크톱은 `SnackbarState::show_snackbar(message)`가 기본 5초이고,
`show_snackbar_with_duration(message, duration)`으로 `SnackbarDuration::{Short, Normal, Long, Indefinite}`
(3초·5초·10초·영구) 또는 `SnackbarDuration::Millis(ms)`를 지정합니다.
터미널 안내는 `SnackbarRequest`에 문구와 시간을 담아 UI로 전달합니다.

Kotlin의 연결별 `inputNotice: SnackbarState`는 `show(message)` 또는 `show(message, duration)`으로
같은 네 프리셋을 받으며, `show(message, durationMs = 7500)`처럼 밀리초를 직접 지정할 수 있습니다.
밀리초 0은 즉시 종료, 음수는 허용하지 않습니다. 영구 노출은 `Indefinite`로 지정하고 게이지를 만들지 않습니다.
기존 호출부에는 영구 노출을 배정하지 않습니다. 문구 비교 대신 호출 지점에서 프리셋을 정하고,
알림 ID와 단조 시계로 교체·닫기·만료 및 게이지를 관리합니다. Android 일반 오류의 `notice` 대화상자는 별도 경로입니다.

### UI 번역

`locales/en.json`은 한국어 원문 키와 영어 번역의 공통 목록입니다. 문구와 번호 자리표시자(`{0}` 등)를
추가·수정한 뒤 `python3 scripts/generate_locales.py`로 Kotlin 목록을 갱신합니다.
Rust는 같은 JSON을 바이너리에 포함합니다. `--check`는 누락된 키·번역·자리표시자와 생성 파일 일치를 검사합니다.
표시 문구는 `t`, 인수가 있는 문구는 `tr!`(Rust)/`tr`(Kotlin)를 사용하며 완성된 사용자 문자열에 번역을 적용하지 않습니다.
언어 선택은 `preferences.language`의 `system|ko|en`이며 예전 설정은 `system`, 알 수 없는 언어 선택도 시스템으로 해석합니다.
tmux 명령·에러 코드·원격 터미널 바이트는 언어와 무관하게 유지합니다.

## Android 빌드

```sh
rustup target add --toolchain 1.98.1 aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
npm ci --prefix web --ignore-scripts --no-audit
npm run bundle --prefix web
./gradlew :androidApp:assembleDebug :androidApp:assembleRelease :androidApp:lintDebug
python3 scripts/collect_rust_android.py
```

Gradle의 Rust 작업은 `scripts/build_rust_android.py`로 네 ABI를 빌드합니다.
`dglEngine` 선택 속성은 제거되어 전달하면 실패합니다.
Debug APK는 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 하나를 사용합니다.
검사·배포 스크립트는 이 원본을 검증하고 APK 복사본을 만들지 않습니다. Release APK는
`androidApp/build/outputs/apk/release/androidApp-release-unsigned.apk`이며 미서명입니다.
앱 ID는 `kim.jujin.dglc`, versionName/code는 `0.1.0` / `1`입니다.

## 데스크톱 패키징

```sh
python3 scripts/package_rust_mac.py
# release 프로필이 필요할 때
python3 scripts/package_rust_mac.py --release
```

출력은 `artifacts/rust-candidate/mac/DaGaLi.app`과 `build.json`입니다.
스크립트는 Xcode 경로를 설정하고 로컬 ad-hoc 서명·검증을 수행합니다. Developer ID·공증은 하지 않습니다.
저장된 `desktop/resources/dgl.icns`를 번들 Resources에 넣고 `CFBundleIconFile`로 연결한 뒤 서명합니다.
Windows는 `scripts/run_windows.cmd` 또는 `scripts/build_windows.ps1`로 빌드·ZIP을 생성합니다.
기본은 빌드만, `-Test`는 단위 검사, `-Run`은 빌드 후 앱 실행입니다.

### 앱 아이콘 갱신

공통 도형·디자인 명세·크기별 미리보기와 변환 절차는 [앱 아이콘 안내](../assets/app-icon/README.md)에 있습니다.
일반 빌드는 저장된 리소스를 사용합니다. 갱신할 때만 Python 가상환경에 Pillow 11.3.0을 설치하고
`scripts/generate_app_icons.py`로 원본 PNG부터 플랫폼별 리소스·모바일 상단 Compose 벡터까지 생성합니다. `--check`로 파일 일치를 확인한 뒤
Android Debug·lint와 macOS 패키징을 확인합니다. Windows는 기존 SDK `rc.exe`로
실행 파일의 아이콘 ID `1`을 컴파일하며 네이티브 빌드·GUI 확인은 Windows에서 별도로 수행합니다.

## 선택 검사

격리 SSH 검사는 OpenSSH·임시 키·독립 tmux socket을 사용합니다. 기존 사용자 서버·키를 사용하지 않습니다.
`integration.py`의 기본 명령은 JVM 참조 검사이므로 Rust 검사는 `--command`를 지정합니다.

```sh
python3 scripts/build_test_tmux.py 3.7c
python3 scripts/integration.py --tmux .tools/tmux-3.7c/tmux-3.7c/tmux --command cargo test --locked -p dgl-core --test ssh_integration isolated_auth_management_input_reconnect -- --ignored --exact
```

복귀 응답 확인·소켓을 닫지 않는 무응답·출력 역압력 중 3초 판정·동일 tmux 작업/입력 복원과
크기 변경 중 큐 입력의 한글·영문 순서/중복 여부는 아래 한 검사로 확인합니다.
`--command` 이후 인수와 Cargo의 `--` 구분자는 자식 명령에 그대로 전달합니다.

```sh
python3 scripts/integration.py --tmux .tools/tmux-3.7c/tmux-3.7c/tmux --command cargo test --locked -p dgl-core --test ssh_integration resume::healthy_resume_and_blackhole_recovery_preserve_work_without_replaying_input -- --ignored --exact
```

제목 빠른 조회와 기존 숨김 구독을 함께 확인할 때:

```sh
python3 scripts/integration.py --tmux .tools/tmux-3.7c/tmux-3.7c/tmux --command cargo test --locked -p dgl-core --test ssh_integration hierarchy_titles_all_sessions_without_screen_reset -- --ignored --exact
```

검사 중 해당 fixture 클라이언트의 메타데이터 구독을 잠시 제거하여 OSC 빠른 경로를 분리합니다.
제목 조회·숨김/전면 복귀·출력 원문·입력 대상/화면 보존을 확인한 뒤 구독을 복원하고
숨긴 창/다른 세션 제목·재접속 검사를 이어갑니다. 사용자 tmux 설정·서버는 사용하지 않습니다.

tmux 소스 빌드에는 C 컴파일러·make·libevent·ncurses 개발 헤더가 필요합니다.
Android 계측은 전용 에뮬레이터와 명시적인 `ANDROID_SERIAL`을 사용합니다.
API36 후보 검사는 앱/계측 APK를 먼저 빌드하고 fixture 안에서 실행합니다.
연결 수명·IME·정상 복귀의 즉시 확인 dim과 SSH 유지·유지 끔의 자동 재연결/취소·삭제된 세션/인증 실패 보존·알림 인텐트·task 제거 후 새 실행만 짧게 확인할 때는 아래 실행에
`DGL_ANDROID_TEST_CLASSES=kim.jujin.dglc.AndroidConnectionLifetimeTest`를 추가합니다.
전체 API matrix로 확장하지 않습니다.

`kim.jujin.dglc.AndroidOutputRecoveryTest`는 같은 fixture에서 렌더러 ACK를 실제 15초 넘게 지연시킵니다.
구독 종료 후 SSH 세대·tmux client·원격 프로세스 유지, 모든 표시 pane의 ACK 대기, 늦은 ACK·표면 교체,
자동 화면 복원 실패 후 수동 재시도를 확인합니다. 물리 기기의 장시간 잠금·Doze 재현을 대신하지 않습니다.
Debug 진단은 `adb -s "$ANDROID_SERIAL" logcat -s DglResume`으로 복귀·검사 제출/완료·구독/연결 세대·화면 ACK·입력 허용 시각을 확인합니다.

```sh
./gradlew :androidApp:assembleDebug :androidApp:assembleDebugAndroidTest
ANDROID_SERIAL=emulator-5554 python3 scripts/integration.py --command python3 scripts/rust_android_integration.py
```

성능·자원 측정은 [PERFORMANCE.md](PERFORMANCE.md)의 표본·종료 조건을 먼저 정합니다.
에이전트 실제 CLI 검사에는 설치·로그인과 경우에 따라 유료 호출이 필요하므로 [AGENT_STATUS.md](AGENT_STATUS.md)의 범위를 따릅니다.

Android 작업 공간과 기존 모바일 입력·볼륨키·계층을 전용 API36 한 대에서 짧게 검사할 때:

```sh
./gradlew :androidApp:assembleDebug :androidApp:assembleDebugAndroidTest :androidApp:lintDebug
ANDROID_SERIAL=emulator-5554 python3 scripts/workspace_android.py
```

839/840dp 경계·넓은 분할·키보드와 높이 부족 시 임시 확대·탭 이동/폭 전환 전후 WebView 동일성·
활성 그룹 입력·전역 모달 차단·드래그 확정/취소, 320dp 상단 버튼·36dp 탭/단축키 순서·선택/닫기,
알림 빈/긴 목록·내부 스크롤·IME 공간·닫기/바깥 터치/뒤로 가기·입력 차단/포커스 복원, 세션 폰트 핀치·취소·최신 기본값 초기화·탭/Activity 수명과
터미널 가로 스크롤 경계도 검사합니다. 화면 크기·밀도·IME·글꼴·애니메이션 설정은
`finally`에서 복원합니다. 결과는 `artifacts/android-workspace/`에 남기며 명시적 검사 이름 목록과 대조합니다.
하단 CLI 메뉴는 키보드 표시/숨김 유지·크기 변경 복원 중 메뉴 유지·명령 1회 전송·닫기 후 포커스와
입력 차단·가로 화면의 내부 스크롤을 `AndroidMobileInputTest.cliLauncherKeepsImeAndSurvivesResizeWithoutLeakingInput`으로 확인합니다.
JVM 기본 검사에는 분할·이동·합치기·선택·최소 공간/최대 그룹·배치 저장·세션 예약·오래된 연결 대상 거부가 포함됩니다.
`--section workspace`, `--section mobile-input`, `--section compact`로 필요한 구간만 재검사할 수 있습니다.
전체 Android API matrix로 확대하지 않습니다.
삼성 키보드 실기 인수는 키보드를 열자마자 한글을 입력해 조합 글자가 커서에 붙어 있는지와 최종 문자열이 정확한지를 확인합니다.
영문·숫자도 비교하며, 자동 좌표 검사·계측 APK 컴파일을 이 실기 결과로 계산하지 않습니다.

터미널 너비·터치 스크롤만 검사할 때는 위 Debug·계측 APK를 전용 API36에 설치한 뒤
`am instrument -w -r -e class`의 선택을
`kim.jujin.dglc.AndroidPlatformTest#bundledRendererRemoteColumnsAndTouchScrolling,kim.jujin.dglc.AndroidPlatformTest#windowPanesKeepIndependentScreensAndOnlyTheContainerRequestsSize`로 제한합니다.
실제 번들 xterm의 80열·분할 pane 마지막 열과 줄바꿈, 스크롤바 숨김·가로 이동,
SGR·바이너리 휠·방향키 대체·로컬 이력 및 실제 Android 터치의 키보드 미표시를 확인합니다.
`scripts/verify_android_results.py`의 `expected_for_selection`과 `verify`로 해당 이름의 완료·실패·skip을 대조합니다.

## 의존성·배포 검사

```sh
./gradlew writeDependencyInventory
python3 scripts/audit_dependencies.py
python3 scripts/verify_licenses.py
python3 scripts/verify_lint.py
python3 scripts/collect_artifacts.py
python3 scripts/scan_secrets.py
```

`collect_artifacts.py`는 빌드된 Debug·Release APK의 ID/SDK·백업 정책·번들 고지·Rust payload를 확인하고
`artifacts/build-manifest.json`, `artifacts/SHA256SUMS`를 생성합니다.
보고서와 해시 목록의 APK 경로는 저장소 루트 기준이며, 루트에서 `sha256sum -c artifacts/SHA256SUMS`로 확인합니다.
`collect_rust_android.py`는 원본 Debug APK의 서명·Rust payload를 확인하고
`artifacts/rust-candidate/android/build.json`에 원본 경로와 검사 결과를 기록합니다. 두 도구 모두 APK를 복사하지 않습니다.
잠금·검증 메타데이터·라이선스 갱신은 생성 결과를 검토하고 반영합니다.

## CI

`main` push/PR의 **Build and verify**는 Linux 기본 job 하나를 실행합니다.
Python·JVM 검사, Android Debug 빌드·lint, Wrapper·비밀정보·번들 검사를 포함합니다.
Rust 코어/데스크톱 단위 검사는 위 로컬 명령으로 별도 실행합니다.
`README.md`·`TASKS.md`·`AGENTS.md`·`docs/**`·`scripts/README.md`만 바뀌면 CI를 생략합니다. Debug APK는 `android-debug` artifact로 수집합니다.

| 수동 입력 | 추가 작업 |
|---|---|
| `verify_packaging` | Release·계측 APK, 의존성·라이선스·배포 검사 |
| `verify_ssh` | JVM 참조 SSH/tmux 3.3a·3.7c 통합 검사 |
| `verify_android` | Android API 34·35·36 계측 |

입력은 기본 꺼짐입니다. **Build Windows Rust candidate**는 별도 수동 workflow이며
Windows MSVC 빌드·ZIP만 수행합니다. CI 성공은 서명·공개 릴리스·Windows 실기 인수 완료를 뜻하지 않습니다.
