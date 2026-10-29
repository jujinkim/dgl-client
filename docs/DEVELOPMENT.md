# 개발·빌드·검사

저장소 루트에서 실행합니다. Windows 전용 환경은 [WINDOWS.md](WINDOWS.md)를 따릅니다.
현재 모듈은 [ARCHITECTURE.md](ARCHITECTURE.md), 도구별 역할은 [scripts/README.md](../scripts/README.md)에 있습니다.

## 도구

| 용도 | 저장소 기준 |
|---|---|
| 공통 스크립트 | Python 3.9 이상 |
| Rust | 1.98.1, `Cargo.lock` 사용 |
| Android | JDK 17, SDK 37 (compile)·36 (target), Build Tools 36.0.0, NDK 29.0.14206865 |
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
python3 scripts/generate_ui_icons.py --check
python3 -m unittest discover -s scripts/tests -v
cargo test --locked -p dgl-core -p dgl-uniffi
./gradlew :app:desktopTest
python3 scripts/verify_test_results.py --mode linux --output artifacts/jvm-tests.json
npm test --prefix web
```

`verify_test_results.py`는 `--mode linux|ssh`와 `--output`이 필수이며 `app/build/test-results/desktopTest`의
모든 지정 JVM 테스트를 대조합니다. `linux` 모드는 macOS에서도 쓰는 비 SSH 모드입니다.
JVM 테스트 이름을 바꾸거나 추가·삭제하면 이 검증기의 명시적 목록도 갱신합니다.

| 예상 결과 | 대상 | 이유 |
|---|---|---|
| JVM skip (`linux` 모드) | `IntegrationTest`, `RecoveryTest`, `PerformanceTest` | JVM 참조 엔진의 외부 SSH 환경 필요 |
| Rust ignore | `dgl-core` `--test ssh_integration` 전체 | `scripts/integration.py`의 격리 OpenSSH/tmux 필요 |
| Rust ignore | `dgl-desktop` `keychain_roundtrip_and_delete` | 잠금 해제된 대화형 macOS Keychain 필요 |

skip·ignore 결과를 통과나 실기 검사로 보고하지 않습니다.

macOS 데스크톱까지 확인할 때:

```sh
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer cargo test --locked --workspace
```

데스크톱 변경만 검사할 때는 같은 환경에서 `cargo test --locked -p dgl-desktop`을 실행합니다.
GPUI 검사는 실제 OS 표시와 Windows MSVC 빌드를 대신하지 않습니다.

`npm test --prefix web`은 `app/src/androidMain/assets/terminal/`의 커밋된 터미널 스크립트를 검사합니다.
`npm run bundle --prefix web`은 xterm 파일을 같은 디렉터리로 복사하며, CI는 번들 후 커밋본과 차이가 있으면 실패합니다.

기본은 위 기능·단위 검사이며 전체 Android API 계측과 장시간 자원 측정은 별도 선택입니다.
검증 범위·남은 한계는 [VALIDATION.md](VALIDATION.md), 변경별 실행 요약은 [HISTORY.md](HISTORY.md)에 둡니다.

### 스낵바 API

Rust 데스크톱은 `SnackbarState::show_snackbar(message)`가 기본 5초이고,
`show_snackbar_with_duration(message, duration)`으로 `SnackbarDuration::{Short, Normal, Long, Indefinite}`
(3초·5초·10초·영구) 또는 `SnackbarDuration::Millis(ms)`를 지정합니다.
터미널 안내는 `SnackbarRequest`에 문구와 시간을 담아 UI로 전달합니다.

Kotlin의 연결별 `inputNotice: SnackbarState`는 `show(message)` 또는 `show(message, duration)`으로
같은 네 프리셋을 받으며, `show(message, durationMs = 7500)`처럼 밀리초를 직접 지정할 수 있습니다.
밀리초 0은 즉시 종료, 음수는 허용하지 않습니다. 영구 노출은 `Indefinite`로 지정하고 게이지를 만들지 않습니다.
기존 호출부에는 영구 노출을 배정하지 않습니다. 문구 비교 대신 호출 지점에서 프리셋을 정합니다.
Android 일반 오류의 `notice` 대화상자는 별도 경로입니다.

### UI 번역

`locales/en.json`은 한국어 원문 키와 영어 번역의 공통 목록입니다. 문구와 번호 자리표시자(`{0}` 등)를
추가·수정한 뒤 `python3 scripts/generate_locales.py`로 Kotlin 목록을 갱신합니다.
Rust는 같은 JSON을 바이너리에 포함합니다. `--check`는 누락된 키·번역·자리표시자와 생성 파일 일치를 검사합니다.
표시 문구는 `t`, 인수가 있는 문구는 `tr!`(Rust)/`tr`(Kotlin)를 사용하며 완성된 사용자 문자열에 번역을 적용하지 않습니다.
언어 선택은 `preferences.language`의 `system|ko|en`이며 예전 설정과 알 수 없는 값은 `system`으로 해석합니다.
tmux 명령·에러 코드·원격 터미널 바이트는 언어와 무관하게 유지합니다.

### UI 아이콘

공통 SVG 원본과 도형 규칙은 [UI 아이콘 안내](../assets/ui-icons/README.md)에 있습니다.
`scripts/generate_ui_icons.py`로 Compose `ImageVector`와 Rust 포함 목록을 생성합니다.
`--check`는 SVG 형식과 생성물 일치를 검사합니다. 일반 빌드에는 생성 도구·외부 SVG 파일이 필요하지 않습니다.

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

### 앱 버전 관리

루트 `Cargo.toml`의 `[workspace.package].version`이 단일 기준입니다. 현재 `0.1.0`을 유지합니다.
Rust 코어·데스크톱은 workspace 버전을 상속하고 표시에는 빌드 시 `CARGO_PKG_VERSION`을 사용합니다.
Gradle은 같은 값을 Android `versionName`과 `app/build/generated/appVersion/`의 Kotlin `APP_VERSION`으로 생성합니다.
생성 디렉터리는 Git에 추가하지 않습니다. macOS 번들 두 버전 필드·`build.json`과 Windows ZIP의
`build.json`도 Cargo 메타데이터를 사용합니다.

배포 시 workspace 버전을 수정하고 Android `androidApp/build.gradle.kts`의 `versionCode`를
이전 배포보다 큰 정수로 올립니다. 현재 코드는 `1`입니다. APK와 필요한 데스크톱 패키지를 다시 빌드한 뒤:

```sh
python3 scripts/verify_app_versions.py --android --mac
# Windows 네이티브 패키지를 만든 환경에서는 --windows
```

플랫폼을 생략하면 생성 Kotlin·Cargo 메타데이터·원본 Debug APK 버전을 비교합니다. `--android`·`--mac`·
`--windows`를 조합해 필요한 플랫폼만 검사할 수 있습니다. macOS는 실제 `.app`과 메타데이터,
Windows는 ZIP 내부·외부 메타데이터를 비교하며 Android 도구가 필요하지 않습니다. 실제 네이티브 실행 검증은 별도입니다.
README의 현재 버전과 Git 제외된 `.dashboard-props.json`의 프로젝트·파일별 버전/빌드 번호도 대조·갱신합니다.

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
기본은 빌드만, `-CheckOnly`는 필수 도구 점검만, `-Test`는 단위 검사, `-Run`은 빌드 후 앱 실행입니다.

### 앱 아이콘 갱신

공통 도형·디자인 명세·크기별 미리보기와 변환 절차는 [앱 아이콘 안내](../assets/app-icon/README.md)에 있습니다.
일반 빌드는 저장된 리소스를 사용합니다. 갱신할 때만 Python 가상환경에 Pillow 11.3.0을 설치하고
`scripts/generate_app_icons.py`로 플랫폼별 리소스를 생성합니다. `--check`로 파일 일치를 확인한 뒤
Android Debug·lint와 macOS 패키징을 확인합니다. Windows는 기존 SDK `rc.exe`로
실행 파일의 아이콘 ID `1`을 컴파일하며 네이티브 빌드·GUI 확인은 Windows에서 별도로 수행합니다.

## 선택 검사

격리 SSH 검사는 OpenSSH·임시 키·독립 tmux socket을 사용하며 기존 사용자 서버·키·tmux 설정을 사용하지 않습니다.
tmux 소스 빌드에는 C 컴파일러·make·libevent·ncurses 개발 헤더가 필요합니다(`build_test_tmux.py`는 `3.3a`·`3.7c`).
`integration.py`의 기본 명령은 Rust 엔진의 `ssh_integration` ignore 검사 전체이며 CI `verify_ssh`와 같습니다.
에이전트 관측 검사가 포함되므로 기본 명령은 `--agent-home` 없이 실행하면 실패합니다.
개별 Rust 검사·JVM 참조·Android 실행은 `--command`로 지정합니다. `--command` 이후 인수와
Cargo의 `--` 구분자는 자식 명령에 그대로 전달합니다.

```sh
python3 scripts/build_test_tmux.py 3.7c
python3 scripts/integration.py --agent-home --tmux .tools/tmux-3.7c/tmux-3.7c/tmux
python3 scripts/integration.py --tmux .tools/tmux-3.7c/tmux-3.7c/tmux --command cargo test --locked -p dgl-core --test ssh_integration isolated_auth_management_input_reconnect -- --ignored --exact
```

개별 선택 이름은 `isolated_auth_management_input_reconnect`, `workspace_focus_tree_and_independent_connections`,
`hierarchy_titles_all_sessions_without_screen_reset`, `resume::healthy_resume_and_blackhole_recovery_preserve_work_without_replaying_input`처럼
`crates/dgl-core/tests/ssh_integration*`의 함수 경로를 사용합니다.

JVM 참조 엔진의 SSH 회귀가 필요할 때만 아래처럼 실행하고 `ssh` 모드 검증기로 결과를 대조합니다.

```sh
python3 scripts/integration.py --tmux .tools/tmux-3.7c/tmux-3.7c/tmux --command ./gradlew :app:desktopTest --rerun-tasks
python3 scripts/verify_test_results.py --mode ssh --output artifacts/jvm-ssh-tests.json
```

### Android 계측

전용 에뮬레이터와 명시적인 `ANDROID_SERIAL`을 사용하며 API36 한 대로 짧게 확인합니다. 전체 API matrix로 확장하지 않습니다.
앱·계측 APK를 먼저 빌드합니다. 계측 APK는 `androidApp/build/outputs/apk/androidTest/debug/androidApp-debug-androidTest.apk`입니다.
선택 이름은 `am instrument -e class` 형식(쉼표로 구분한 `kim.jujin.dglc.Class` 또는 `kim.jujin.dglc.Class#method`)입니다.
실행 스크립트(`rust_android_integration.py`·`workspace_android.py`·`mobile_input_android.py`)는 `scripts/verify_android_results.py`로
선택한 이름의 완료·실패·skip을 대조합니다. `am instrument`를 직접 실행한 결과는 이름 목록과 대조되지 않습니다.
Android 테스트 이름을 바꾸면 이 검증기의 명시적 목록도 갱신합니다.

API36 후보 검사는 SSH fixture 안에서 실행합니다. 기본 선택은 `AndroidPlatformTest`·`AndroidAppFlowTest`·
`AndroidVolumeKeyTest`·`AndroidHierarchyTest`·`AndroidMobileInputTest`이며,
`DGL_ANDROID_TEST_CLASSES`로 바꿀 수 있습니다(예: `kim.jujin.dglc.AndroidConnectionLifetimeTest`,
`kim.jujin.dglc.AndroidOutputRecoveryTest#tabRebindAndWindowSwitchWaitForFreshVisiblePanes`).
`AndroidAppFlowTest`·`AndroidConnectionLifetimeTest`·`AndroidOutputRecoveryTest`는 `dgl.fixture` 인수가 없으면 skip되며
검증기는 이를 실패로 판정하므로 이 경로로만 실행합니다. 결과는 `artifacts/rust-candidate/android/`에 남습니다.

```sh
./gradlew :androidApp:assembleDebug :androidApp:assembleDebugAndroidTest
ANDROID_SERIAL=emulator-5554 python3 scripts/integration.py --command python3 scripts/rust_android_integration.py
```

작업 공간·모바일 입력·볼륨키·계층은 `workspace_android.py`로 검사합니다. `ANDROID_SERIAL`이 `emulator-`로 시작하는 API36이어야 하며,
화면 크기·밀도·IME·글꼴·애니메이션 설정은 `finally`에서 복원합니다. 결과는 `artifacts/android-workspace/`에 남습니다.
`--section workspace`, `--section mobile-input`, `--section compact`로 필요한 구간만 재검사할 수 있습니다.

```sh
./gradlew :androidApp:assembleDebug :androidApp:assembleDebugAndroidTest :androidApp:lintDebug
ANDROID_SERIAL=emulator-5554 python3 scripts/workspace_android.py
```

fixture가 필요 없는 메서드는 APK 설치 후 직접 실행할 수 있습니다. 자주 쓰는 선택:

| 범위 | 선택 |
|---|---|
| 하단 CLI 메뉴 | `AndroidMobileInputTest#cliLauncherKeepsImeAndSurvivesResizeWithoutLeakingInput`, `AndroidVolumeKeyTest#cliLauncherCommandsLayoutDismissalAndTargetChanges` |
| 키보드 항상 보이기 | `AndroidMobileInputTest#alwaysVisibleKeyboardRespectsEditorsLifecycleAndActiveTerminal` |
| 터미널 너비·터치 스크롤 | `AndroidPlatformTest#bundledRendererRemoteColumnsAndTouchScrolling`, `AndroidPlatformTest#windowPanesKeepIndependentScreensAndOnlyTheContainerRequestsSize` |
| IME 프레임 시간 | `AndroidMobileInputTest#imeTransitionsRecordFrameDurations` (결과 `cache/ime-frames.json`) |

```sh
adb -s "$ANDROID_SERIAL" shell am instrument -w -r -e class kim.jujin.dglc.AndroidMobileInputTest#imeTransitionsRecordFrameDurations kim.jujin.dglc.test/androidx.test.runner.AndroidJUnitRunner
adb -s "$ANDROID_SERIAL" shell run-as kim.jujin.dglc cat cache/ime-frames.json
```

IME 프레임 결과는 앱 창의 처리 시간 진단이며 SurfaceFlinger 표시 지연·전체 성능 인수로 해석하지 않습니다.
로그 태그 `DglPresentationTest`의 `elapsedMs`는 요청부터 복원 완료 콜백까지이며 첫 표시 프레임 측정값이 아닙니다.
Debug 빌드의 복귀·구독/연결 세대·화면 ACK·입력 허용 진단은 `adb -s "$ANDROID_SERIAL" logcat -s DglResume`으로 확인합니다.
에뮬레이터 결과는 삼성 키보드 실기·물리 기기의 장시간 잠금/Doze 확인을 대신하지 않습니다.

성능·자원 측정은 [PERFORMANCE.md](PERFORMANCE.md)의 표본·종료 조건을 먼저 정합니다.
에이전트 실제 CLI 검사에는 설치·로그인과 경우에 따라 유료 호출이 필요하므로 [AGENT_STATUS.md](AGENT_STATUS.md)의 범위를 따릅니다.

## 의존성·배포 검사

```sh
./gradlew writeDependencyInventory
python3 scripts/audit_dependencies.py
python3 scripts/verify_licenses.py
python3 scripts/verify_lint.py
python3 scripts/collect_artifacts.py
python3 scripts/scan_secrets.py
```

`audit_dependencies.py`는 Gradle inventory·npm lock과 함께 Cargo.lock의 crates.io 패키지 전체를 OSV에 대조하므로
Rust 툴체인이 필요합니다. runtime은 배포 타깃별(Android 네 ABI의 `dgl-uniffi`, macOS·Windows의 `dgl-desktop`)
`cargo tree -e normal,no-proc-macro` 결과이며, 나머지는 build/test로 분류합니다. 예외는 `security/dependency-exceptions.json`입니다.
`collect_artifacts.py`는 빌드된 Debug·Release APK를 확인하고 `artifacts/build-manifest.json`, `artifacts/SHA256SUMS`를 생성합니다.
해시 목록의 APK 경로는 저장소 루트 기준이며, 루트에서 `sha256sum -c artifacts/SHA256SUMS`로 확인합니다.
`collect_rust_android.py`는 원본 Debug APK의 서명·Rust payload를 확인하고
`artifacts/rust-candidate/android/build.json`에 기록합니다. 두 도구 모두 APK를 복사하지 않습니다.
`scan_secrets.py`는 Gitleaks가 필요합니다. 잠금·검증 메타데이터·라이선스 갱신은 생성 결과를 검토하고 반영합니다.

## CI

`main` push/PR의 **Build and verify**는 Linux 기본 job 하나를 실행합니다.
Python·Rust 코어/UniFFI·web·JVM 단위 검사, Android Debug 빌드·lint, Wrapper·비밀정보·번들 검사를 포함합니다.
GPUI 데스크톱(`dgl-desktop`) 검사는 위 macOS 로컬 명령, Windows는 `build_windows.ps1 -Test`로 별도 실행합니다.
`README.md`·`TASKS.md`·`AGENTS.md`·`docs/**`·`scripts/README.md`만 바뀌면 CI를 생략합니다. Debug APK는 `android-debug` artifact로 수집합니다.

| 수동 입력 | 추가 작업 |
|---|---|
| `verify_packaging` | Release·계측 APK, Gradle·Cargo·npm 의존성 감사, 라이선스·배포 검사 |
| `verify_ssh` | Rust 엔진 SSH/tmux 3.3a·3.7c 통합 검사, 로그를 artifact로 수집 |
| `verify_android` | Android API 34·35·36 계측 |

입력은 기본 꺼짐입니다. **Build Windows Rust candidate**는 별도 수동 workflow이며
Windows MSVC 빌드·ZIP만 수행합니다. CI 성공은 서명·공개 릴리스·Windows 실기 인수 완료를 뜻하지 않습니다.
