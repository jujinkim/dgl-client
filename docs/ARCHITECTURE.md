# 현재 아키텍처

DGL은 공통 Rust SSH/tmux 코어에 Android와 데스크톱 UI를 연결합니다.
Android는 Compose + WebView/xterm.js, macOS·Windows는 GPUI + alacritty_terminal을 사용합니다.
iOS/iPadOS 앱·Swift/Xcode 타깃은 이 저장소에 없으며 연결 수명·IME 동작도 구현되어 있지 않습니다.
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
앱 ID와 Kotlin 제품·테스트 패키지의 루트는 `kim.jujin.dglc`입니다.
소스 경로·import·계측 실행명·검증기·생성 번역 목록도 같은 패키지를 사용합니다.
모듈 리소스 namespace는 `.shared`·`.mobile`·`.bindings` 접미사를 둡니다. UniFFI 생성 패키지는 `uniffi.dgl_core`입니다.
기존 Keystore 키 별칭·Keychain 서비스·DPAPI entropy의 예전 식별자는 암호화 데이터 호환성을 위해 유지합니다.
이 저장 식별자를 패키지와 함께 바꾸면 업데이트 후 저장된 자격 증명을 읽지 못합니다.

Android는 `noBackupFilesDir/rust-candidate-v1`, 데스크톱은
`desktop/src/platform.rs`가 정한 후보 디렉터리를 사용합니다. 이전 앱의 데이터는 자동 이전하지 않습니다.
`workspace-layout.json`은 각 기기의 그룹 구조·비율만 저장하고 재실행 시 연결은 비워 둡니다.
`host-order.json`은 데스크톱 호스트 ID 순서·연결 시작 구간·수동 배치 여부만 기기에 저장하며 동기화하지 않습니다.
동기화 허용 필드와 민감정보 제외는 [CLOUD_SYNC.md](CLOUD_SYNC.md)를 따릅니다.

## 참조 코드와 생성 파일

`app/src/referenceTest`는 JVM 테스트·Android 테스트 APK에서만 컴파일합니다.
SSHJ/Bouncy Castle 연결 엔진과 이 엔진의 tmux 명령 문자열(`TmuxWire`)·control mode 파서는 제품 APK에 포함하지 않습니다.
제품의 `TmuxCommand`는 Rust 코어에 넘길 조작 요청만 만듭니다. JediTerm은 JVM 렌더링 회귀 검사에만 사용합니다.
Rust APK 수집기는 네 ABI와 필수 Rust 클래스를 확인하고 이전 엔진이 제품 DEX에 있으면 실패합니다.

`coreBindings/src/main/kotlin/uniffi`, `coreBindings/src/main/jniLibs`와 빌드 출력은 생성물입니다.
Android의 `DglApplication`이 `AppModel`/Rust 연결을 소유하고 Activity와 `ConnectionService`가 공유합니다.
`specialUse` 포어그라운드 서비스는 사용자 시작 SSH 유지 용도를 명시하며 `START_NOT_STICKY`로 실행합니다.
홈 이동·잠금 중 통신에 partial wake lock을 사용하고, 10분 제한을 유지 수명 안에서 5분마다 갱신합니다.
설정 중단·연결 종료·재시도 소진·서비스 파괴·최근 앱 제거 시 wake lock과 갱신 작업을 해제합니다.
`MainActivity.onStart/onStop`의 실제 표시 전환을 프로세스 런타임에서 판정해 각 연결에 `checkConnection`을 전달합니다.
회전은 표시 상태를 유지합니다. 중복 검사는 합치고 검사·접속 중에는 중복 시작하지 않습니다.
포커스·IME 변경은 검사 계기가 아닙니다. 별도 exec 채널의 `:` 응답을 3초 제한으로 검사하며,
터미널 명령·출력 역압력과 독립적으로 실패를 판정합니다. 정상 응답 후 actor는 목록·대상을 확인하고
변경·일시정지된 pane만 캡처합니다. 새 WebView 구독·실제 재접속에는 전체 캡처를 유지합니다.
Rust `checking`을 Android 어댑터가 검사 제출 직후 동기적으로 반영합니다. 입력 차단·오래된 입력 취소와 함께
첫 복귀 화면에 45% dim·`연결 확인 중…`을 표시하며, 정상 확인 완료 후 즉시 해제합니다.
실제 재접속·출력 구독 실패에는 이전 화면·45% dim을 유지하고, 현재 구독/복구 주기/세대의 모든 pane이
렌더링 완료를 알려야 입력과 dim을 해제합니다. 일반 SSH도 새 셸의 초기 렌더링을 기다립니다.
렌더링 timeout은 의도적인 coroutine 취소와 구분하고 종료된 작업은 출력 구독을 해제합니다.
구독 교체·연결 교체는 이전 렌더러 작업의 정리 완료를 기다리며, ACK·정리는 소유 구독과 연결 세대를 검사합니다.
tmux 출력 오류 뒤 복귀는 SSH를 유지하고 구독·스냅샷만 한 번 자동 복원합니다. 실패하면 수동 재시도를 제공하며
일반 셸에는 이 방식을 적용하지 않습니다. Debug의 `DglResume` 로그에는 단조 시각·구독/세대·검사·화면 ACK·입력 허용만 기록합니다.
유지 설정을 끄면 `ConnectionLifetime`이 각 어댑터의 `suspendForBackground()`를 호출합니다.
`BackgroundResume`은 자동 중지한 연결의 직전 세션·pane을 메모리에만 보관하고 `onResume()`에서 한 번 소비합니다.
사용자 종료·탭 닫기·재시도 정지·task 제거는 예약을 취소하며 인증/호스트키 실패·삭제된 세션은 예약하지 않습니다.
백그라운드 자동 중지는 서비스·wake lock을 해제하며 프로세스 재실행으로 예약을 복원하지 않습니다.
Android·데스크톱 재시도는 네트워크 오류에 최대 3회, 첫 시도 즉시·이후 각각 1초 간격입니다.
수동 재시도는 새 3회 주기를 즉시 시작합니다. Android는 프로세스 표시 상태 전환으로 복귀를 판정하고,
데스크톱은 창 활성화 전환과 3초 이상의 UI tick 간격으로 전면·절전 복귀를 감지합니다.
Android 복귀 시 네트워크 소진은 새 주기를 시작하지만 사용자 정지는 유지합니다.
루트 뒤로가기는 [Android 기본 백그라운드 전환](https://developer.android.com/guide/components/activities/tasks-and-back-stack#back-press-behavior)을 사용합니다.
최근 앱 제거는 앱 모델·연결·관측/동기화 작업을 닫으며 원격 tmux에는 종료 명령을 보내지 않습니다.
Android 서비스 등록 기준은 [specialUse 문서](https://developer.android.com/develop/background-work/services/fgs/service-types#special-use),
IME 콜백은 [키보드 동기화 문서](https://developer.android.com/develop/ui/views/layout/sw-keyboard)를 따릅니다.

Android 터미널 assets는 `web/bundle.mjs`로 갱신하며 CI에서 소스와 번들 차이를 검사합니다.
기술 선택의 과거 경위는 [HISTORY.md](HISTORY.md)에 요약합니다.

## Android 작업 공간과 렌더러 소유권

`WorkspaceState`는 Kotlin의 순수 분할 트리·그룹별 탭·선택 모델이고 `AppModel`은 탭 ID와 Rust 연결 어댑터를 연결합니다.
호스트/세션 예약은 연결 시작 전에 이루어지며 알림·탐색기·모바일 시트가 같은 탭 경로를 사용합니다.
`ClientConnection.generation`은 지연된 대상 선택에 쓰는 읽기 전용 어댑터 세대입니다. Rust wire·UniFFI 형식은 유지합니다.

`App`은 창의 사용 가능한 너비 840dp를 기준으로 화면을 선택합니다. Android 입력 기능은
화면 배치와 별개이며 입력 도구·볼륨키는 활성 탭에만 등록합니다. 좁은 창의 하단 가로 탭은 전체 연결 목록을 사용하고 스크롤 상태는 선택 연결의 구성 수명 바깥에 둡니다. 알림은 버튼 기준 팝업이며 IME를 제외한 화면 공간으로 제한합니다. 여러 표면의 모달 입력 차단은 모델의 소유자 집합으로 공유합니다.
`AndroidTerminalResources`는 Activity 안에서 연결별 WebView를 보관합니다. 숨긴 표면은 unbind하고 입력·크기 요청을 막으며,
탭 닫기와 Activity 해제 때 WebView를 정리합니다. 프로세스 소유 Rust 엔진과 연결 수명은 그대로 유지합니다.
`WorkspaceRepository`는 별도 기기 로컬 `workspace-layout.json`에 그룹 구조·비율만 원자적으로 저장합니다.
