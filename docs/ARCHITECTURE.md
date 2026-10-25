# 현재 아키텍처

DGL은 공통 Rust SSH/tmux 코어에 Android와 데스크톱 UI를 연결합니다.
Android는 Compose + WebView/xterm.js, macOS·Windows는 GPUI + alacritty_terminal을 사용합니다.
이전 JVM 앱은 실행·배포하지 않으며 일부 Kotlin/JVM 구현은 회귀 검사와 격리 SSH fixture에만 남아 있습니다.

| 경로 | 역할 |
|---|---|
| `crates/dgl-core` | 호스트·설정·입력 모델, russh 연결, tmux 파서·관리·복구, 출력 예산, 파일 동기화·에이전트 관측 |
| `crates/dgl-uniffi` | 코어의 JSON 상태·명령과 바이트 출력에 대한 UniFFI 경계 |
| `coreBindings` | 생성 Kotlin 바인딩, Android 네이티브 라이브러리·저장소 콜백 |
| `mobileUi` | Rust 연결 어댑터·모바일 상태와 수명 관리 |
| `app` | 공통 Kotlin 모델·Compose 화면·Android 터미널 컴포넌트, JVM 참조 테스트 |
| `androidApp` | Android 진입점·Manifest·계측 검사 |
| `desktop` | GPUI 창·탭 그룹·입력·터미널·OS 보호 저장·알림 |
| `web` | Android 터미널 번들·브리지·단위 검사. 랜딩 웹사이트 소스가 아님 |
| `fixtures` | Rust/Kotlin 공통 계약·표시·에이전트 훅 fixture |
| `scripts` | 빌드·수집·검증·선택적 계측 도구 |

## 연결과 세션

연결마다 독립 SSH transport와 명령 actor를 둡니다. tmux Control Mode로 목록·배치를 읽고
형식화된 명령으로 조작합니다. 원격 상태의 기준은 tmux이며 앱 종료가 원격 세션 삭제를 뜻하지 않습니다.
지원 tmux가 없으면 일반 SSH PTY로 전환하고 세션 보존의 차이를 안내합니다.

같은 세션의 외부 클라이언트와 창·활성 pane 선택을 공유합니다. 선택 창의 모든 보이는 pane을
서버 좌표·열/행으로 표시하며, 입력 포커스 세대와 pane별 출력 세대를 분리합니다.
연결 복구는 서버 상태를 다시 읽고 화면을 복원한 후 입력을 허용합니다.
[코어 계약](CORE_API.md), [창 동기화](TMUX_WINDOW_SYNC.md), [복원 ADR](decisions/002-terminal-restoration.md)에 상세 규칙이 있습니다.

## 저장과 플랫폼 경계

Android Keystore, macOS Keychain, Windows CURRENT_USER DPAPI를 플랫폼 어댑터로 제공합니다.
보호 저장 실패 시 평문 저장으로 대체하지 않습니다. Linux 보호 저장과 패키징은 미구현입니다.
앱 ID는 `kim.jujin.dglc`; 내부 Kotlin namespace·기존 저장 키에 남은 `rovessh`는 앱 ID와 구분합니다.

Android는 `noBackupFilesDir/rust-candidate-v1`, 데스크톱은
`desktop/src/platform.rs`가 정한 후보 디렉터리를 사용합니다. 이전 앱의 데이터는 자동 이전하지 않습니다.
`workspace-layout.json`은 데스크톱 그룹 구조·비율만 저장하고 재실행 시 연결은 비워 둡니다.
`host-order.json`은 데스크톱 호스트 ID 순서·연결 시작 구간·수동 배치 여부만 기기에 저장하며 동기화하지 않습니다.
동기화 허용 필드와 민감정보 제외는 [CLOUD_SYNC.md](CLOUD_SYNC.md)를 따릅니다.

## 참조 코드와 생성 파일

`app/src/referenceTest`는 JVM 테스트·Android 테스트 APK에서만 컴파일합니다.
SSHJ/Bouncy Castle 연결 엔진은 제품 APK에 포함하지 않습니다. JediTerm은 JVM 렌더링 회귀 검사에만 사용합니다.
Rust APK 수집기는 네 ABI와 필수 Rust 클래스를 확인하고 이전 엔진이 제품 DEX에 있으면 실패합니다.

`coreBindings/src/main/kotlin/uniffi`, `coreBindings/src/main/jniLibs`와 빌드 출력은 생성물입니다.
Android 터미널 assets는 `web/bundle.mjs`로 갱신하며 CI에서 소스와 번들 차이를 검사합니다.
기술 선택의 과거 경위는 [HISTORY.md](HISTORY.md)에 요약합니다.
