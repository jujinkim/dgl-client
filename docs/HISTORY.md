# 과거 결정·작업 요약

현재 동작·검증 절차는 [문서 목록](README.md), 남은 작업은 [TASKS.md](../TASKS.md)를 기준으로 합니다.
현재 검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 검증 범위와 한계는 [VALIDATION.md](VALIDATION.md)를 따릅니다.
이 문서는 과거 작업의 요약입니다. 과거 지시·완료 판정을 현재 규칙으로 적용하지 않습니다.
커밋별 변경·검사 수·전체 로그·폐기된 명령은 Git에서 필요할 때 확인합니다.

## 2026-10-23: JVM MVP

KMP/Compose·SSHJ·JediTerm(데스크톱)·xterm.js(Android)로 첫 구현을 만들었습니다.
PTY 없는 tmux Control Mode, 호스트키 검증, 보호 저장, snapshot/live 순서와 입력 무재전송,
세션·창·pane 관리, 격리 SSH fixture와 공통 회귀 검사를 도입했습니다.

당시 tmux 3.3a·3.7c와 Android API34·35·36 에뮬레이터 검사를 수행했습니다.
[원격 CI 실행](https://github.com/jujinkim/dgl-client/actions/runs/39898373064)은
`899ad82b540a5b284c59630b0713a8950b09de4a`에서 Linux·tmux·Android 6개 job 성공을 기록했습니다.
이 결과는 현재 Rust 앱의 실기 인수를 대신하지 않습니다.
Windows JVM MSI/EXE·Temurin 소스 수집·두 사용자 DPAPI 검증 자동화는 작성했지만 Windows에서 실행하지 않았습니다.

의존성 371개 버전 OSV 0건, 라이선스·원문·Wrapper·비밀 검사 결과는 그 시점의 선택 그래프에 한정됩니다.
이후 코드·의존성의 현재 보안 결과로 간주하지 않습니다. 라이선스 원문과 해시 자료는 `licenses/`에 보존합니다.

## 2026-10-24: Rust 단일 실행 전환

공통 Rust 코어(russh), UniFFI Android 바인딩, GPUI/alacritty_terminal 데스크톱을 구현했습니다.
기존 JVM 앱 실행·설치물과 Android 엔진 선택을 제거했습니다. Kotlin 참조 엔진은 회귀 검사에만 남겼습니다.
공통 계약 fixture 120건으로 시작했고 현재 추가된 fixture는 원본 JSON에서 확인합니다.

성능 인수를 기능 이식의 선행 조건에서 분리했습니다. 실제 Mac GUI 조회 `cgWindowNotFound`,
Keychain 쓰기 `-25308`로 끝난 시도는 미검증으로 기록했습니다. Windows Rust 빌드·ZIP·수동 CI는 유지하며 실기는 사용자 담당입니다.

API36/WebView 133.0.6943.137/software GPU의 첫 표시 프레임 진단은 정상 입력 8·지연 입력 8·
무변화 입력 1, 총 17개였습니다. WebView 수신 프레임 406개의 `surface_frame_trace_id=-1`,
compositor→Android surface 연결 부재, `misplaced_end_event=1` 때문에
**proven 0, unresolved 17, invalid_trace**로 남겼습니다. 콜백·상한 지연을 실제 표시 지연 통과로 쓰지 않습니다.
현재 도구 계약은 [PERFORMANCE.md](PERFORMANCE.md)에 있습니다.

## 2026-10-24–25: 기능 확장·검증 범위 결정

데스크톱 탭 그룹·드래그 분할·배치 저장, tmux 패널 도구, 사용자 단축키,
CLI 실행·관측·알림, Drive 파일 동기화, 수동 재접속/정지, 모바일 텍스트 작성,
개인키 파일 선택과 호스트 오류 안내를 추가했습니다. AGY·Grok Build는 실험 지원입니다.

앱 식별자를 `kim.jujin.dglc`로 통일했습니다(`495c2ea`). 이전 Android 앱의 데이터 자동 이전은 제공하지 않습니다.
`5fa6b99`에서 선택 창의 모든 보이는 pane을 실제 tmux 배치로 표시하도록 바꿨습니다.
외부 창·pane 선택 동기화, 입력/렌더링 세대 분리, pane별 로컬 화면 유지와 창별 에이전트 집계를 적용했습니다.

tmux 3.3a의 공유 fixture에서는 간헐적인 세션 생성 `InvalidArgument`가 있어 각 검사에 새 fixture를 쓴
개별 실행을 최종 근거로 삼았습니다. 초기 실패를 없던 것으로 처리하지 않습니다.
Mac 실제 GUI·Keychain, Windows 네이티브 빌드·DPAPI·GUI, Android 실물과 장시간 네트워크 전환은 미검증입니다.
사용자 지시에 따라 이후 기본 검증은 기능 검사·유닛 테스트이며 전체 API 계측을 자동 재개하지 않습니다.

## 2026-10-25: 문서·스크립트 정리

제품 소개를 SSH 재접속·tmux 재연결에 맞췄습니다.
슬로건은 **Reconnect less. Resume more.**, 랜딩 URL은 [ddal.ggak.life](https://ddal.ggak.life)입니다.
README는 소개·시작 안내, 상세 조작은 USAGE, 빌드·검사는 DEVELOPMENT, 남은 일은 TASKS로 분리했습니다.
`NEXT_WORK.md`, 과거 TASKS/VALIDATION 내용, 아키텍처 전환 계획·후보·기준선·기능 대조표,
초기 JVM ADR은 이 요약으로 합쳤습니다. 유효한 코어·성능 계약은 `docs/`로 옮겼습니다.

폐기된 JVM MSI/EXE 검사, Temurin 소스 수집, 두 사용자 JVM DPAPI harness와 관련 검증 코드를 제거했습니다.
Windows Rust 패키저·단위 검사·빌드 메뉴·수동 CI와 공통 JVM 회귀 검사는 유지했습니다.

## 2026-10-25–27: 데스크톱·공통 기능

- 입력: 하단 고정 입력 도구·스크롤 단축키, 연결별 여러 줄 작성창·초안·IME·대상 세대 검사,
  하단 Ctrl·Shift·Alt 토글과 공통 Rust 키 인코더를 추가했습니다.
  텍스트 작성의 `Enter 함께 보내기`는 텍스트 전송 성공 뒤의 독립 Enter 요청으로 분리했습니다.
- 화면·설정: 시스템·DGL 기본·사용자 HEX 강조 색상, 플랫폼 창 스타일, 설정 상세 화면,
  한국어/영어·기기별 언어 선택, 스낵바 기본 5초와 남은 시간 게이지, 알림 패널과 네이티브 툴팁을 추가했습니다.
- 동기화: v1 데이터·캐시를 v2로 전환하고 데이터셋·이력과 강조 선택의 병합을 유지했습니다.
- 세션·탭: 복원 중 세션 행 위치 유지, 상태줄의 알림·설정 버튼, 호스트 순서·기기별 드래그 정렬,
  호스트/세션 ID별 탭 재사용을 추가했습니다. tmux 세션·창·pane 제목을 기존 메타데이터 구독으로 GUI 탭에 반영하고,
  활성 pane의 OSC 0/2를 감지하면 제목만 즉시 조회합니다(연결당 500ms·동시 1건).
- 크기: resize 최신값 병합·실패 후 재시도, 연결별 폰트 초기화와 데스크톱 Ctrl+휠을 추가했습니다.
- 표시 이름·아이콘: 앱 표시 이름을 DaGaLi로 통일했습니다. 아이콘 원본·프롬프트를 보존하고 Android 적응형·단색,
  macOS ICNS, Windows ICO를 같은 원본에서 재생성합니다. 절차는 [앱 아이콘 안내](../assets/app-icon/README.md)에 있습니다.

이 기간 검증은 Rust·JVM·웹·Python 단위 검사, Android Debug·lint, Mac ad-hoc 패키징, 일부 격리 tmux 검사 중심입니다.
아이콘 작업 당시 전체 Rust 서식 검사는 기존 `password_shell.rs` 차이로 실패했습니다.
Mac computer-use는 앱 식별자 연결 실패와 `cgWindowNotFound`로 창·툴팁을 관찰하지 못했습니다.
실제 macOS GUI·Keychain·Dock/Finder, Windows 네이티브 빌드·GUI(MSVC 포함)는 수행하지 않았습니다.

## 2026-10-26: Android 연결 수명·복귀

- Kotlin/Java 제품·참조·검사 패키지를 `kim.jujin.dglc`로 통일하고 보호 저장 호환 식별자는 보존했습니다.
- 프로세스 공유 엔진과 SSH 유지 서비스(기본 켜짐), 최근 앱/task 제거 시 로컬 정리, 입력 오류 분류·비차단 안내를 추가했습니다.
- 창 크기에 따른 탐색기·세션 탭·분할 작업 공간과 입력/렌더러 수명을 분리했습니다.
- 복귀 시 별도 SSH exec로 3초 응답을 확인하고, 공통 네트워크 재시도는 3회·1초 간격입니다.
  정상 응답 확인은 입력만 잠시 막고, 실제 복구에서만 이전 창 전체와 dim을 유지합니다.
  백그라운드 유지를 끈 연결은 복귀 시 같은 탭·세션으로 한 번 재연결하며 수동 종료·실패·삭제 세션은 보존합니다.
- 숨긴 shell의 가짜 가로 overflow를 제한하고 탭별 폰트 핀치를 추가했습니다.
  터미널 크기 변경은 애니메이션 없이 교체하고 전환 효과는 재접속 복구에만 적용합니다.

API36 선택 계측과 격리 SSH/tmux 3.7c 복귀 검사를 수행했습니다.
실제 손가락 swipe·화면 잠금·물리 기기 절전 정책·네트워크 전환·장시간 계측은 수행하지 않았습니다.

## 2026-10-27–28: Android 화면·입력·IME

- 화면: 840dp 미만 화면은 두 줄 헤더(로고·전환·메뉴·알림 / 현재 호스트·tmux 버튼)를 씁니다.
  xterm 자동 맞춤을 제거해 원격 행·열을 유지하고 행 단위 터치 스크롤을 적용했습니다.
- 하단 단축키: 840dp 미만은 기본 두 줄(70dp), 설정으로 한 줄(36dp)을 고릅니다.
  두 줄 배치는 Compose Staggered Grid이며 넓은 작업 공간은 한 줄을 유지합니다.
- 복귀·복구: 크기 최초 즉시 제출·50ms 병합·실패 후 150ms 재시도와 크기 변경 중 입력을 적용했습니다(`c2d4aca`).
  복귀 시 기존 화면 위에 dim을 즉시 표시하고, 출력 timeout 때 SSH를 유지한 채 구독·스냅샷을 복원합니다.
  탭 재부착·창 전환의 캡처 전달은 최대 16 KiB로 묶고 모든 표시 pane의 최종 렌더까지 로딩·입력 차단을 유지합니다.
- IME: IME를 논리 크기에서 제외하고 layout 단계에서 가시 영역만 조정합니다.
  기기 로컬 `androidKeyboardMode`(자동·이동·리사이즈)를 추가했고, 하단 CLI 메뉴는 같은 Activity의 오버레이로 옮겨
  열기·선택·닫기 동안 WebView 포커스와 IME를 보존합니다.
- 키보드 항상 보이기(`2cd265f`): 기기 로컬 설정(기본 꺼짐)을 켜면 활성 터미널에서 키보드를 자동 표시·복원하고
  리사이즈를 적용합니다. 크기 옵션·닫기 버튼은 비활성화하고 대화상자·작성 창·비활성 터미널에서는 포커스를 가져오지 않습니다.
  Back이 Activity까지 전달되어 종료되던 문제를 고쳤습니다.

헤더·터치 스크롤·복귀 dim·CLI 메뉴·Staggered Grid·탭 복원·키보드 항상 보이기는 API36 에뮬레이터 선택 계측으로 확인했습니다.
IME 논리 크기 제외·두 줄 단축키 도입·`androidKeyboardMode`는 기기가 없거나 실제 키보드 계측을 하지 않아
APK 컴파일·lint까지만 확인했습니다. 초기 실행에서 에뮬레이터 System UI 대화상자·ANR, 화면 밖 항목·목록 반영 대기,
포커스 준비, 실행 래퍼 시간 초과, About/연결 수명 동시 실행, 접근성 선택 상태·해제 후 IME 준비, Back 전환 시 Activity 종료로
실패한 계측과 설정 fixture 예상값 실패는 통과 수에서 제외했습니다.
IME 열기/닫기 진단에서 WebView 크기 변경은 62→0회, 원격 resize는 0회였으나 앱 창 프레임 p95는
167.17→193.72ms로 개선을 확인하지 못했습니다. 탭 248ms·창 675ms는 요청부터 복원 콜백까지이며 첫 표시 프레임 측정이 아닙니다.
API36/Gboard 800×411dp 가로 화면의 하단 도구 가림은 TASKS에 남겼습니다.
물리 Android·삼성 키보드·실제 SSH·전체 API matrix·장시간 검사는 수행하지 않았습니다.

## 2026-10-28: 버전·공통 아이콘

Cargo workspace 버전을 Android 표시·APK·데스크톱·패키징의 단일 기준으로 연결하고 설정 마지막에 About app을 추가했습니다.
공통 SVG와 Compose/Rust 생성물로 제품 UI의 문자·이모지 아이콘을 교체했습니다.
Windows 네이티브와 실기 인수는 수행하지 않았습니다.

## 2026-10-29: CI·의존성 감사와 tmux 결함

기본 CI에 Rust 코어/UniFFI·web 단위 검사를 추가하고, 수동 `verify_ssh`를 JVM 참조 대신 Rust `ssh_integration`으로 바꿨습니다.
첫 실행에서 즉시 재시도 전환(`c2d4aca`) 뒤 갱신되지 않은 통합 검사 단언과, `--agent-home` 없이 조용히 통과하던 에이전트 검사를 고쳤습니다.
의존성 감사에 Cargo.lock(crates.io)을 추가했습니다. tokio는 1.54.1로 올렸고,
gpui 0.2.2·russh·AGP가 버전을 고정한 나머지 권고는 `blockedBy`·완화 조건·90일 만료를 갖춘 예외로 등록했습니다.

이 검사로 세션 없는 control client의 `refresh-client -B` 구독 타이머가 tmux 3.2~3.6b 서버를 SIGSEGV로 종료시키는
결함(3.7에서 수정)을 확인했습니다. 3.7 미만에는 구독 대신 같은 format을 1초마다 조회합니다.
이후 tmux 3.3a·3.7c 통합 검사에서 새 크래시 보고서는 없었습니다.

## 2026-10-29: 코어 일원화·왕복 축소

- 코어 상태를 `Arc` 스냅샷으로 공유하고, Android는 `state_sequence()`가 바뀔 때만 상태 JSON을 받습니다.
- 재시도 판정·문구·소진 여부는 상태 JSON의 `derived`로, 출력 수용은 `State::accepts_output`, 알림 수집·읽음·전달은
  `Inbox::tick`으로 코어가 판정합니다. 데스크톱의 관측 단계도 코어 기준(Ready·Restoring·Selecting)으로 맞췄습니다.
  첫 구현에서 직렬화 키 순서가 바뀌어 Android 입력 허용이 열리지 않던 결함을 API36 계측으로 찾아 고쳤습니다.
- tmux 목록 갱신과 그 앞의 명령을 한 번 왕복으로 보내고, 같은 대상의 대기 입력은 최대 16개씩 묶습니다.
  묶음 중간에 연결 작업이 중단되면 이미 전달된 입력이 취소로 보고되던 문제를 고쳤습니다.
- 에이전트 관측은 2초마다 도우미를 새로 실행하지 않고 연결마다 상주 도우미에 묻습니다.
  도우미가 한 번도 응답하지 못하면(예: Python 없음) 30초 동안 기본 조회로 대신합니다.
- 연결 처리 `connected()`를 `Actor`와 `Remote::{Tmux, Shell}`로 나눴으며 동작은 바꾸지 않았습니다.

상태 스냅샷은 API36 `AndroidOutputRecoveryTest`·`AndroidConnectionLifetimeTest`로, 명령 왕복 축소·도우미 상주·`Actor` 분리는
tmux 3.3a·3.7c 통합 검사로 확인했습니다. 출력 수용·알림 규칙 이동에는 따로 기록한 검사 결과가 없습니다.

## 2026-10-29: 데스크톱 정리

터미널 배경 칠하기를 줄이고 스낵바 게이지를 100ms 단위로만 다시 그립니다.
수동 재접속은 기존 탭의 연결만 교체합니다. 단축키를 표 하나로 정의해
Windows에서 `Ctrl+Shift+F10`–`F12`가 10–12번째 연결로 바뀌던 문제를 고쳤습니다.
Windows 빌드·실행과 실제 화면 확인은 하지 않았습니다.

## 2026-10-29: 이전 엔진 잔재 제거

- control mode 파서·`TmuxWire`·목록 해석·`SshTransport`/`TmuxController`를 제품 공통 코드에서 `app/src/referenceTest`로 옮겼습니다.
  Rust APK 검사는 옮긴 클래스를 거부합니다.
- 쓰이지 않던 JVM Windows 자격 증명 저장소·JNA platform·`WindowsSecurityTest`·검증기 `windows` 모드,
  `App`의 JVM 데스크톱 화면 분기, mobileUi의 Compose 설정, androidApp의 미사용 lifecycle 의존성을 제거했습니다.
  이 정리(`8106e72`)의 lock 재기록으로 내려간 slf4j-api는 `desktopMain`에 2.0.17을 선언해 고정했습니다.
- `verify_resources.py`의 데스크톱 worker·`DesktopResourceProbe`·`:app:prepareResourceProbe`를 제거했습니다.
  자원 부하는 Android API34–36만 수집합니다. 이전 Android 앱 기준을 다시 만들 수 없어
  `migration_android_baseline.py`·`migration-idle`·`evaluate --baseline`을 제거하고 Android trace를 `incomplete`로 남깁니다.
- `terminal.js`의 제스처가 공통 입력 차단 조건을 읽습니다. pane 경계 드래그가 입력 차단·연결 복구 중에도
  `resize-pane`을 보낼 수 있던 문제를 고쳤습니다. 쓰이지 않던 `@xterm/addon-fit`과 그 asset·허용 목록·고지를 제거했습니다.

스크립트 단위 검사·JVM 검사와 Android Debug APK·계측 테스트 컴파일을 확인했고 에뮬레이터 계측은 실행하지 않았습니다.

## 과거 파일 찾기

정리 직전 기준 커밋은 `5fa6b99`입니다.

```sh
git show 5fa6b99:NEXT_WORK.md
git show 5fa6b99:docs/VALIDATION.md
git show 5fa6b99:docs/ARCHITECTURE_MIGRATION.md
git show 5fa6b99:docs/migration/R1_BASELINE.md
git show 5fa6b99:scripts/verify_windows_installers.py
git log --all --oneline -- docs scripts
```

원문 요구사항과 SHA-256은 [KICKOFF_PROMPT.md](../KICKOFF_PROMPT.md)에 그대로 보존합니다.
