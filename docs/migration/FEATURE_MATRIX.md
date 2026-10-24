# 기능·회귀 인수 대조표 (R1)

2026-10-24. 이번 실행 대상은 **현재 macOS 26.6.2 ARM64와 Dgl_API36**이다.
사용자가 범위를 축소했다. macOS 14/Windows/Linux/Android 34·35는 이번 인수 대상에서
제외하며 지원 완료를 주장하지 않는다. 제품의 원래 대상/Android 네 ABI 요구는 보존한다.

`J:`는 `app/src/desktopTest/.../`, `C:`는 `app/src/commonTest/.../core/`,
`A:`는 `androidApp/src/androidTest/.../` 아래 테스트다. 구현됨·자동 통과·수동 통과는 다르다.
실행 증거는 [R1 보고서](R1_BASELINE.md), 원래 결과는 [VALIDATION](../VALIDATION.md).

| ID | 보존할 동작 | 기존 자동 증거/fixture | 새 구현 검증 및 수동 종료 조건 | 단계 |
|---|---|---|---|---|
| F01 | 호스트 생성/편집/삭제·별칭·즐겨찾기·최근 접속·독립 연결 | `LocalRepository`, `J:PreferencesTest`, settings fixture | 두 호스트 동시 연결, 한 연결 닫아도 다른 연결 유지, 삭제 확인 취소 시 불변 | R3/4/5 |
| F02 | 설정 기본값·매크로·ID·credentialRef·trust | `J:MigrationContractTest.settingsFixtures`, `C:TerminalInputTest` | 구형→신형→구형 왕복, 원본/암호문 보존, 실패 복귀 | R3/6 |
| F03 | 비밀번호·RSA·Ed25519·암호화 키·생성/공개키 복사 | `J:AuthenticationTest`, `J:IntegrationTest.hostTrustMismatchAndKeyAuthentication`, `A:AndroidSshTest` | 각 인증 성공/실패 구분, 실제 키 가져오기 UI, 안전한 오류 | R2/3 |
| F04 | 최초 신뢰·변경 차단·신뢰 삭제·저장 실패 | 위 인증 검사, `J:WindowsSecurityTest`, `A:AndroidPlatformTest.keystoreRoundTripTamperingAndBackupExclusion` | Keychain/Keystore 거부 시 저장·연결 중단, 변경 키 인증 전 차단 | R2/3/5 |
| F05 | tmux 탐지 경로·실패 상세·일반 SSH fallback | `J:TmuxDiscoveryTest` 전부, `IntegrationTest.tmuxDiscoveryUsesLoginPathForCreateListAttachAndReconnect` | 경로 탐지 실패/지원하지 않는 버전 표시, shell 전환 후 입력·종료 | R3 |
| F06 | byte framing·UTF-8 분할·octal·위조 알림·상한 | `C:ProtocolTest`, protocol/escapes/state fixture | 같은 fixture를 Rust에서 사용, malformed/EOF 실패 폐쇄 | R1/3 |
| F07 | 형식화 명령·두 단계 인용·ID 검증 | commands fixture, `IntegrationTest.plainSshShellAndHostileNamesAndErrorRecovery` | tmux 3.3a/3.7c에서 hostile name/trailing semicolon가 데이터로만 처리 | R1/3 |
| F08 | 세션/창/pane 조회·생성·이름변경·선택·분할·resize·삭제 | `IntegrationTest.actualSshControlInputExternalChangesAndProcessPreservation`, `A:AndroidAppFlowTest` | 명령 팔레트/키보드/터치로 같은 작업, 삭제 취소, 외부 변경 반영 | R3/4/5 |
| F09 | snapshot/live 순서·크기 세대·ANSI/Unicode/alternate screen | `J:RendererTest`, `IntegrationTest.simultaneousClientsSnapshotDuringOutputAndSessionDeletion` | 연속 번호 출력과 반복 snapshot, 중복/누락 0, 오래된 resize 폐기 | R2/3 |
| F10 | 단일 선택 pane 표시와 포커스 고정 입력 | 기존 단일 pane 테스트만 있음 | 서버 레이아웃 일치, A 입력 직후 B 포커스해도 B에 전달 0, 외부 kill/resize | R3/4 |
| F11 | 출력 256KiB·16KiB 묶음·ack·느린 소비자 | `ResourceScenario`, desktop pipe/Android ack 검증 | 모든 pane 합산 예산, 가득 찬 상태에서 cancel/close 완료, 작업/버퍼 0 | R2/3/5 |
| F12 | 입력 64KiB·32 대기 슬롯·개행·키/매크로 | keys/input fixture, `IntegrationTest.macrosUseSelectedPaneAndPlainShellOnce` | 명시적 거절/취소/오래된 세대 결과, 빈 입력/한도 초과, 재전송 0 | R1/3/5 |
| F13 | 반복 단절·복구·동일 PID·삭제 세션 대체 금지 | `J:RecoveryTest`, `ResourceScenario`, `A:AndroidSshTest` | 실제 단절·재연결, 정상 종료 후 원격 PID 유지·control client 0 | R3 |
| F14 | 테마·폰트·단축키 바·볼륨 키 | `C:TerminalInputTest`, `J:PreferencesTest`, `A:AndroidVolumeKeyTest` 4개 | press/repeat/release·설정 취소·dialog/선택·복구 중 입력 차단 | R4/5 |
| F15 | WebView 재생성·회전·복귀·수명 | `A:AndroidAppFlowTest`, `AndroidVolumeKeyTest.visibleShellDialogsSelectionRecoveryAndRecreation`, resource probe | API36 ViewModel 유지, 재구독/해제, 네이티브 핸들/버퍼 회수 | R2/5 |
| F16 | 선택·복사·붙여넣기·마우스 보고·한/일 IME | `A:AndroidPlatformTest.bundledRendererImeBridgeResizeAndNavigationIsolation`는 합성 IME | 실제 조합/삭제/후보창 위치/중복 없는 paste, 포커스/배율/움직임 감소 | R2/4/5 |
| F17 | 실제 Codex/Claude TUI | 합성 ANSI/Unicode 테스트만 있음 | 실제 로그인/권한/alternate screen/출력/실행 중 재접속, 원격 PID 유지 | R4/5 |
| F18 | 기동·로컬 표시·프레임·유휴·크기 | `J:PerformanceTest`는 SSH callback만 측정 | 고정 성능 계약의 원시 표본·정의·빌드 식별, 상대/절대 기준 판정 | R1/2/4/5 |
| F19 | 장시간 부하·단절·재생성 후 회수 | `scripts/verify_resources.py`와 `ResourceScenario` | 기존 soak 시간/입력/FD/메모리 기준 유지, smoke로 대신하지 않음 | R3/4/5 |
| F20 | 패키징·설치·업데이트·제거·고지·해시 | `collect_artifacts.py`, APK/Windows 검증 도구 | 이번에는 Mac/APK, 네 ABI 포함, 실패 원본 보존, 사설 서명/공개 게시 제외 | R6 |

## 단계 판정 원칙

R1의 기준선은 기존 구현의 결함이나 미측정 항목도 그대로 기록한다. 기존 앱을 실행할 수
없는 환경은 비교 불가이며 후보에는 절대 기준을 적용한다. Android 상대 기준에는 **같은
API36 AVD·렌더러·시나리오·빌드 종류**의 유효 기준선이 필요하다. 합성 IME/에뮬레이터 결과는
물리 입력 결과가 아니다. 자동 검증을 통과해도 남은 필수 항목을 임의 통과 처리하지 않는다.

R2는 Rust/GPUI 실제 SSH→신뢰→tmux 선택→터미널 입출력→종료/재접속과 Android binding
수명·Keystore를 검사한다. GPUI 실패가 실증되면 Iced, Tauri+xterm.js 순으로 같은 조건에서
비교한다. 미실행은 툴킷 탈락이 아니다. R2–R5 기능 구현은 진행하며 GUI·성능 미검증을 구분한다. Android는 Rust 단일 엔진이며, Mac은 GPUI 앱만 실행 대상으로 제공한다.

## 후보 구현 상태 (이번 완료 기준)

이 표의 상태는 기존 JVM 자동 증거와 구분한다. `대기`는 통과가 아니다.

| 범위 | 필수 여부 | Rust/Mac/Android 구현 | 새 자동 검증 | 화면 검증 |
|---|---|---|---|---|
| F01–F02 호스트·설정·독립 연결 | 필수 | 양 UI·호환 저장 구현 | 120 fixture/설정 roundtrip·원본 보존 통과 | 전체 CRUD·복수 연결 인수 대기 |
| F03–F04 인증·키·신뢰·보호 저장 | 필수 | russh·Keychain·Keystore 연결 | RSA/Ed25519/암호화 키/비밀번호·변경 키 차단·Keystore 통과 | Android 지문 확인 통과; Mac Keychain -25308·GUI 대기 |
| F05 일반 SSH·진단 | 필수 | PATH/login/common 탐지·PTY·상세 화면 | 비밀번호 shell fallback·로그인 PATH 통과 | 양 UI 상세 화면 대기 |
| F06–F07 파서·명령 인용 | 필수 | Rust 이식 | 120 fixture·모든 byte split·hostile name 통과 | 화면 검사 비대상 |
| F08 tmux 관리·외부 변경 | 필수 | 세션/창/패널 조작·삭제 확인 구현 | 두 tmux 버전; Android split/창 생성/선택 통과 | 모든 rename/resize/delete·취소 인수 대기 |
| F09–F12 출력·입력·상한·취소 | 필수 | 세대/순서·ack·큐·완료 결과 구현 | bounded queue/close·구독 교체·오래된 입력·snapshot/live 통과 | Mac 실제 터미널 및 긴 연속 출력 인수 대기 |
| F13 복구·PID·세션 소멸 | 필수 | 자동 복구·직접 세션 선택 | 두 버전 실제 TCP 단절/자동 복구·동일 PID·자동 대체 금지 통과 | 복구 화면 인수 대기 |
| F14–F15 테마·키·Android 수명 | 필수 | GPUI 설정·Android 바/매크로/볼륨·ViewModel | API36 키보드/매크로/회전/복귀·WebView 교체 통과 | Android 밝음/어두움·큰 글자 확인; Mac 대기 |
| F16 선택·clipboard·IME | 필수 | alacritty/GPUI 입력·기존 xterm 경로 | Android 합성 IME 통과 | 물리 한글/일본어·Mac 실제 입력 대기 |
| 기본 디자인 | 필수 | Mac 계층/탭·Android 선택기/하단 시트/편집 화면 | 긴 이름·글자 1.3배·키보드 위 저장/취소 확인 | Android 기본 화면 확인; 빈/오류/복구·Mac 대기 |
| 후보 패키징 | 필수 | Mac GPUI·Android Rust 단일 엔진·격리 저장 | 네 ABI·앱 ID·서명·이전 엔진 DEX 제외 검사 | Mac 실행 화면 인수 대기 |
| F17 실제 Codex/Claude | 후속 | 기존 상태 보존 | 미실행 | 미실행 |
| F18 정식 성능·최적화 | 후속 | 측정 도구 보존 | R1 결과 보존 | unresolved 보존 |
| F19 장시간 soak·전체 플랫폼 matrix·추가 원격 CI | 후속 | 기존 상태 보존 | 재개 안 함 | 미실행 |
| 기본 실행 교체·이전 엔진 제외 | 사용자 후속 지시 | Android Rust 기본 실행·JVM 앱 제거·referenceTest 격리 | 기본 Debug/Release 빌드와 APK 검사 | 기존 화면 인수 상태 유지 |
| 사용자 데이터 이전 | 후속 | 미착수 | 미실행 | 미실행 |

세부 구현·실행 증거: [후보 기록](CANDIDATE.md).
