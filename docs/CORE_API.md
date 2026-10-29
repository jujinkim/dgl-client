# 공통 코어 계약

현재 Rust 코어와 Android·데스크톱 어댑터의 계약입니다. 이름은 언어 중립 명세이며 실제 구현 매핑을 아래에 기록합니다.
`fixtures/migration/contract-v1.json`은 Kotlin/Rust가 함께 읽는 저장·프로토콜 호환 fixture입니다.

## 모듈과 공개 값

| 모듈 | 소유권 |
|---|---|
| `crates/dgl-core` | 순수 모델/설정/입력, SSH, tmux 파싱·명령·레이아웃, 연결·복구·출력 제어 |
| `crates/dgl-uniffi` | Android용 외부 객체(`NativeConnection`·`NativeAgentInbox`)·JSON/바이트 변환·`CoreError` 변환, async `shutdown`. 콜백 인터페이스는 없고 Kotlin이 polling한다 |
| `desktop` | GPUI 후보, 터미널 엔진/그리기/IME/포커스, OS 보안 저장 |
| `:coreBindings` | 빌드 산출물만 보유: `scripts/build_rust_android.py`가 만드는 생성 Kotlin 바인딩(`src/main/kotlin/uniffi`, JNA 로딩 포함)·`jniLibs`, `scripts/collect_rust_notices.py`가 만드는 Rust 고지 assets |
| `:mobileUi` | Rust 연결 어댑터(`RustClientConnection`·`RustAgentInbox`·`RustKeyTools`·`RustFileSync`·`RustLocalization`). Compose·ViewModel은 없다 |
| `:app` | 공통 모델·Compose 화면, Android WebView/xterm.js 렌더러(`androidMain`), Keystore 자격 증명 저장(`AndroidCredentialStore`)·설정 저장소; 이전 연결 엔진은 referenceTest 전용 |
| `:androidApp` | Activity·Application·연결 서비스·알림, 엔진 조립(`EngineFactory.kt`) |

코어에 Compose/GPUI/JNI 객체, OS 창, UI 표시 문구를 넣지 않는다. 모듈 경로가 없는 이름은
언어 중립 API 명세이고 실제 후보는 아래 매핑을 사용한다. 모듈 경로·`State` 필드로 적은 이름은 실제 Rust 타입이다.

- `ConnectionHandle`: 생성 후 재사용하지 않는 opaque ID. `close` 이후 모든 일반 호출은 `Closed`.
- `flow::Generation { connection, pane, size }`: 연결(`State.generation`)마다 하나인 u64 카운터 묶음이며 패널별 세대는 없다.
  시작값은 `{1, 0, 0}`이다.
  - `connection`: SSH 연결 시도·세션 하나가 끝날 때(재접속 전)와 종료 시 증가하며 이때 `pane`/`window`/`render_targets`를 비운다.
  - `pane`: 입력 포커스 세대다. 목록 갱신으로 활성 pane(`State.pane`)이 바뀔 때, 일반 shell 진입, control client를 닫고 MissingSession으로 전환할 때 증가한다.
  - `size`: 연결 전체 공용 카운터다. pane 화면을 캡처할 때마다(크기·모드 변경, 강제·일시정지 복원) 1 증가하고, 일반 shell resize도 증가시킨다.
- `flow::PaneTarget { connection, pane_id, generation }`: `connection`은 연결 핸들이다. 입력을 만들 때 `Connection::target()`
  (`State::input_target()`, 현재 포커스 pane + 현재 `State.generation`)을 캡처하며 송신 시 UI의 현재 포커스를 다시 조회하지 않는다.
- 렌더 target: `State.render_targets`(pane ID → PaneTarget)는 캡처 시점의 연결 세대, `pane: 0`, 증가한 `size`로 만든다.
  `pane` 필드를 항상 0으로 두므로 포커스 변경은 렌더 target을 무효화하지 않는다. 보이지 않게 된 pane의 항목은 제거한다.
  일반 shell은 `pane_id: "shell"` 하나이며 입력 target과 같다.
- `State::accepts_output(target)`: shell은 입력 target과 완전히 같을 때, 그 외는 `render_targets[pane_id]`와 같고
  단계가 Ready/Restoring일 때만 true다. UniFFI `NativeConnection.accepts_output(target_json)`이 같은 판정을 노출한다.
- `Command { requestId, operation }`: 세션/창/패널 생성·선택·이름 변경·분할·resize·삭제,
  새로고침, snapshot, `checkConnection`. raw shell/control 문자열을 일반 UI에 노출하지 않는다.
- `dgl_core::Error`: 필드 없는 enum이다. Authentication, HostKeyUnknown, HostKeyChanged, Network, Protocol,
  MissingSession, Storage, InvalidArgument, QueueFull, NotReady, StaleGeneration, Cancelled, Closed.
  JSON(`State.error`, 완료 결과 `error`)과 `Display`는 변형 이름 문자열이다. UniFFI는 이를 `CoreError::Failure { code }`
  (Kotlin `CoreException.Failure.code`)로 전달하며 `code`가 같은 변형 이름이다. 작업·재시도 여부·진단 필드는 없다.
- 진단은 오류와 별개인 `State.diagnostics`(`ssh::Probe { stage, exit_status, outcome, path, version }`)다.
  tmux 탐지 단계·종료 코드·결과·찾은 경로와 미지원 시 `tmux -V` 결과(최대 128자)만 담으며 비밀번호/키/터미널 출력/사용자 입력을 포함하지 않는다.
- `StateEvent { sequence, connectionGeneration, state }`: 연결 단계, 신뢰 요청,
  기본값 false인 `checking`, 서버 권위의 sessions/windows/panes/layout, 요청 완료/취소, 종료 완료.
- `OutputEvent { subscription, sequence, target, kind, size, bytes }`: Resize, ResetBegin,
  ResetChunk, ResetEnd, Live. byte buffer는 수신측 소유 불변 복사본이며 UTF-8 코드포인트가
  배치/이벤트 경계를 넘을 수 있다. Kotlin에서 임의로 문자열 변환하지 않는다.

`RemoteWindow.index`(정수 창 번호), `RemotePane.title`(터미널 제목)을 상태 JSON에 포함한다.
Rust·Android 어댑터는 같은 계약을 사용한다. 제목·프로세스 변경 구독은 상태만 갱신하며,
선택 패널의 크기·모드가 그대로이면 출력 Reset과 세대 변경을 발생시키지 않는다.
표시 문구·폴백은 데스크톱/Kotlin 표시 모델에서 결정한다. [계층 표시](SESSION_HIERARCHY.md)를 참고한다.

## 제목 갱신 표시 상태

`Connection::set_title_updates_enabled(bool)`와 UniFFI의 같은 메서드는 제목 빠른 조회를 켜고 끈다.
기본값은 false이며 설정·상태 JSON·동기화에는 저장하지 않는다. 출력 구독과 별개인 표시 신호다.
데스크톱은 전면 창의 각 그룹에서 선택된 연결을, Android는 앱 전면 상태와 실제 표시 연결 집합을 전달한다.
Android 어댑터는 네이티브 연결을 다시 만들 때도 최신 값을 적용한다.

활성 pane의 라이브 출력에서 조각난 OSC 0/2(BEL/ST 종료)를 감지하되 출력 바이트는 변경하지 않는다.
첫 신호는 즉시 `display-message`로 해당 pane 제목만 조회하고, 이후 조회 시작은 연결당 최소 500ms 간격·동시 1건이다.
대기 중 신호는 합치며 조회 중 들어온 마지막 신호도 후속 조회에 반영한다. 신호가 없으면 추가 타이머·통신을 만들지 않는다.
기존 전체 pane 조회를 시작하기 전에 관측한 신호는 그 조회로 처리한다. 조회 도중 새 신호는 보수적으로 남긴다.
표시값은 tmux의 실제 제목이며 전체 목록과 같은 인용·제어 문자 처리를 사용한다.
제목이 달라진 경우에만 해당 제목과 상태 sequence를 바꾸며 입력 대상·연결 단계·화면 snapshot은 바꾸지 않는다.

숨김·대상/연결 세대 변경은 감지 조각과 대기 신호를 폐기한다. 응답 적용 전 표시 revision·연결/pane 세대를 다시 확인한다.
이미 보낸 명령은 응답까지 읽고 오래된 결과만 버려 Control Mode 응답 순서를 보존한다.
조회 대상 소실은 기존 목록 갱신으로, 통신 단절은 기존 복구 경로로 처리한다.
숨김·백그라운드와 누락 보완은 기존 tmux 1초 format 구독을 사용한다. 원격 훅·설정·새 의존성은 추가하지 않는다.

## 명령과 입력

연결별 actor 한 개가 명령을 직렬화한다. 입력 요청 하나의 조각들은 해당 요청 내에서
연속 전송하며 snapshot/관리 명령이 중간에 끼지 않는다. tmux 입력은 최대 256바이트씩
hex 인코딩한다. 같은 대상·revision으로 대기 중인 입력 요청은 최대 16개를 묶고, 조각을
한 번에 최대 16줄씩 써서 응답을 함께 기다린다. 대상·허용 상태는 쓰기마다 다시 검사하며,
실패한 요청의 남은 조각은 보내지 않는다. 같은 쓰기에 이미 실린 조각은 tmux가 실행한다.
목록 갱신(`list-sessions`·`list-windows -a`·`list-panes -a`)과 그 앞의 선택·관리·크기 명령도
줄마다 별도 명령으로 한 번에 써서 한 번 왕복으로 처리한다. Control Mode가 없을 때의 목록은
태그를 붙인 세 목록을 `tmux` 한 번의 exec으로 받는다. 입력 한 요청은 UTF-8 변환 후 1..65,536바이트, 대기 요청은 32개 이하다.
이는 **요청별 64KiB, 32개 대기 슬롯**이며 기존 구현이 64KiB 총 대기 메모리를 보장했다는
의미가 아니다. 실행 중 요청 1개를 포함한 입력 payload 상한은 33×64KiB다.

`submitInput(target, bytes)`는 Accepted(requestId), InvalidArgument, QueueFull,
StaleGeneration, NotReady, Closed 중 하나를 즉시 반환한다. Accepted는 원격 처리 완료가
아니다. 실행 직전 세대를 다시 검사하고 완료 이벤트를 Sent/Cancelled/StaleGeneration/
TransportLost로 전달한다. 완료 결과는 `Completion { request_id, outcome, error }`이며 outcome은
Sent(입력)/Completed/Cancelled/StaleGeneration/TransportLost(Network)/Failed다. 연결 단절 시 전송 성공 범위를 알 수 없는 요청도
TransportLost로 끝내며 **자동 재전송하지 않는다**. 확인·복구 중 입력은 거절한다. 확인 시작 전 대기하던 입력도 취소하며 실패 입력을 재전송하지 않는다.
포커스가 바뀌어도 이미 수락한 입력은 원래 pane으로만 가거나 취소된다. 키·텍스트 입력은 `PaneTarget::same_input_destination`으로
연결 핸들·pane ID·`connection`/`pane` 세대만 비교하므로 크기 세대만 달라진 경우는 목적지 변경으로 취급하지 않는다.
이 검사는 제출 시, 실행 직전, 남은 조각을 쓰기 전마다 반복하며 불일치는 StaleGeneration이다.
`Restoring && resizing`인 동일 대상의 입력은 기존 큐로 받으며, 실제 연결 검사·재접속에는 받지 않는다.
좌표가 포함된 마우스 보고는 제출·전송 시 크기 세대까지 검사한다. 별도 입력 보관·재전송 큐는 없다.

취소는 requestId 기준으로 멱등적이다. 이미 보낸 바이트는 회수하지 않는다. 관리 명령의
불확정 원격 결과도 자동 반복하지 않고 refresh로 확인한다. UI의 삭제 확인 후에만
형식화된 삭제 명령을 제출한다.

## 출력·역압력·스냅샷

연결 전체에서 대기+전달 중인 **모든 pane 출력 payload 합이 262,144바이트 이하**다.
개별 전달은 최대 16,384바이트, 메타데이터 이벤트를 포함해 미확인 이벤트 256개 이하다.
구독별 unbounded channel, snapshot을 전부 별도 버퍼로 누적하는 우회는 금지한다.
Android 복구 화면은 원격 출력 버퍼와 별개로 마지막 렌더링 창의 DOM 사본 하나를 유지한다.
새 layout/Reset 전에 보관하고 현재 구독의 모든 pane 표시 완료 뒤 해제한다.
일반 pane 스냅샷의 취소·시간 초과와 복구 실패는 이 화면을 지우지 않는다.
프로토콜 파서의 별도 안전 상한(행 1MiB, 응답 4MiB)은 이 출력 예산과 구분해 계측한다.

수신측은 실제 터미널 엔진이 이벤트를 적용한 뒤 `ack(subscription, sequence)`한다.
전달 직후/FFI 복사 직후 ack하지 않는다. ack는 구독의 연속 sequence까지만 누적 가능하며
미전달 번호는 InvalidArgument, 이미 확인한 번호는 멱등 성공이다. ack된 바이트만 예산을
반환한다. 구독당 소비자 한 개. 느린 소비자는 SSH read까지 역압력을 전달한다.
tmux `pause-after`/pause 후에는 resume 전에 새 snapshot을 얻는다.

패널별 순서는 Resize → ResetBegin → ResetChunk* → ResetEnd → Live*다. 초기 크기를
적용하기 전에 reset/live를 그리지 않는다. snapshot 생성 중 live 출력은 같은 제한 예산에
포함하거나 원격에서 pause하고 snapshot 경계 이후로 재개한다. 누락/중복 여부는 연속
번호 fixture로 검사한다. snapshot 실패 시 부분 화면을 Ready로 발표하지 않는다.

resize 후 크기·모드가 달라진 보이는 pane만 다시 캡처하며, 캡처마다 연결 공용 size 카운터를 올려 그 pane의 렌더 target을
교체한다. 외부 split/kill/resize는 서버 레이아웃으로 상태를 갱신하고 영향받는 패널만 복원한다. 구독 해제/패널 삭제/새 세대에서 오래된 출력과
스냅샷 결과를 폐기하고 예산을 반환한다. 이미 UI로 넘어간 옛 버퍼도 세대 확인 후 적용하지
않고 ack/해제한다. 여러 패널 출력은 pane별 순서를 보존하며 tmux 제어 스트림 순서로 전달한다.

## 상태, 종료, Android 수명

상태와 출력 구독은 별개다. 상태 스냅샷은 최신 값으로 합칠 수 있지만 신뢰 요청·명령 결과·
종료 이벤트는 잃으면 안 된다. 명령 결과는 제한된 요청 슬롯과 함께 유지하며 슬롯이 찼으면
새 요청을 거절한다. 종료·취소·연결 확인은 데이터 큐와 별도 제어 경로/취소 토큰을 사용한다.
제어 요청에는 데이터 33슬롯과 별도로 3슬롯을 예약한다. 중복 연결 확인은 같은 request ID로 합친다.
`checkConnection`은 기존 SSH의 별도 exec 채널에서 `:`을 최대 3초 기다린다. 일반 명령·출력 역압력이 검사를 막지 않는다.
성공 결과는 현재 연결 세대를 검사한 뒤 actor에서 목록/대상을 확인하고 변경·일시정지된 패널만 캡처한다.
정상 확인은 화면을 보존하며 새 구독과 실제 재접속에는 전체 snapshot을 유지한다. snapshot 조각은 시작 당시 구독에만 속한다.
네트워크 오류는 첫 재시도 즉시, 이후 각각 1초 간격으로 최대 3회 재접속한다.
수동 재시도는 즉시 새 3회 주기를 시작하며 진행 중 접속을 중복 시작하지 않는다.
복귀 시 소진된 네트워크 실패는 새 예산을 시작하고 사용자 정지·종료와 인증/호스트키 오류는 유지한다.

`close()`는 멱등적이며 출력 ack를 기다리지 않는다. 새 요청을 Closed로 거절하고 출력 구독·버퍼를 즉시 반환한 뒤
actor에 정지를 알린다. actor는 진행 중 입출력·타이머와 SSH/control client를 버리고, 미완료 요청을 Cancelled 결과로 남긴 뒤
상태를 Closed로 바꾸고 연결 세대를 올린다. `join()`은 이 종료를 기다린다. tmux 서버·세션·원격 pane 프로세스에는 종료 명령을 보내지 않는다.
완료 후 subscriptions/buffered bytes가 0이며 이후 출력 polling·구독은 Closed다.
일반 SSH shell 종료는 PTY 종료로 별도 구분한다.

Android 프로세스 런타임이 연결 핸들을, Activity의 WebView가 출력 구독을 소유한다.
화면 재생성은 런타임 연결을 유지하되 WebView 출력 구독을 교체하고 새 snapshot을 요청한다.
task 제거는 구독·명시적 코어 종료·외부 객체의 `close()`까지 수행한다.
native await 취소는 호출 종료와 코어 작업 취소를 구분해 정의한다.
[UniFFI async 취소](https://mozilla.github.io/uniffi-rs/latest/futures.html)와
[Kotlin 명시적 객체 해제](https://mozilla.github.io/uniffi-rs/latest/kotlin/lifetimes.html)를
따르며 GC/finalizer만으로 네이티브 자원 회수를 보장하지 않는다.

## 저장·호환·복귀

`CredentialStore`/`FileStore`는 플랫폼 어댑터다. Android Keystore, Windows DPAPI,
macOS Keychain의 저장 실패를 Storage로 전달하며 평문 fallback은 없다. Linux 저장소는 미구현이다.
최초 키 신뢰는 사용자 승인 후 보호 저장이 성공해야 다음 연결에 적용한다. 변경된 키는
인증 전에 차단한다. 비밀번호·RSA·Ed25519·암호화 키 실패를 Network로 분류하지 않는다.

설정은 기존 `LocalState` 필드·enum 이름·기본값·ID·credentialRef·trust를 보존한다.
알 수 없는 설정 필드는 기존처럼 읽기에서 무시하지만 호환되지 않는 타입/잘못된 ID는
실패한다. 설정 파일은 최대 16MiB, 쓰기는 임시 파일 후 원자 교체를 우선한다.
바이트 동일성 대신 모델 의미의 왕복 호환성을 검사한다. 데스크톱 창/레이아웃은 별도
`workspace-layout.json`이며 기존 settings에 혼합하지 않는다.

후보 저장 영역은 기존 앱과 분리하며 사용자 데이터 자동 이전은 구현하지 않는다.
Android ID와 Kotlin 패키지 루트는 `kim.jujin.dglc`이며 초기 앱과 별도로 설치된다.
기존 Keystore 키 별칭·Keychain 서비스·DPAPI entropy는 저장된 자격 증명 호환성을 위해 보존한다.
Kotlin 패키지 변경은 저장 키 교체나 초기 앱 데이터 자동 이전을 수행하지 않는다.

## 구현 매핑

- `dgl_core::connection::Connection`이 actor를 소유한다. `input`/`action`은 request ID,
  `state()`는 sequence 포함 최신 스냅샷을 공유 `Arc`로 반환해 조회가 복제를 만들지 않고, 갱신은 읽는 쪽이
  스냅샷을 보유할 때만 복사한다. `sequence()`는 변경 확인만 한다. `completions()`는 한 번만 소비하는 제한된 결과를 반환한다.
  `close()` 후 `join()`과 완료 결과 조회는 허용되며, 일반 요청은 Closed다.
- `NativeConnection`은 상태/명령/대상을 JSON으로 전달하고 출력은 바이트 record와 `NativeOutputKind` enum으로 전달한다.
  상태 JSON의 `derived`는 코어가 계산한 재시도 판정(`reconnecting`, `retry_available`, 소진, 지역화 문구)이다.
  어댑터는 이를 다시 계산하지 않고 플랫폼 고유의 출력·표시 상태만 더한다. 데스크톱 재시도 패널은 확인 중 숨기는
  `offers_retry()`를 쓰며, 코어는 확인 중에도 재시도 요청을 받아 진행 중인 확인과 합친다.
  원시 tmux/shell 문자열은 UI에 노출하지 않는다. Kotlin은 `stop → jobs join → shutdown → close`로 해제한다.
  콜백 대신 상태·결과·출력을 각각 polling하며 결과는 요청 슬롯과 함께 보존한다. 상태는 `state_sequence()`가
  바뀔 때만 `state_json()`을 받아 파싱하고, 같은 스냅샷의 원격 목록·에이전트 디코드는 반복하지 않는다.
- 연결당 출력 구독 하나로 현재 창의 모든 보이는 pane을 전달한다. 구독 교체는 이전 출력 예산을 반환하고 오래된 구독을 거절한다.
- GPUI는 alacritty 엔진 적용 후, Android는 xterm write callback 후 ack한다. Kotlin은
  적용 직전에 `acceptsOutput(targetJson)`으로 다시 확인하고, 거절한 이벤트도 ack한다. 구독이 없으면 화면 출력 큐를 만들지 않는다.
- 데스크톱 보안 저장은 `desktop/src/storage.rs`가 macOS Keychain(`storage/macos.rs`)·Windows DPAPI(`storage/windows.rs`)를
  선택한다. Android는 `app/src/androidMain`의 `AndroidCredentialStore`(Keystore)를 후보 전용 디렉터리
  (`noBackupFilesDir/rust-candidate-v1`, `androidApp`의 `EngineFactory.kt`)에 사용한다. 플랫폼 자격 증명을 코어 생성 시 전달하고 코어에 OS UI를 넣지 않는다.
- `:mobileUi`는 Rust 연결 어댑터를 소유하며 Compose/xterm 화면·플랫폼 렌더러는 `:app`의
  재사용 컴포넌트를 호출한다. 앱 엔진 선택은 없으며 이전 엔진은 테스트 소스에서만 컴파일한다.
- 키 생성·공개키 추출은 UniFFI `generate_key`/`public_key`를 통해 Rust에서 수행한다.
  Kotlin `KeyTools`는 UI 경계이며 SSHJ/BC 런타임 의존성은 없다.
- 데스크톱 그룹 배치는 `workspace-layout.json`, 기기별 호스트 ID 순서는 `host-order.json`에 저장한다. 공통 설정·동기화 형식과 공개 API는 유지한다. 탭·연결·자격 증명은 배치 파일에 저장하지 않으며 재실행 시 그룹을 빈 상태로 만든다.

공통 fixture·출력 큐/종료·인증·복구 검사와 미검증 항목은
[검증 기록](VALIDATION.md)을 따른다. 자동 검사는 실제 IME/모든 VT 상태의 증거가 아니다.

## 탐색·포커스

- `Action::SelectTarget { session, window?, pane? }` / JSON `selectTarget`는 세션 ID와 선택적인 창·패널 ID를 받는다. 연결된 control client의 목록(없으면 exec 목록 한 번)에서 소속 관계를 검증하고 세션 연결 → 창/패널 선택 → snapshot을 하나의 actor 요청으로 순차 실행한다. 생략된 대상은 해당 세션/창의 활성 패널을 우선한다. 기존 `selectSession`·`selectPane`는 유지한다.
- 연결 전 초기 목록도 세션·창·패널을 모두 제공한다. 빈 세션 목록이면 하위 조회를 생략한다.
- `Operation::FocusPane { target, direction }` / JSON `focusPane`는 `%N`과 `L/R/U/D`만 허용한다. `select-pane -t '%N' -L/-R/-U/-D` 실행 뒤 창의 실제 활성 패널을 다시 조회하고 입력 PaneTarget(포커스 세대)·화면을 함께 갱신한다. 현재 표시 패널과 다른 시작 target은 StaleGeneration으로 거부한다.
- 같은 세션의 외부 창·활성 pane 선택을 반영한다. 다른 세션으로의 이동은 따라가지 않는다. 출력 target은 pane별 렌더링 식별자, 입력 target은 현재 포커스를 사용한다.
- 데스크톱 resize는 터미널별 한 요청만 진행하며 이후 크기는 최신 값으로 병합한다. 요청 완료 후 달라진 최신 크기를 보낸다.

## 창 단위 렌더링

`State.window`와 `render_targets: pane ID → PaneTarget`을 추가했다. 출력 target은 입력용
`Connection.target()`과 독립적이다. 포커스 변경은 렌더러 세대를 무효화하지 않는다.
`RemoteWindow`에 width/height/zoomed/visible_layout, `RemotePane`에 left/top을 추가했다.
`ResizePaneTo`는 검증된 pane에 셀 단위 width/height를 요청한다. 코어 `State::accepts_output`을 UniFFI `accepts_output`으로
노출해 Android가 현재 렌더링 대상을 확인한다. bridge 입력은 원래 pane과 입력 target을 함께 제출한다.
기존 저장 키·입력 ACK·출력 예산·Windows 빌드 진입점은 유지한다.
자세한 계약은 [TMUX_WINDOW_SYNC.md](TMUX_WINDOW_SYNC.md)를 따른다.

Android Kotlin의 `ConnectionStatus.presenting`은 탭 재부착·창 전환의 최신 화면 준비 상태이며 기본값은 false다.
true이면 `acceptsInput`/`acceptsPointer`를 차단한다. `TerminalSurface.presentationPending(active)`는
측정·렌더링을 유지하면서 불투명 로딩 표시를 제어한다. 현재 구독·연결·창 전환 세대의 모든 보이는 pane이
`ResetEnd` 렌더 ACK를 보내고 snapshot 요청과 코어 복원이 끝나면 공개한다. Rust·UniFFI wire 형식은 바꾸지 않는다.
