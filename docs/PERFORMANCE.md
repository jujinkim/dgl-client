# 선택적 성능 측정 계약

2026-10-24, 후보 실행 전 고정. 숫자·표본 수는
[`performance-v1.json`](../fixtures/migration/performance-v1.json), 장비는
[`environments-v1.json`](../fixtures/migration/environments-v1.json)이 기준이다.
기존 수집 범위는 Mac과 API36 AVD에 한정된다. 기본 검증에 계측을 자동 추가하지 않으며 다른 환경 통과를 추정하지 않는다.

| 항목 | 정의·표본 | 판정 |
|---|---|---|
| cold 기동 | release 프로세스 실행 요청→첫 창이 보이고 입력 처리 가능. 재부팅 후 첫 실행 30회, 각 부팅 ID를 원시 증거에 연결 | p95 ≤1,000ms |
| warm 기동 | 같은 OS 세션에서 기존 프로세스 정상 종료 후 재시작 30회. cold와 합산하지 않음 | p95 ≤1,000ms |
| 로컬 입력 | OS 키 이벤트 수신→해당 변화가 표시된 프레임의 presentation timestamp, 200표본. 테스트 경로의 로컬 echo/커서 변화, SSH 왕복 제외 | p95 ≤33ms |
| 프레임 | 60Hz에서 scroll/분할 resize/탭 전환 중 연속 presentation timestamp 차이 3,600프레임. 빠진 프레임을 제외하지 않음 | p95 ≤16.7ms |
| 유휴 | 실제 SSH 연결 1개·터미널 1개, 출력 중지·30초 안정화 후 60초 이상. 1초 간격, 최소 61표본 | 한 코어 기준 평균 CPU ≤1%, 시간 가중 평균 전체 RSS ≤150MiB |
| 크기 | 최종 release 다운로드 패키지의 실제 bytes, 설치 트리 logical bytes. hardlink는 inode당 한 번, symlink 자체 크기만 | 다운로드 ≤50MiB, 설치 ≤150MiB |
| Android | 같은 API36 AVD·WebView·software GPU·빌드 종류·시나리오에서 로컬 입력/프레임 p95 및 앱+WebView 유휴 RSS 비교 | 각각 기존 대비 ≤1.10배 |

p95는 정렬 후 `ceil(0.95*n)-1` 위치다. CPU는 누적 process CPU seconds의 증가량을
wall seconds로 나눈 뒤 100을 곱하며 코어 수로 나누지 않는다. RSS는 표본 사이 사다리꼴
시간 가중 평균이다. Mac RSS와 Android RSS/PSS는 혼합하지 않는다. GPU allocation은
별도 값 또는 수집 불가 이유를 기록한다. Android 앱 프로세스뿐 아니라 연결된 WebView
sandbox 프로세스를 포함한다. 호스트의 에뮬레이터 프로세스 RSS는 Android 앱 RSS가 아니다.

실제 측정의 OS·장비·GPU·RAM·전원·배율·refresh rate·빌드·소스 커밋·binary hash·표본 수와
도구 오버헤드를 기록한다. 현재 Mac의 시스템 버전은 26.6.2이며 macOS 14 증거가 아니다.
기존 앱이 실행 불가이면 비교 불가로 남기고 후보에는 절대 기준을 적용한다.
Android 상대 판정에는 유효한 같은 환경 기준선이 반드시 필요하다.

## 도구와 현재 한계

`scripts/migration_performance.py`는 원시 timestamp 검증/집계, Mac process-tree CPU/RSS
수집, 패키지·설치 크기 수집을 제공한다. 보고서에서 `collected`는 개별 계측 성공일 뿐
전체 인수 통과가 아니다. 누락·NaN·음수·재사용된 표본·역행 시계·프로세스 교체·짧은 구간·
틀린 계약·틀린 Android 기준선은 `incomplete`/비정상 종료로 처리한다.

```sh
python3 scripts/migration_performance.py idle --pid <앱-PID> --output artifacts/migration-r1/mac-idle.json
python3 scripts/migration_performance.py package --download <release-package> --installed <installed-root> --output artifacts/migration-r1/package.json
python3 scripts/migration_performance.py evaluate <presentation-trace.json> --output artifacts/migration-r1/performance.json
python3 scripts/migration_performance.py evaluate <android-candidate.json> --baseline <android-legacy.json> --output artifacts/migration-r1/android-comparison.json
```

`idle`은 30초 안정화 후 최소 60초를 수집하며 앱의 모든 자식 프로세스를 합산한다.
이 도구만으로 SSH/터미널 개수나 무출력을 증명하지 못하므로 앱 시나리오 기록과 결합한다.
명령행·프로세스 환경·터미널 원문은 수집하지 않는다. 수집 중 다른 부하를 실행하지 않는다.

Android는 기존 `ResourceScenario`에 **별도 `migration-idle` 경로**를 추가했다.
`scripts/migration_android_baseline.py`는 자신이 시작한 Dgl_API36/임시 OpenSSH/tmux만
사용한다. 실제 xterm/SSH 연결 후 원격 출력을 중지하고 30초 안정화, 65초 표본 수집,
정상 종료·control client 회수를 검사한다. `/proc/<pid>/stat`의 CPU ticks/RSS pages와
start-time identity를 사용하며 WebView 서비스가 앱에 연결됐는지 확인한다. 매 실행의
ticks/page size/앱/renderer PID를 식별한다. 결과의 `missingMetrics`를 지우지 않는다.
기존 smoke/soak 분기와 판정은 바꾸지 않았고 이 경로는 장시간 인수를 대신하지 않는다.

```sh
JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home \
ANDROID_HOME="$HOME/Library/Android/sdk" python3 scripts/migration_android_baseline.py
```

종료 시 임시 자격 증명·reverse 포트·전용 AVD를 정리하고 기본 instrumentation runner로
APK를 다시 빌드한다. 원시 보고서는 git 제외 `artifacts/migration-r1/` 아래다. 테스트 전용
SSH 키는 fixture 안에만 존재하고 보고서에 포함하지 않는다.

Android는 아래의 **테스트 전용 표시 계측 훅**을 추가했다. Mac 기동/input-ready·표시 훅,
Android의 정확한 첫 입력 표시·전체 시나리오·정식 표본 수는 아직 미완료다.
evaluator의 unit-test 데이터를 실측으로 사용하지 않는다.
`PerformanceTest`의 20개 SSH callback 표본, xterm write ack, `am start TotalTime`,
headless 렌더러 시간은 위 GUI 지표로 변환할 수 없다. 이 미완료 항목 때문에 성능 기준선
전체 통과나 R1 전체 통과를 선언하지 않는다.

## API36 표시 훅 검증

`MigrationPresentationProbe`는 계측 APK에만 포함된다. 실제 앱·SSH·xterm을 열고 원격 출력을
멈춘 뒤 OS 기능 키 F10 8회를 보낸다. 테스트용 xterm 키 처리기가 로컬 문자만 표시하며 SSH로
입력을 보내지 않았는지는 원격 workload의 `inputBytes == 0`으로 확인한다. 물리 IME 검증은 아니다.
이후 3초 스크롤을 수행한다. 별도 자원 sampler는 이 구간에서 중지한다.

`INPUT → RENDER → VISUAL → DRAW` 표식을 Perfetto에 기록한다. DRAW의 Choreographer
surface token과 SurfaceFlinger display token을 연결하며 **SurfaceFlinger display slice의
끝**을 표시 시각으로 사용한다. 앱 slice의 끝은 GPU/post 완료라서 표시 시각이 아니다.
근거: [Perfetto FrameTimeline](https://perfetto.dev/docs/data-sources/frametimeline).

WebView visual-state 콜백은 이후 draw에 내용이 반영됨을 보장하지만 그 내용이 이전 프레임에
이미 표시되지 않았다고 보장하지 않는다. 따라서 현재 입력 값은
`inputUpperBoundP95Ms`이며 계약의 `localInputP95Ms`와 구분한다.
근거: [WebView.postVisualStateCallback](https://developer.android.com/reference/android/webkit/WebView#postVisualStateCallback(long,android.webkit.WebView.VisualStateCallback)).

수집기는 데이터 손실·누락·중복 marker·연결할 수 없는 frame token·입력 프레임 drop을
거부한다. 스크롤 중 dropped surface 수를 남기고 다음 표시까지의 시간 간격을 유지한다.
서로 무관한 token 0을 표시 근거로 사용하지 않는다. `hook_verified`나 `trace_analyzed`는
부분 계측 성공이며 `qualifiesAsPerformanceAcceptance`는 항상 false다.

```sh
env JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home \
  ANDROID_HOME="$HOME/Library/Android/sdk" python3 scripts/migration_android_presentation.py \
  --trace-processor .tools/trace_processor_shell
python3 scripts/migration_android_presentation.py \
  --trace-processor .tools/trace_processor_shell --analyze <presentation.perfetto-trace> --output <analysis.json>
```

별도로 준비한 로컬 Perfetto `trace_processor_shell`을 명시합니다. 바이너리는 저장소/제품에 추가하지 않습니다.
원시 trace, 추출 행, APK·도구·소스 해시, 실패 보고서를
`artifacts/migration-r1/`에 보존한다. AVD·임시 키·reverse 포트 정리와 기본 test runner
복구를 수행하며, 오프라인 재분석은 기존 수집 보고서를 덮어쓰지 않는 새 출력 파일을 사용한다.

## API36 첫 표시 프레임 진단

`--mode upper-bound|first-presentation`은 수집과 `--analyze`에 동일하게 적용한다.
기본값은 기존 `upper-bound`다. `first-presentation`은 정상 F10 입력 8회, 로컬 write를
100ms 지연하는 입력 8회, write 없이 redraw만 요청하는 입력 1회를 실행한다. 입력 직전에
테스트 JS에 동일 ID를 지정하며, xterm의 해당 셀·화면·커서가 실제 변경되고 그 행이 렌더된
경우에만 content/render 표식을 기록한다. 무변화 입력은 200ms 후 셀·커서 불변을 확인한다.
JS 표식·xterm 콜백은 표시 완료가 아니다.

전용 Dgl_API36에만 `/data/local/tmp/webview-command-line`의
`EnablePerfettoSystemTracing`을 적용하고, 기존 파일은 권한을 보존해 백업·복구한다.
기존 시스템 trace에 `track_event`를 더하며 input, latencyInfo, blink, blink.user_timing,
cc, viz, android_webview, benchmark, graphics.pipeline, disabled-by-default-devtools.timeline
범주를 명시한다. 제품 WebView·터미널 자산·설정 형식은 변경하지 않는다.
근거: [Chromium WebView flags](https://github.com/chromium/chromium/blob/133.0.6943.137/android_webview/docs/commandline-flags.md),
[Perfetto Chrome tracing](https://perfetto.dev/docs/getting-started/chrome-tracing).

보고서의 입력별 `status`는 `proven`, `unresolved`, `no-content-change` 계약을 따른다.
현재 Chromium 133 어댑터는 부분 연결만 지원하며 **`proven`을 발급하지 않는다**. 실제 trace의
렌더 표식 ancestor, `main_frame_pipeline.main_frame_id`, 같은 프레임의 draw·submit,
flow 행을 보존한다. 유효한 compositor→Android surface 연결과 앞선 모든 프레임의 내용
증거가 없으면 `firstPresentationNs=null`을 유지한다. 시간상 가까운 프레임을 선택하지 않는다.
기존 visual-state→draw→SurfaceFlinger 연결은 별도 `drawableUpperBound`이며 첫 표시
증거로 승격하지 않는다. 그 종료 시각만 SurfaceFlinger display slice의 끝을 사용한다.

`clock_snapshot`의 MONOTONIC→BOOTTIME 변환과 입력 구간 전후의 native paired clock
표식을 보존·검사한다. JS `performance.now()`를 OS 시계로 직접 환산하지 않는다.
원시 trace 손실·parser 오류·시계 불일치는 `invalid_trace`, 연결 공백은 `unresolved`다.
`complete`는 짧은 진단 수집/분석의 완료 여부이며, `firstPresentationProven=false`와
`qualifiesAsPerformanceAcceptance=false`를 유지한다. 지연 실험은 key→write의 인과 순서만
확인하며 정확한 100ms 표시 차이나 입력 성능 통과를 주장하지 않는다.

```sh
env JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home \
  ANDROID_HOME="$HOME/Library/Android/sdk" python3 scripts/migration_android_presentation.py \
  --mode first-presentation --trace-processor .tools/trace_processor_shell
python3 scripts/migration_android_presentation.py --mode first-presentation \
  --trace-processor .tools/trace_processor_shell --analyze <presentation.perfetto-trace> \
  --output <new-analysis.json>
```

오프라인 결과는 새 JSON과 `.extracted.json` 쌍으로 저장하며 기존 증거를 덮어쓰지 않는다.
trace config·원시 이벤트·frame/flow ID·clock 근거와 소스·APK·ADB·에뮬레이터·Perfetto 해시를
보존한다. 성공·실패·중단 경로 모두 WebView 플래그·앱/임시 설정·reverse 포트·전용 AVD를
정리하고, 격리 SSH/tmux fixture를 회수한 뒤 기본 test runner를 APK manifest로 확인한다.

## Presentation trace 형식 v1

JSON 최상위 필드:

- `schema: 1`, `scope: macos-current-arm64 | android-api36-avd`, `contractSha256`(고정 계약 파일 SHA),
  `sourceCommit`(40자리), `binarySha256`(64자리), `clock: monotonic-nanoseconds`,
  `inputEnd: presented`, `synthetic: false`.
  여기서 synthetic은 만들어 낸 timestamp/성능 수치를 뜻한다. 자동화로 키를 보내고 실제
  OS presentation timestamp를 수집하는 것은 가능하며 물리 IME 인수와 별도다.
- `environment`: device, os, cpu, gpu, ramBytes, displayHz, displayScale, powerMode,
  scenario, buildMode, memoryMetric, webviewVersion(Mac은 null), renderer.
- `startup.cold[]`/`startup.warm[]`: `{id, launchNs, inputReadyNs}`. Mac만 필수.
- `localInput[]`: `{id, inputNs, presentedNs}`.
- `frames[]`: `{id, previousPresentNs, presentedNs}`. 앞 행 end와 다음 행 start가 일치해야 한다.
- `idle`: connections, terminals, outputBytes, stabilizeSeconds, completeProcessInventory,
  gpuMemory(`{bytes}` 또는 `{bytes:null, unavailableReason}`), samples.
- `idle.samples[]`: `{elapsedSeconds, processes:[{pid, identity, role, cpuSeconds, rssBytes}]}`.
  role은 app/child/webview. 같은 프로세스를 여러 역할로 중복 계수하지 않는다.
- `package`: downloadBytes, installedBytes. Mac만 필수.

플랫폼 훅은 입력/표시의 동일 clock domain을 검증해야 한다. trace의 표식만으로
실측을 입증하지 않으며 원시 OS trace·producer 코드·명령·수집 환경을 보고서에 연결한다.
collect/evaluate 명령이 성공해도 실제 IME·TUI·기능/데이터/배포 인수는 별도다.
