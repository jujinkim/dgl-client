# 공통 코어 계약 v1 (R1)

2026-10-24. 후보가 보존할 계약이다. 아래에 실제 구현 매핑과 인수 범위를 별도로 기록한다. 기존 Kotlin API를 새 공개 API로 간주하지 않는다.
`fixtures/migration/contract-v1.json`은 기존 호환 계약, 이 문서는 새 동시성·수명 계약이다.

## 모듈과 공개 값

| 모듈 | 소유권 |
|---|---|
| `crates/dgl-core` | 순수 모델/설정/입력, SSH, tmux 파싱·명령·레이아웃, 연결·복구·출력 제어 |
| `crates/dgl-uniffi` | Android 외부 객체·바이트 변환·콜백, 비동기 취소·해제 |
| `desktop` | GPUI 후보, 터미널 엔진/그리기/IME/포커스, OS 보안 저장 |
| `:coreBindings` | 생성 바인딩·네이티브 로더·Keystore/파일 콜백 |
| `:mobileUi` | Compose/xterm.js 프레젠테이션·ViewModel 수명 |
| `:app` | 공통 모델·화면·플랫폼 저장소; 이전 연결 엔진은 referenceTest 전용 |

코어에 Compose/GPUI/JNI 객체, OS 창, UI 표시 문구를 넣지 않는다. 아래 이름은
언어 중립 API 명세다. 실제 후보는 아래 매핑을 사용한다.

- `ConnectionHandle`: 생성 후 재사용하지 않는 opaque ID. `close` 이후 모든 일반 호출은 `Closed`.
- `Generation { connection, pane, size }`: u64 단조 증가 값. wrap은 오류로 종료.
  pane 세대는 패널 ID별로 관리하며 제거 후 같은 ID를 관측해도 이전 세대를 재사용하지 않는다.
- `PaneTarget { connection, paneId, generation }`: 입력을 만들 때 캡처한다.
  송신 시 UI의 현재 포커스를 다시 조회하지 않는다.
- `Command { requestId, operation }`: 세션/창/패널 생성·선택·이름 변경·분할·resize·삭제,
  새로고침, snapshot. raw shell/control 문자열을 일반 UI에 노출하지 않는다.
- `Error { code, operation, retryable, diagnostic }`: Authentication, HostKeyUnknown,
  HostKeyChanged, Network, Protocol, MissingSession, Storage, InvalidArgument,
  QueueFull, NotReady, StaleGeneration, Cancelled, Closed. 진단은 경로 탐지 단계·안전한 종료 코드만
  허용하며 비밀번호/키/원격 출력/사용자 입력을 포함하지 않는다.
- `StateEvent { sequence, connectionGeneration, state }`: 연결 단계, 신뢰 요청,
  서버 권위의 sessions/windows/panes/layout, 요청 완료/취소, 종료 완료.
- `OutputEvent { subscription, sequence, target, kind, bytes }`: Resize, ResetBegin,
  ResetChunk, ResetEnd, Live. byte buffer는 수신측 소유 불변 복사본이며 UTF-8 코드포인트가
  배치/이벤트 경계를 넘을 수 있다. Kotlin에서 임의로 문자열 변환하지 않는다.

## 명령과 입력

연결별 actor 한 개가 명령을 직렬화한다. 입력 요청 하나의 조각들은 해당 요청 내에서
연속 전송하며 snapshot/관리 명령이 중간에 끼지 않는다. tmux 입력은 최대 256바이트씩
hex 인코딩한다. 입력 한 요청은 UTF-8 변환 후 1..65,536바이트, 대기 요청은 32개 이하다.
이는 **요청별 64KiB, 32개 대기 슬롯**이며 기존 구현이 64KiB 총 대기 메모리를 보장했다는
의미가 아니다. 실행 중 요청 1개를 포함한 입력 payload 상한은 33×64KiB다.

`submitInput(target, bytes)`는 Accepted(requestId), InvalidArgument, QueueFull,
StaleGeneration, NotReady, Closed 중 하나를 즉시 반환한다. Accepted는 원격 처리 완료가
아니다. 실행 직전 세대를 다시 검사하고 완료 이벤트를 Sent/Cancelled/StaleGeneration/
TransportLost로 전달한다. 연결 단절 시 전송 성공 범위를 알 수 없는 요청도
TransportLost로 끝내며 **자동 재전송하지 않는다**. 복구 중 입력은 거절한다.
포커스가 바뀌어도 이미 수락한 입력은 원래 pane으로만 가거나 취소된다.

취소는 requestId 기준으로 멱등적이다. 이미 보낸 바이트는 회수하지 않는다. 관리 명령의
불확정 원격 결과도 자동 반복하지 않고 refresh로 확인한다. UI의 삭제 확인 후에만
형식화된 삭제 명령을 제출한다.

## 출력·역압력·스냅샷

연결 전체에서 대기+전달 중인 **모든 pane 출력 payload 합이 262,144바이트 이하**다.
개별 전달은 최대 16,384바이트, 메타데이터 이벤트를 포함해 미확인 이벤트 256개 이하다.
구독별 unbounded channel, snapshot을 전부 별도 버퍼로 누적하는 우회는 금지한다.
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

resize는 해당 패널의 size 세대를 증가시킨다. 외부 split/kill/resize는 서버 레이아웃으로
상태를 갱신하고 영향받는 패널만 복원한다. 구독 해제/패널 삭제/새 세대에서 오래된 출력과
스냅샷 결과를 폐기하고 예산을 반환한다. 이미 UI로 넘어간 옛 버퍼도 세대 확인 후 적용하지
않고 ack/해제한다. 여러 패널 출력은 각각 순서를 보존하고 라운드로빈으로 전달한다.

## 상태, 종료, Android 수명

상태와 출력 구독은 별개다. 상태 스냅샷은 최신 값으로 합칠 수 있지만 신뢰 요청·명령 결과·
종료 이벤트는 잃으면 안 된다. 명령 결과는 제한된 요청 슬롯과 함께 유지하며 슬롯이 찼으면
새 요청을 거절한다. 종료·취소는 데이터 큐와 별도 제어 경로/취소 토큰을 사용한다.

`close()`는 멱등적이며 출력 ack를 기다리지 않는다. 새 요청 거절 → 세대 무효화 → 모든
입출력/타이머/콜백 취소 → SSH/control client 종료 → 작업 join → 구독/버퍼 반환 →
Closed 완료 순서다. tmux 서버·세션·원격 pane 프로세스에는 종료 명령을 보내지 않는다.
완료 후 active tasks/subscriptions/buffered bytes가 0이며 후속 콜백이 없어야 한다.
일반 SSH shell 종료는 PTY 종료로 별도 구분한다.

Android ViewModel은 핸들과 구독을 소유한다. `onCleared`에서 먼저 구독을 취소하고
명시적 코어 종료를 호출한 뒤 생성된 모든 외부 객체의 `close()`를 실행한다. 화면 재생성은
ViewModel 연결을 유지하되 WebView 출력 구독을 교체하고 새 snapshot을 요청한다.
native await 취소는 호출 종료와 코어 작업 취소를 구분해 정의한다.
[UniFFI async 취소](https://mozilla.github.io/uniffi-rs/latest/futures.html)와
[Kotlin 명시적 객체 해제](https://mozilla.github.io/uniffi-rs/latest/kotlin/lifetimes.html)를
따르며 GC/finalizer만으로 네이티브 자원 회수를 보장하지 않는다.

## 저장·호환·복귀

`CredentialStore`/`FileStore`는 플랫폼 어댑터다. Android Keystore, Windows DPAPI,
macOS Keychain, Linux Secret Service의 실패를 Storage로 전달하며 평문 fallback은 없다.
최초 키 신뢰는 사용자 승인 후 보호 저장이 성공해야 다음 연결에 적용한다. 변경된 키는
인증 전에 차단한다. 비밀번호·RSA·Ed25519·암호화 키 실패를 Network로 분류하지 않는다.

설정은 기존 `LocalState` 필드·enum 이름·기본값·ID·credentialRef·trust를 보존한다.
알 수 없는 설정 필드는 기존처럼 읽기에서 무시하지만 호환되지 않는 타입/잘못된 ID는
실패한다. 설정 파일은 최대 16MiB, 쓰기는 임시 파일 후 원자 교체를 우선한다.
바이트 동일성 대신 모델 의미의 왕복 호환성을 검사한다. 데스크톱 창/레이아웃은 별도
`desktop-layout-v1.json`이며 기존 settings에 혼합하지 않는다.

후보는 별도 테스트 경로만 사용한다. R6 전환은 기존 영역을 복사하고 검증한 뒤 새 영역을
선택한다. 원본 `.sealed`/Keystore 키를 추출하거나 평문 임시 파일로 내보내지 않는다.
Android ID `kim.jujin.rovessh`, Keystore alias
`kim.jujin.rovessh.credentials.v1`을 유지한다. 복귀는 전환 전 복사본 기준이며 전환 이후
변경분은 자동 역병합하지 않고 별도 보존한다. 실패 시 원본은 바뀌지 않아야 한다.

## 2026-10-24 후보 구현 매핑

- `dgl_core::connection::Connection`이 actor를 소유한다. `input`/`action`은 request ID,
  `state()`는 sequence 포함 최신 스냅샷, `completions()`는 한 번만 소비하는 제한된 결과를 반환한다.
  `close()` 후 `join()`과 완료 결과 조회는 허용되며, 일반 요청은 Closed다.
- `NativeConnection`은 상태/명령/대상을 JSON으로 전달하고 출력은 바이트 record로 전달한다.
  원시 tmux/shell 문자열은 UI에 노출하지 않는다. Kotlin은 `stop → jobs join → shutdown → close`로 해제한다.
  콜백 대신 상태·결과·출력을 각각 polling하며 결과는 요청 슬롯과 함께 보존한다.
- 이번 제품 범위는 선택 pane 한 개와 출력 구독 한 개다. 구독 교체는 이전 출력 예산을 반환하고
  오래된 구독을 거절한다. 다중 pane 동시 표시/라운드로빈은 이번 API 구현에 추가하지 않는다.
- GPUI는 alacritty 엔진 적용 후, Android는 xterm write callback 후 ack한다. Kotlin은
  target/generation을 적용 직전에 다시 비교한다. 구독이 없으면 화면 출력 큐를 만들지 않는다.
- macOS Keychain 어댑터는 `desktop/src/storage.rs`; Android는 기존 Keystore 구현을
  후보 전용 디렉터리에 사용한다. 플랫폼 자격 증명을 코어 생성 시 전달하고 코어에 OS UI를 넣지 않는다.
- `:mobileUi`는 Rust 연결 어댑터를 소유하며 Compose/xterm 화면·플랫폼 렌더러는 `:app`의
  재사용 컴포넌트를 호출한다. 앱 엔진 선택은 없으며 이전 엔진은 테스트 소스에서만 컴파일한다.
- 키 생성·공개키 추출은 UniFFI `generate_key`/`public_key`를 통해 Rust에서 수행한다.
  Kotlin `KeyTools`는 UI 경계이며 SSHJ/BC 런타임 의존성은 없다.
- 새 화면 상태는 메모리에만 둔다. 기존 설정에 레이아웃 필드를 추가하지 않았으며
  `desktop-layout-v1.json`은 향후 창 상태를 저장할 때 사용할 이름이다.

120 fixture·출력 큐/종료·인증·복구 검증 결과와 아직 인수하지 않은 항목은
[후보 기록](CANDIDATE.md)을 따른다. 자동 검사는 실제 IME/모든 VT 상태의 증거가 아니다.
