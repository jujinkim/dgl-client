# R1 실행 보고서 — 2026-10-24

상태: **진행 중. R1 전체 필수 검증 미완료, R2–R7 미착수.**
기준 커밋 `53f1ed4c689c9e8e209987fb01401ce3bcdff78b`, `main`의 미커밋 작업 트리에서 실행했다.
시작 시 README/TASKS/전환 문서의 기존 변경을 보존했다. 커밋·push·기본 실행 전환·기존 엔진
삭제·제품 데이터 이전은 수행하지 않았다. 변경된 소스/fixture와 보고서 해시는 로컬
`artifacts/migration-r1/manifest.json`에 기록한다.

## 적용 범위

사용자 후속 지시에 따라 **현재 Mac과 Android API36 전용 에뮬레이터만 검증**한다.
현재 시스템은 macOS **26.6.2 (25G83)**, MacBookAir10,1, M1 8 CPU/8 GPU cores, RAM 16GiB,
내장 2560×1600 Retina다. macOS 14 결과가 아니다. Windows/Linux/Android 34·35와 물리
기기/물리 IME 결과를 이번 실행으로 주장하지 않는다.

JDK는 `/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home`의 17.0.20.1,
Gradle 9.7.0이다. 설치된 Xcode는 26.6 (17F113), `DEVELOPER_DIR` 후보 경로는
`/Applications/Xcode.app/Contents/Developer`다. 시스템 `xcode-select`는 변경하지 않았다.
R2 Rust 빌드를 실행하지 않았으므로 Rust/GPUI 빌드 성공을 의미하지 않는다.

API36은 Dgl_API36, Google APIs ARM64, 4 vCPU/2GiB RAM, 1080×2400/420dpi/60Hz,
`-no-snapshot -gpu software`다. 실제 이미지 fingerprint는
`google/sdk_gphone64_arm64/emu64a:16/BE2A.250530.026.F3/13894323:userdebug/dev-keys`,
WebView는 `com.google.android.webview 133.0.6943.137`이다. 이 환경 식별이 바뀌면 Android
상대 성능 비교에 기존 기준선을 재사용하지 않는다.

## 구현한 산출물

- [기능 대조표](FEATURE_MATRIX.md): 기존 기능 20개 영역과 자동 테스트·수동 종료 조건 연결.
- [코어 API 계약](CORE_API.md): 형식화 명령/구조화 오류, 패널별 세대, 입력 대상 고정,
  출력 총 256KiB/전달 16KiB/ack, 입력 요청별 64KiB·32 대기 슬롯, 취소/종료·바인딩 수명,
  보호 저장 실패·설정 호환·원본 보존 계약.
- 언어 중립 fixture **120건**: protocol 19, escapes 7, commands 31, keys 39, input 11,
  settings 8, state 5. `MigrationContractTest` 6개가 같은 JSON을 직접 읽는다. 유효한
  parser 스트림은 모든 byte 경계와 1바이트씩 feed하고 전체 이벤트/finish를 검사한다.
- [성능 계약](PERFORMANCE.md): 장비·예산·표본 수 고정, strict trace evaluator,
  Mac CPU/RSS/패키지 크기 수집, API36 실제 연결 유휴 수집 경로 추가.
- 기존 `ResourceScenario`에 test-only `migration-idle` 분기를 추가했다. production APK와
  기본 smoke/soak 판정 및 기존 앱 API를 전환하지 않았다.
- 기존 기본 CI 작업에서 fixture/Python 검사가 자동으로 실행된다. job/플랫폼 matrix를
  늘리지 않았고 원격 CI 실행도 요청하지 않았다.

## 실행 결과

| 검사 | 결과 | 로컬 증거 |
|---|---|---|
| 변경 전 JVM | 21 통과·9 예상 skip | `artifacts/migration-r1/legacy-jvm-before.json` |
| fixture 추가 후 최종 JVM | 27 통과·9 예상 skip | `artifacts/migration-r1/legacy-jvm-after.json` |
| 실제 SSH + tmux 3.3a | 35 통과·Windows 전용 1 skip | `artifacts/migration-r1/legacy-ssh-3.3a.json` |
| 실제 SSH + tmux 3.7c | 35 통과·Windows 전용 1 skip | `artifacts/migration-r1/legacy-ssh-3.7c.json` |
| API36 기본 계측 | 8 통과·0 skip | `artifacts/android-matrix-36.json`, `artifacts/android-tests-36.json` |
| Python 회귀 | 61 통과 (신규 성능 도구 13개 포함) | `python3 -m unittest discover -s scripts/tests -v` |
| 원문 보존 | SHA 불변 | `python3 scripts/check_prompt.py` |
| 대시보드 | JSON validator 통과·Git ignore 유지 | standalone Node 24 validator |
| Mac GUI 실행 | 실패, `HeadlessException` | `./gradlew :app:run`, 이번 셸의 `GraphicsEnvironment.isHeadless() == true` |
| API36 유휴 수집 | 65표본/63.989초, 정상 종료·control client 회수 | `artifacts/migration-r1/android-idle-20261023T172140Z/summary.json` |
| 기본 test runner 복구 | `androidx.test.runner.AndroidJUnitRunner` 확인 | `apkanalyzer manifest print` |

SSH 검사는 격리된 OpenSSH와 private tmux socket/일회용 키를 사용했다. 비밀번호·키 인증,
신뢰/변경된 호스트키 차단, tmux 탐지, snapshot/live, 반복 복구·동일 PID·입력 미재전송을
포함한다. 기존 사용자 서버/authorized_keys/소켓/자격 증명을 사용하지 않았다.

API36 계측은 실제 WebView·합성 IME·Keystore tamper·SSH 키·동일 PID·회전/복귀/재생성·
볼륨 키 동작을 확인했다. 합성 입력을 물리 IME·실제 Codex/Claude 인수로 대체하지 않는다.

## 성능 기준선의 측정 범위

| 지표 | tmux 3.3a | tmux 3.7c | 의미 |
|---|---:|---:|---|
| SSH 연결→snapshot | 109.805ms | 113.171ms | loopback, 앱 기동 아님 |
| 입력→control callback p95 | 2.091ms | 2.048ms | 각 20표본, 화면 표시 아님 |
| 지속 출력 | 4,158,594 B/s | 4,377,883 B/s | 기존 합성 출력 workload |

출처는 `artifacts/performance-3.3a.json`, `performance-3.7c.json`이다. 이 값들을
새 로컬 표시/프레임 예산의 통과 근거로 사용하지 않는다.

API36 유휴: 실제 연결 1개·xterm 터미널 1개, 출력 중지 후 30초 안정화, 약 64초 수집.
앱+WebView CPU는 한 코어 기준 평균 **0.281%**, RSS 시간 가중 평균은
**563,221,822 bytes (537.1MiB)**다. 이는 기존 debug 앱 + resource instrumentation의
기준선이다. Android에 데스크톱의 150MiB 절대 예산을 적용하지 않는다. 후보 비교는 동일
빌드/장비/시나리오의 지연·메모리 10% 조건으로 수행해야 하며 아직 후보가 없다.

CPU는 Android `CLK_TCK=100`, RSS는 page size 4,096으로 환산했다. 앱과 연결된 WebView
PID/start-time을 별도로 확인했다. GPU allocation은 software AVD에서 앱별 측정 불가로
기록했으며 0으로 가정하지 않았다. 1초 ADB process 조회와 2초 ResourceScenario metrics
조회 비용이 포함되어 있고 따로 차감하지 않았다. 그 비용 자체의 독립 측정은 하지 않았다.

최초 유휴 실행(`android-idle-20261023T171023Z`)은 ADB root 전환 명령이 exit 1로 실패해
표본 0의 incomplete 보고서를 남겼다. 별도 전용 AVD 검사에서 UID 0을 확인한 뒤 재실행이
성공했다. 첫 성공 실행(`android-idle-20261023T171426Z`)은 0.438%/537.7MiB였다.
측정 소스 해시 기록과 실행 중 변경 감지를 보강한 뒤 최종 실행(`android-idle-20261023T172140Z`)을
수집했고 위 표는 그 결과다. 최초 실패와 이전 성공 결과를 삭제하거나 바꾸지 않았다. 각 실행은 전용 AVD·임시
SSH를 정리했고 종료 후 `adb devices`에 실행 중인 에뮬레이터가 없음을 확인했다.

## 남은 R1 필수 작업과 다음 단계 게이트

1. Mac 기동 input-ready·presentation 훅과 Android의 정확한 첫 입력 표시를 연결한다.
   API36의 테스트 전용 표시 훅은 추가했으며 아래 후속 기록을 따른다. 현재 입력 결과는
   drawable-frame 표시까지의 상한이라 정식 입력 지표로 사용하지 않는다.
2. Mac GUI 실행 가능한 세션에서 release 기동/입력/프레임/연결 유휴/설치 크기를 수집한다.
   기존 앱이 계속 실행 불가이면 비교 불가를 유지하고 후보의 절대 기준으로 판정한다.
3. API36 동일 AVD의 로컬 표시/프레임 기준선을 확보한다. 현재 유휴 결과만으로 Android
   전체 상대 성능 판정을 할 수 없다.
4. 원시 trace·도구 오버헤드·빌드 식별을 연결해 R1 판정한다. **그 이후** R2 Cargo workspace,
   Rust/GPUI 실제 SSH 검증 앱, UniFFI/Keystore 수명 검사와 채택 ADR을 진행한다.

R2–R7 구현을 완료한 것으로 보고하지 않는다. 기술 후보를 실행하지 않았으므로 GPUI를
탈락 처리하거나 Iced/Tauri로 변경하지도 않았다. 기본 실행은 기존 KMP/Compose다.

이어갈 구현 순서와 종료 조건은 [NEXT_WORK.md](../../NEXT_WORK.md)에 모았다.

## 표시 훅 후속 실행 — 2026-10-24

R1 중 Android 표시 계측 경로를 구현하고 **짧은 훅 검증**을 수행했다. 제품 UI·터미널
번들·기본 실행 엔진은 변경하지 않았으며, 추가 훅은 `androidTest`와 `resourceProbe`에만 있다.
기존 미커밋 R1 변경을 보존하고 계속 `main`에서 작업했다.

| 검사 | 실제 결과 |
|---|---|
| Android debug·계측 APK 빌드 | 통과, 종료 시 기본 `AndroidJUnitRunner` 복구를 APK manifest로 확인 |
| API36 표시 훅 | `hook_verified`, OS 기능 키 8회·3초 스크롤 표시 간격 80개·dropped surface 0 |
| 종료 | control client 0, 원격 입력 bytes 0, 출력 중지 유지, 최종 `adb devices` 비어 있음 |
| Python 성능·표시 수집기 검사 | 20 통과 (신규 표시 수집기 7개) |
| JVM 언어 중립 fixture 검사 | `MigrationContractTest` 6 통과 |
| 원문·대시보드 | 원문 SHA 불변, JSON validator 통과, metadata Git ignore 유지 |
| Mac GUI 사전 확인 | JDK 17 jshell의 `GraphicsEnvironment.isHeadless()`가 여전히 true |

최종 증거는 `artifacts/migration-r1/android-presentation-20261023T195311Z/`의
`summary.json`, `presentation.perfetto-trace`, `extracted.json`이다. Perfetto v58.2,
API36 이미지·WebView·software renderer는 위 기준선 환경과 같다. APK·계측 APK·수집기·
훅·trace 해시는 보고서에 기록했다. 최종 trace SHA-256은
`a877ddc2fd5bee089f3222cfe7d426bfc5426bfbacdb267a73e14d25c3005895`다.

실측 진단값은 입력→내용이 준비된 draw의 표시 상한 p95 **630.835ms**, 스크롤 표시 간격
p95 **53.978ms**다. 입력 상한은 WebView render/visual callback 왕복 비용을 포함하고,
정확한 첫 표시 시각이나 물리 IME 지연이 아니다. 작은 표본·software AVD·도구 오버헤드
미분리 상태이므로 이 값을 정식 성능 기준선이나 후보 비교에 사용하지 않는다. 보고서의
`qualifiesAsPerformanceAcceptance`는 false다. R1 전체 상태는 계속 진행 중이다.

실패와 수정 이력도 보존했다:

- `android-presentation-20261023T194729Z`: 문자 A 입력 경로에서 timeout. IME 변환을
  거치지 않는 OS 기능 키 F10을 테스트용 로컬 echo에 연결해 입력 경로를 분리했다.
- `android-presentation-20261023T195001Z`: 앱 시나리오·종료는 완료했으나 수집기가 관련 없는
  display token 0을 중복으로 거부했다. 실제 참조하는 양수 token만 사용하도록 수정했다.
  원본 실패 보고서는 유지하고 `analysis-v2.json`에 별도 재분석을 기록했다(8개 입력,
  83개 표시 간격; 입력 상한 107.294ms). 작은 표본의 실행 간 변동도 기준선 미완료 사유다.
- 최종 실행은 수정된 전체 수집 경로로 통과했다. 중단된 P3-03 soak, 다른 API matrix,
  원격 CI, 재부팅 기반 cold 30회 검증은 실행하지 않았다.

이번 후속 파일·보고서의 해시는 `artifacts/migration-r1/presentation-manifest.json`에
별도로 기록해 최초 R1 `manifest.json`의 과거 증거를 덮어쓰지 않는다.

## 첫 표시 프레임 진단 — 2026-10-24 후속

**이번 선행 과제는 연결 불가 사유를 확인하고 종료했다. 첫 표시 증명·R1 성능 인수는 미완료다.**
`main`과 기존 미커밋 변경을 보존했다. 수집기/오프라인 분석의 `--mode first-presentation`,
테스트 전용 F10 ID·실제 셀 변경 검사, Chromium 부분 연결 분석기와 회귀 검사를 추가했다.
제품 공개 API·설정·터미널 번들은 변경하지 않았다.

최종 원시 증거는 `artifacts/migration-r1/android-presentation-20261024T060040Z/`다.
`summary.json`은 수집 당시 분석이며 최신 재분석은 `analysis-final.json`과
`analysis-final.extracted.json`이다. 수집 소스의 원본 복사본도 `sources/`에 보존했다.
최종 trace SHA-256은
`f11df5115044da9a1a726fe2626c5fd35b357fae7eeef754afb13cfc3fe4f9ed`다.

| 확인 항목 | 관찰·판정 |
|---|---|
| 환경 | Dgl_API36, 기존 API36 fingerprint·WebView 133.0.6943.137·software GPU 유지 |
| 입력 | 정상 8회, 100ms 지연 8회, 무변화 redraw 1회; 앱 시나리오 정상 종료 |
| 내용 연결 | 16개 모두 xterm 지정 셀 변경·해당 행 렌더·Chromium main-frame ID 확인 |
| compositor 경계 | 수신 프레임 406개의 `surface_frame_trace_id`가 모두 `-1`; 유효한 프레임 join key 없음 |
| surface/display | visual-state 뒤 draw의 16개 surface/display 연결만 별도 진단 증거로 보존; 첫 표시로 채택하지 않음 |
| clock | MONOTONIC→BOOTTIME snapshot offset −42/−41ns, native 전후 교차 검사 통과; JS wall-clock 환산 없음 |
| trace 품질 | Perfetto `misplaced_end_event=1` (`data_loss`), parser error 0; 손실을 무시하지 않음 |
| 최종 입력 판정 | `proven=0`, `unresolved=17`, 첫 표시 timestamp 전부 null, `status=invalid_trace` |
| 무변화 입력 | JS/native 불변 표식 확인. 손실 때문에 `no-content-change` 확정을 유보 |
| 지연 순서 | JS key→write: 정상 0.082–0.212ms, 지연 101.134–104.885ms. 인과 순서 진단이며 표시 지연 아님 |
| 정리 | WebView 플래그 원본 복구·전용 AVD 종료·control client 0·기본 `AndroidJUnitRunner` manifest 확인 |

예를 들어 입력 1은 native slice `162701` → content slice `163342` → Chromium
main-frame `1771875297891618817`로 이어진다. 독립적인 visual-state 이후 draw는 surface
`10928`/display `10960`에 연결되지만 그 draw가 최초라는 증거가 없다. 두 경로를 시간상
가까운 이유로 결합하지 않았다. 데이터 손실 경고를 없애더라도 유효한 compositor 경계 ID와
이전 프레임의 내용 증거가 빠져 있어 현 trace만으로 `proven` 판정을 할 수 없다.

`qualifiesAsPerformanceAcceptance=false`를 유지한다. 입력 200개·프레임 3,600개,
resize/탭 전환·독립 오버헤드·Mac 측정·R2는 이번에 진행하지 않았다. 현재 Chromium 133
분석기는 `proven`을 발급하지 않는 부분 연결 진단기다. 다음 작업은 필요한 producer 계측의
확보 여부 결정이며, 수집량을 늘리거나 상한을 첫 표시로 재명명하는 작업이 아니다.

이전 실행도 보존했다. `android-presentation-20261024T055331Z`는 넓은 범주로 수행한
탐색 trace이며 초안 보고서 대신 `analysis-v2.json`의 `invalid_trace` 판정을 따른다.
`android-presentation-20261024T055842Z`는 ADB root 재시작 중 연결 종료로 계측 전에 실패했다.
전용 AVD의 재연결 후 UID 0을 검증하도록 보강하고 회귀 검사 2개를 추가했다. 실패 실행에서도
기본 runner를 복구했다. 최종 실행은 범주를 줄였지만 같은 손실 경고와 연결 공백이 남았다.

| 회귀·최종 검증 | 결과 |
|---|---|
| Python 전체 | 82 통과; 성능·표시 관련 34개(이번 추가 14개) 포함 |
| 기존 upper-bound 실제 수집 | `android-presentation-20261024T060258Z`: hook_verified, 입력 8개·표시 간격 101개·drop 0 |
| debug·계측 APK | 빌드 통과; 각 수집 종료 시 기본 runner manifest 검증 |
| API36 기본 계측 | 8 통과·0 skip; `first-presentation-android-matrix.json` |
| JVM | 27 통과·9 예상 skip, MigrationContractTest 6개 포함; `first-presentation-jvm.json` |
| 최종 정리 | adb device/forward 목록 비어 있음, 격리 SSH/tmux/workload 프로세스 0 |
| 원문·대시보드 | SHA 불변, validator 통과, metadata Git ignore 유지; 기존 docs 등록 경로 유지 |

검증 결과와 최종 파일 해시는 `artifacts/migration-r1/first-presentation-manifest.json`에
기록하며 최초 R1 및 이전 presentation manifest를 덮어쓰지 않는다.

## 재현 명령

아래 명령에는 유효한 JDK 17/Android SDK 환경을 적용한다. 실제 실행 로그/개인 키는 공개
문서에 포함하지 않는다. 셸 명령은 프로젝트 지침에 따라 RTK를 통해 실행했다.

```sh
rtk proxy env JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home ANDROID_HOME="$HOME/Library/Android/sdk" ./gradlew :app:desktopTest
rtk proxy python3 scripts/verify_test_results.py --mode linux --output artifacts/migration-r1/legacy-jvm-after.json
rtk proxy python3 scripts/integration.py --tmux .tools/tmux-3.3a/tmux-3.3a/tmux
rtk proxy python3 scripts/verify_test_results.py --mode ssh --output artifacts/migration-r1/legacy-ssh-3.3a.json
rtk proxy python3 scripts/integration.py --tmux .tools/tmux-3.7c/tmux-3.7c/tmux
rtk proxy python3 scripts/verify_test_results.py --mode ssh --output artifacts/migration-r1/legacy-ssh-3.7c.json
rtk proxy python3 scripts/verify_android_matrix.py --apis 36
rtk proxy python3 scripts/migration_android_baseline.py
rtk proxy python3 -m unittest discover -s scripts/tests -v
```
