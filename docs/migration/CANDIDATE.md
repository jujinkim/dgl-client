# Mac·Android Rust 후보 구현·검증 기록

2026-10-24, `main` 작업 트리. 기존 미커밋 R1 작업을 보존했고 새 브랜치·커밋·원격 CI를 만들지 않았다.
**후보 구현과 개발용 산출물은 준비됐으나 전체 완료 판정은 대기다.** Mac 실제 GUI·Keychain과
아래 남은 기능 화면 인수는 통과 처리하지 않는다. 후속 지시 “legacy는 불필요”에 따라 Android는 Rust 단일 엔진이다.

2026-10-24 추가 지시: Windows 실기 검증은 제외하되 빌드 환경은 제공한다.
Rust 데스크톱에 Windows 플랫폼 분기·DPAPI와 MSVC x64 빌드·Release ZIP·수동 CI를 추가했다.
Windows 네이티브 빌드/실행 결과를 Mac 검증으로 대신하지 않는다. [재현 절차](../WINDOWS.md).

## 구현과 모듈

| 위치 | 역할 |
|---|---|
| `crates/dgl-core` | 호환 모델/설정, 120 fixture 소비, 바이트 파서, 형식화 명령, russh 인증·tmux·PTY, 연결 actor·복구·제한 큐 |
| `crates/dgl-uniffi` | NativeConnection·키 생성/공개키 추출, typed byte output, JSON 상태/대상/명령·완료 결과, stop/shutdown/close |
| `desktop` | GPUI 호스트·연결 탭·tmux 계층/관리·설정, alacritty 터미널/IME/선택, macOS Keychain |
| `coreBindings` | 같은 Cargo.lock에서 생성한 Kotlin 바인딩·JNA·네 ABI `.so`, Rust 고지 assets |
| `mobileUi` | RustClientConnection·RustKeyTools와 명시적 네이티브 수명/출력 구독 관리 |
| `app`, `androidApp` | 공통 ClientConnection 인터페이스, 기존 Compose/xterm 재사용, 모바일 레이아웃·Rust 기본 실행·ViewModel |

주요 고정 버전: GPUI 0.2.2, alacritty_terminal 0.26.0, russh 0.64.1, UniFFI 0.32.2,
Tokio 1.50.0, JNA 5.18.1. Cargo.lock과 Gradle lock/checksum을 포함한다.
새 AGP library marker/JNA AAR 체크섬은 공식 배포 원본과 다시 대조했다.

호스트 추가/편집/삭제·즐겨찾기/최근·독립 연결, 비밀번호/키 인증, 키 가져오기/생성/공개키,
최초 지문 확인·변경 차단, tmux 세션/창/패널 관리와 삭제 확인, 일반 SSH fallback/진단,
복구·테마·폰트·단축키를 연결했다. Android는 기존 매크로·볼륨 키·IME·회전 경로를 재사용한다.
Mac은 한 선택 패널을 중앙에 표시하며 터미널 엔진에 적용한 뒤 출력 예산을 반환한다.

Android rust 레이아웃은 호스트/터미널 분리, 연결 선택기, tmux 하단 시트,
고정 저장/취소 영역이 있는 호스트·설정 전용 화면을 사용한다. 모바일 어댑터가 공통 화면을 재사용한다. 이전 JVM 연결 엔진·SSHJ/BC 키 처리는
`app/src/referenceTest`로 옮겨 회귀 검사와 격리 SSH 관리에만 사용한다. 앱 APK에는
포함하지 않으며 JVM 앱 진입점·Windows 설치물 작업과 CI는 제거했다.

## 계약과 저장 영역

- 입력은 호출 시 캡처한 connection/pane/generation으로만 전송한다. 복구 중 거절,
  오래된 세대 취소, 불확정 전송의 자동 재전송 금지를 유지한다.
- 출력은 연결당 256 KiB, 이벤트당 16 KiB, 미확인 이벤트 256개다. Android는 xterm write
  callback, Mac은 alacritty 적용 후 ack한다. 구독 교체/종료는 예산을 반환한다.
- 요청은 대기 32개/실행 1개, payload는 요청당 64 KiB다. 결과를 소비하지 않으면 슬롯을
  반환하지 않아 결과 큐도 제한된다. 종료 후 남은 요청의 취소 결과를 조회할 수 있다.
- Android ViewModel이 연결을 소유한다. WebView 교체는 새 구독과 snapshot을 사용한다.
  어댑터는 stop → coroutine join → native shutdown → generated close 순서로 해제한다.
  [UniFFI의 명시적 close 지침](https://mozilla.github.io/uniffi-rs/latest/kotlin/lifetimes.html)을 따른다.
- Mac 설정: `~/Library/Application Support/dgl-rust-candidate-v1/settings.json`.
  테스트 시 `DGL_CANDIDATE_DIR`로 변경할 수 있다. Keychain service는
  `kim.jujin.rovessh.rust-candidate.v1`이다.
- Android 설정/암호문: 앱의 `noBackupFilesDir/rust-candidate-v1` 아래.
  앱 ID `kim.jujin.rovessh`, Keystore alias `kim.jujin.rovessh.credentials.v1`을 유지한다.
  지문은 보호 저장 성공 후 설정 참조와 일치해야 사용한다. 평문 fallback은 없다.
- 기존 사용자 설정/암호문의 이전·덮어쓰기는 수행하지 않는다. Android 기본 실행은 Rust다.
  같은 ID이므로 Android 후보 설치는 설치된 앱 바이너리를 교체한다. 저장 영역은 분리된다.

## 재현 명령과 산출물

전제: Rust/Cargo, Android SDK 36/NDK `29.0.14206865`, 네 Android Rust target,
Java 17. 현재 검증 환경은 macOS 26.6.2/M1, Xcode 26.6, Rust/Cargo 1.98.1,
API36 ARM64 AVD다. Mac은 Metal Toolchain 설치 후 빌드했다.

```sh
rtk proxy cargo test -p dgl-core -p dgl-uniffi --locked
rtk proxy env DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer cargo clippy -p dgl-core -p dgl-uniffi -p dgl-desktop --all-targets --locked -- -D warnings
rtk proxy python3 scripts/package_rust_mac.py
rtk proxy open 'artifacts/rust-candidate/mac/dgl Rust Candidate.app'
```

Mac 산출물은 `artifacts/rust-candidate/mac/dgl Rust Candidate.app`이다. 기본은 debug ARM64;
`--release`는 선택 빌드이며 이번 정식 성능 후보가 아니다. 로컬 ad-hoc 서명과
`codesign --verify --strict`만 확인했다. Developer ID·공증·배포 설치 인수는 미완료다.
`build.json`에 프로필·아키텍처·실행 파일 SHA-256을 기록한다.
원시 검사 로그와 후보 소스 해시는 `artifacts/rust-candidate/verification/`에 보존했다.

```sh
rtk proxy env JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home ANDROID_HOME=/Users/jujin/Library/Android/sdk ./gradlew :androidApp:assembleDebug :androidApp:assembleRelease :androidApp:assembleDebugAndroidTest :androidApp:lintDebug
rtk proxy env JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home python3 scripts/collect_rust_android.py
rtk proxy env JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home ./gradlew :app:desktopTest :androidApp:assembleDebug
```

Rust APK는 기본 경로인 `androidApp/build/outputs/apk/debug/androidApp-debug.apk`, 전달용
복사본은 `artifacts/rust-candidate/android/dgl-rust-debug.apk`다. 엔진 선택 옵션은 없으며
`-PdglEngine`은 명확한 오류로 중단한다. 후보는 개발용 debug 서명,
버전 `0.1.0`/code `1`, min SDK 34이며 네 ABI의 Rust 라이브러리를 포함한다.
계측 APK는 `androidApp/build/outputs/apk/androidTest/debug/`에 있으며 사용자 전달 APK와 구분한다.

Rust 고지/의존성 inventory는 Mac Resources와 Android `assets/third-party-rust`에 들어간다.
수집기는 registry에 포함된 라이선스 원문과 manifest를 복사한다. 원문이 없는 항목(Mac 전체 의존 그래프 805개 중 122개)을 명시적으로
기록하며 공개 배포 라이선스 감사를 완료했다고 주장하지 않는다. GPUI text input 예제의 고지와
Apache 원문도 별도로 포함한다.

## 자동 검사 결과

| 검사 | 실제 결과 | 범위/제한 |
|---|---|---|
| Rust 기본 검사 | 8개 통과 | 기존 7개 + UniFFI 키 생성/공개키 roundtrip·잘못된 키 오류 1개 |
| fixture 120건 | 통과 | 기존 fixture 변경 없이 파서 모든 byte split·단일 바이트, escape·명령·입력·설정·상태 비교 |
| tmux 3.3a | 격리 OpenSSH 검사 통과 | 지문 unknown/changed, Ed25519/RSA/암호화 키, 잘못된 키, 이름 인용, snapshot/live, split, 오래된 입력, 실제 TCP 단절·자동 복구, 가득 찬 출력 큐에서 종료, 독립 연결 3회 반복·같은 PID |
| tmux 3.7c | 같은 검사 통과 | 로그인 PATH 탐지, 세션 삭제 후 직접 새 세션 생성/선택, 자동 대체 없음 포함 |
| Android API36 | 계측 7개 통과 | Keystore roundtrip/tamper, 합성 IME, Rust 신뢰/연결·세 키 형식의 공개키 추출, 하단 시트 split/창 생성, 볼륨 매크로, 렌더러 교체·회전·복귀 |
| Mac build | 통과 | ARM64 debug `.app`, local ad-hoc 검증 |
| Android build | 통과 | 옵션 없는 Debug·Release·계측 APK; 네 ABI·Rust 클래스 존재, 기존 엔진/SSHJ/BC DEX 제외 |
| JVM 회귀 | 27 통과·예상 skip 9 | 테스트 소스로 격리한 이전 엔진과 공통 UI 인터페이스 회귀; Windows/격리 SSH 조건 skip 구분 |
| Rust clippy | 통과 | core/UniFFI/desktop all-targets, warnings denied |
| 대시보드 | JSON 검증 통과 | Mac/Android Rust 단일 실행·기본 APK 경로·문서 등록, JSON은 Git 제외 |

격리 SSH 검사는 아래처럼 실행한다. 테스트용 키/서버/tmux socket은 스크립트가 생성·정리한다.
기존 사용자 SSH/tmux를 사용하지 않는다. 기본 cargo test에서 integration/Keychain의 ignore는
통과가 아니며, 위 격리 SSH 행은 별도 명시 실행 결과다.

```sh
rtk proxy python3 scripts/integration.py --tmux .tools/tmux-3.3a/tmux-3.3a/tmux --command sh -c 'cargo test -p dgl-core --test ssh_integration --locked -- --ignored --nocapture'
rtk proxy python3 scripts/integration.py --tmux .tools/tmux-3.7c/tmux-3.7c/tmux --tmux-login-path --command sh -c 'cargo test -p dgl-core --test ssh_integration --locked -- --ignored --nocapture'
rtk proxy env ANDROID_HOME=/Users/jujin/Library/Android/sdk python3 scripts/integration.py --tmux .tools/tmux-3.7c/tmux-3.7c/tmux --command python3 scripts/rust_android_integration.py
```

Android 검사는 실행 중에만 글자 배율 1.3과 소프트 키보드 표시를 설정하고 원래 값으로 복구한다.
포트 reverse/임시 fixture도 정리한다. 최종 실행 후 reverse가 비어 있고 글자 배율이
1.0으로 복구된 것을 확인했으며, 작업에서 기동한 Dgl_API36 에뮬레이터를 종료했다. 이전 후보 실행은 `OK (5 tests)`, 28.988초였으며 Rust 단일 엔진 전환 후 재검증은 `OK (5 tests)`, 53.583초로 통과했다. 화면 캡처는 WebView visual-state callback과
실제 터미널 글자 표시를 확인한 후 남긴다. 편집 화면 저장 버튼의 키보드 위 위치도 검사한다.
`api36-instrumentation.txt`와 `rust-light.png`,
`rust-dark.png`, `rust-editor.png`를 후보 산출물 디렉터리에 남긴다.

## 화면 확인과 남은 인수

Android의 밝은/어두운 기본 화면, 긴 호스트 이름, 글자 확대, 키보드 위 편집 저장/취소를
짧게 확인했다. 자동 조작은 actual Compose/WebView/SSH 경로를 사용한다. 합성 IME는
물리 한글·일본어 키보드 후보창/조합 인수와 구분한다.

Mac 후보 프로세스는 이벤트 루프에 진입했으나 computer-use에서 해당 앱 창을 찾지 못했다
(`cgWindowNotFound`). 이 결과를 GPUI 결함이나 GUI 통과로 판정하지 않는다.
실제 Keychain roundtrip 검사도 쓰기에서 OSStatus `-25308`로 실패했다. 평문 fallback 없이
저장 실패를 전달하며, 잠금 해제된 사용자 세션에서 저장→종료→재기동을 다시 확인해야 한다.

남은 필수 인수:

- Mac 실제 GUI의 연결 전 과정, 한글/일본어 IME, 선택/복사/paste/스크롤,
  대표 TUI·탭 전환·크기 변경·Keychain 저장 후 재기동.
- 두 UI의 호스트 추가/편집/삭제, 가져온 키의 공개키 복사, 여러 독립 연결,
  모든 tmux rename/resize/delete 확인과 취소를 실제 화면으로 끝까지 확인.
- 빈 목록·인증/저장 실패·복구 상태의 화면 확인, 더 좁은 화면/포커스/키보드 탐색.
- 순서/상한 테스트는 통과했지만 긴 연속 출력·여러 snapshot의 전체 중복/누락 인수와
  네이티브 수명 반복 연결 3회 검증을 장시간 soak 통과로 확대 해석하지 않는다.

성능 원시 결과와 unresolved는 [R1_BASELINE](R1_BASELINE.md)에 보존했다. 정식 성능,
실제 Codex/Claude, 장시간 soak, 전체 플랫폼 matrix/추가 원격 CI, 사용자 데이터 이전,
정식 배포는 이번에 재개하지 않았다. Android 기본 실행 교체와 이전 엔진의 프로덕션 제외는
사용자 후속 지시에 따라 완료했다.

## Rust 단일 실행 전환 (2026-10-24 후속 지시)

- `settings.gradle.kts`는 `coreBindings`/`mobileUi`를 항상 포함하고 `dglEngine`을 거절한다.
  `EngineFactory`는 main source에 하나만 있으며 Rust 연결·키 도구를 주입한다.
- SSHJ·Bouncy Castle·SLF4J는 JVM/Android **테스트 의존성**이다. 키 생성과 RSA/Ed25519/
  암호화 개인키의 공개키 추출은 Rust 코어를 통해 수행한다.
- APK 수집기는 Debug/Release 모두 네 ABI와 Rust 구현 클래스, 이전 엔진·SSHJ·BC의
  부재를 검사한다. 보조 DEX에 포함돼도 실패하며 이를 검사하는 Python 회귀 5개를 추가했다.
- 이전 Windows 설치물 CI를 제거하고 현재 Android CI 작업에 필요한 Rust 1.98.1·NDK를
  설정했다. 원격 실행이나 matrix 확대는 하지 않았다.
- 기존 데이터는 그대로 보존하며 후보 전용 저장 디렉터리를 유지한다. 이전 앱으로의
  자동 fallback 또는 사용자 데이터 마이그레이션은 없다.

검증 로그: `artifacts/rust-candidate/verification/rust-only-*`.

후속 검증 결과: Rust 8개(120 fixture 집계 포함), JVM 27개/예상 skip 9개,
Python 87개, 최종 API36 계측 7개(50.186초) 통과. Debug·Release·계측 APK 빌드와 Rust clippy 통과.
lint의 새 알림 2개는 기존 SSHJ/SLF4J를 테스트 의존성으로 옮기면서 드러난 버전 안내이며,
정확한 메시지·테스트 전용 범위·재검토 기한을 기록했다. BC 런타임 예외는 제거했다.
JNA AAR의 내장 라이선스가 기존 JNA 고지와 같은 것을 확인해 해시·출처를 등록했다.
Gradle 의존성 inventory도 이전 JVM 앱을 테스트 범위로 분류하며 Android runtime에는
SSHJ/BC/JediTerm/SLF4J가 없음을 확인했다. Android AAR에 딸려오는 이전 ABI는
앱의 네 ABI 필터로 제외한다. Rust 공개 배포 라이선스 전체 감사는 여전히 후속이다.

최종 API36 검사는 기본 5개에 볼륨 키 편집 저장/취소·순서·유효성 검사와
셸/대화상자/복구/재생성 입력 차단 검사 2개를 포함한다. 이전 `tmux 접기` 조작은
모바일 하단 시트 열기/닫기로 바꾸고 시트가 열렸을 때 입력이 차단됨을 확인했다.
최종 네 ABI APK로 7개 모두 통과했고, reverse·글자 배율·키보드 설정과 임시 fixture를
정리했다. 이번 실행에서 생성한 Dgl_API36은 종료했다.
