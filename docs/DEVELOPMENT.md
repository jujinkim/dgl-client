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

검증기의 `linux` 모드는 macOS에서도 사용하는 비 Windows·비 SSH JVM 검사 모드입니다.
외부 SSH/Windows 환경이 필요한 JVM 검사는 예상 skip, Rust 통합/Keychain 검사는 기본 ignore입니다.
이 결과를 실기 검사로 보고하지 않습니다.

macOS 데스크톱까지 확인할 때:

```sh
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer cargo test --locked --workspace
```

데스크톱 변경만 검사할 때는 같은 환경에서 `cargo test --locked -p dgl-desktop`을 실행합니다.
호스트 순서 모델과 GPUI 드래그·세션 탭 재사용 검사를 포함하며 대화형 Keychain 검사는 기본 ignore입니다.

기본은 위 기능·단위 검사이며 전체 Android API 계측과 장시간 자원 측정은 별도 선택입니다.
검사 결과·남은 한계는 [VALIDATION.md](VALIDATION.md)에 기록합니다.

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
Debug APK는 `androidApp/build/outputs/apk/debug/androidApp-debug.apk`, 전달용 복사본은
`artifacts/rust-candidate/android/dgl-rust-debug.apk`입니다. Release APK는
`androidApp/build/outputs/apk/release/androidApp-release-unsigned.apk`이며 미서명입니다.
앱 ID는 `kim.jujin.dglc`, versionName/code는 `0.1.0` / `1`입니다.

## 데스크톱 패키징

```sh
python3 scripts/package_rust_mac.py
# release 프로필이 필요할 때
python3 scripts/package_rust_mac.py --release
```

출력은 `artifacts/rust-candidate/mac/dgl Rust Candidate.app`과 `build.json`입니다.
스크립트는 Xcode 경로를 설정하고 로컬 ad-hoc 서명·검증을 수행합니다. Developer ID·공증은 하지 않습니다.
저장된 `desktop/resources/dgl.icns`를 번들 Resources에 넣고 `CFBundleIconFile`로 연결한 뒤 서명합니다.
Windows는 `scripts/run_windows.cmd` 또는 `scripts/build_windows.ps1`로 빌드·ZIP을 생성합니다.
기본은 빌드만, `-Test`는 단위 검사, `-Run`은 빌드 후 앱 실행입니다.

### 앱 아이콘 갱신

원본·프롬프트·크기별 미리보기와 변환 절차는 [앱 아이콘 안내](../assets/app-icon/README.md)에 있습니다.
일반 빌드는 저장된 리소스를 사용합니다. 갱신할 때만 Python 가상환경에 Pillow 11.3.0을 설치하고
`scripts/generate_app_icons.py`를 실행합니다. `--check`로 파일 일치를 확인한 뒤
Android Debug·lint와 macOS 패키징을 확인합니다. Windows는 기존 SDK `rc.exe`로
실행 파일의 아이콘 ID `1`을 컴파일하며 네이티브 빌드·GUI 확인은 Windows에서 별도로 수행합니다.

## 선택 검사

격리 SSH 검사는 OpenSSH·임시 키·독립 tmux socket을 사용합니다. 기존 사용자 서버·키를 사용하지 않습니다.
`integration.py`의 기본 명령은 JVM 참조 검사이므로 Rust 검사는 `--command`를 지정합니다.

```sh
python3 scripts/build_test_tmux.py 3.7c
python3 scripts/integration.py --tmux .tools/tmux-3.7c/tmux-3.7c/tmux --command cargo test --locked -p dgl-core --test ssh_integration isolated_auth_management_input_reconnect -- --ignored --exact
```

tmux 소스 빌드에는 C 컴파일러·make·libevent·ncurses 개발 헤더가 필요합니다.
Android 계측은 전용 에뮬레이터와 명시적인 `ANDROID_SERIAL`을 사용합니다.
API36 후보 검사는 앱/계측 APK를 먼저 빌드하고 fixture 안에서 실행합니다.
연결 수명·IME·짧은 복귀·알림 인텐트·task 제거만 짧게 확인할 때는 아래 실행에
`DGL_ANDROID_TEST_CLASSES=kim.jujin.dglc.AndroidConnectionLifetimeTest`를 추가합니다.
전체 API matrix로 확장하지 않습니다.

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
활성 그룹 입력·전역 모달 차단·드래그 확정/취소를 검사합니다. 화면 크기·밀도·IME·글꼴·애니메이션 설정은
`finally`에서 복원합니다. 결과는 `artifacts/android-workspace/`에 남기며 명시적 검사 이름 목록과 대조합니다.
JVM 기본 검사에는 분할·이동·합치기·선택·최소 공간/최대 그룹·배치 저장·세션 예약·오래된 연결 대상 거부가 포함됩니다.
`--section workspace`, `--section mobile-input`, `--section compact`로 필요한 구간만 재검사할 수 있습니다.
전체 Android API matrix로 확대하지 않습니다.

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
`artifacts/dgl-client-0.1.0-debug.apk`, `build-manifest.json`, `SHA256SUMS`를 생성합니다.
`collect_rust_android.py`는 Debug 서명·Rust payload를 확인해 후보 전달 경로에 복사하는 도구입니다.
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
