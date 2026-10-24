# 실행 검증과 인수 상태

> 2026-10-24 현재 실행 대상은 Rust 기반 Android와 Mac GPUI다. 이전 JVM 앱의 실행·Windows
> 설치물 경로는 제거했으며, 아래 결과는 당시 검증 기록으로 보존한다. 최신 명령·산출물은
> [후보 기록](migration/CANDIDATE.md)을 따른다. 이전 엔진 코드는 테스트 APK/JVM 검사에만 사용한다.


## 아키텍처 전환 R1 (2026-10-24)

이번 실행 대상은 사용자 후속 지시에 따라 **현재 macOS 26.6.2 ARM64와 Android API36**이다.
macOS 14/Windows/Linux/Android 34·35를 이번에 검증했다고 표시하지 않는다.
공통 fixture 120건을 읽는 JVM 테스트 6개를 추가했다. 변경 전 JVM은 21 통과·9 예상 skip,
fixture 추가 후 실제 OpenSSH + tmux 3.3a/3.7c는 각각 35 통과·Windows 전용 1 skip,
API36 전용 AVD는 8개 계측 테스트 모두 통과했다. 기존 기본 CI 작업 안에서 fixture와
Python 판정 도구 테스트를 실행하며 새 자동 job을 추가하지 않았다.

Mac `:app:run`은 이번 셸에서도 `HeadlessException`으로 종료됐다. SSH callback 지연을
GUI 표시 지연으로 대체하지 않는다. 기동·로컬 presentation·프레임 계측이 남아 R1 전체는
진행 중이며 R2 이후를 통과 처리하지 않았다. 명령·측정·미완료 항목은
[R1 기준선 보고서](migration/R1_BASELINE.md)에 기록한다. 이하 기존 검증 이력은 보존한다.

검증일: 2026-10-23. 개발 호스트: macOS ARM64, JDK 17. 초기 검증은 Gradle 8.13, P4-03 이관은 Gradle 9.7.0이다. 테스트 자격 증명은 실행 중 생성하여 종료 시 폐기했다. 실제 사용자 서버·tmux 소켓·authorized_keys를 사용하지 않았다.

## P4-03 이관 검증

Kotlin/Compose compiler 2.4.21, AGP 9.3.3, Gradle 9.7.0, Build Tools 36.0.0.
앱 진입점·Manifest·계측 테스트는 `androidApp`, 플랫폼 구현·terminal assets는
`app` Android KMP library에 위치한다. 앱 ID·SDK·저장 형식은 유지했다.

- JVM 격리 OpenSSH + tmux 3.3a/3.7c: 각 19 통과, Windows DPAPI 1 skip.
- Android debug·unsigned release·instrumentation APK 및 lint: 빌드 통과. Lint 오류 0·경고 11을 정확한 진단과 기한으로 검토했다.
- 선택 의존성 371개 버전 OSV: runtime/build/test 모두 0, 취약점 예외 없음.
- 보안 스크립트 회귀 테스트 12개, runtime 라이선스 182개 버전 및 Skia/JNA 네이티브 고지 검사 통과.
- JediTerm 3.59 source JAR 두 개로 core/UI 교체용 JAR 재빌드 통과.
- 원문 SHA·Wrapper SHA·Gitleaks 3범위·npm integrity/bundle/audit 통과.
- 최종 Android API 34/35/36: 각각 4개 instrumentation 통과. `artifacts/android-matrix.json`의 complete=true와 각 AVD 결과를 확인했다.
- debug·unsigned release APK의 앱 ID/SDK/Manifest·10개 backup 제외·terminal 8개·전체 LICENSE/NOTICE 일치, 민감 파일 이름 부재 확인. Release의 최적화된 resource 경로는 Manifest resource ID로 추적했다. Debug v2 서명 검증 통과.
- 데스크톱 고지/소스/JVM legal 477개 파일과 해시를 확인하고 `desktop-third-party.zip`으로 수집했다. Windows 실제 설치물은 미생성이다.

전용 AVD에서 `swiftshader_indirect` 사용 시 System UI ANR 창이 앱을 가려
툴바 접근성 검사가 실패했다. 실패 화면 한 장으로 확인하고 현재 지원되는
`-gpu software`로 변경한 뒤 최종 API 34/35/36의 각 4개 테스트가 통과했다. API 35에서는 초기 GUI pane split timeout도 한 번 발생했으며 개별 재실행과 최종 전체 재실행에서 통과했다. 이 관측을 실기 안정성 입증으로 일반화하지 않는다. 실패를 숨기거나
앱 검증 시간을 늘려 통과시키지 않았다. 테스트 도구는 자체 실행한 AVD 프로세스만
종료하고 임시 SSH 키·포트·키보드 설정을 정리한다.

## 초기 구현 검증 기록 (P4-03 이전)

| 대상 | 수행한 검증 | 결과 |
|---|---|---|
| JVM + OpenSSH + tmux 3.3a | 프로토콜 9, 렌더러 3, 인증 1, 실제 SSH 통합 4, 반복 복구 1, 성능 1 | 19 통과, Windows 전용 DPAPI 1 skip |
| JVM + OpenSSH + tmux 3.7c | 동일 테스트, 별도 임시 SSH·tmux 환경 | 19 통과, Windows 전용 DPAPI 1 skip |
| Android API 34 | Keystore tamper·백업 제외, 실제 WebView, 합성 한국어·일본어 조합, SSH 키 인증·동일 PID 재접속, 전체 앱 키보드·회전·복귀 | 4 통과 |
| Android API 35 | 동일 Android 테스트 | 4 통과 |
| Android API 36 | 동일 Android 테스트 | 4 통과 |
| Android 빌드 | debug APK, instrumentation APK, lintDebug | 빌드 통과, lint 오류 0·경고 14. debug APK v2 서명 검증 |
| 보안·무결성 | 원문 SHA-256, Wrapper SHA-256, Gradle dependency verification, npm integrity/audit, Gitleaks | 별도 보안 보고와 최종 검사 기록 참조 |
| Windows | MSI/EXE 작업, Windows 전용 DPAPI 테스트, SHA 고정 CI 정의 | 실행 환경 없음. **빌드·설치·실기 통과 아님** |
| GitHub CI | Linux 검증, tmux 두 버전, Windows 패키징 및 Android API 34/35/36 계측 workflow 작성 | 미실행. GitHub CLI 인증 없음 |

macOS 데스크톱 GUI launch도 시도했으나 현재 실행 셸에 GUI 세션이 없어 `HeadlessException`으로 중단됐다. JVM 컴파일·headless 렌더러 테스트만 통과했다. macOS app-image 패키징은 Compose의 Homebrew JDK vendor 검사에서 중단됐으며 검사를 우회하지 않았다. Windows CI는 Temurin JDK를 사용하도록 정의했지만 실제 실행 결과는 없다.

Lint 경고에는 의존성 새 버전 안내, BC 라이브러리의 TrustAll X509 helper, 리소스 폴더 버전 한정 안내가 포함된다. 앱의 SSH 호스트키 검증을 비활성화한 것은 아니며 외부 WebView 네트워크도 차단되어 있다. 경고를 숨기거나 보안 검사 완료로 바꾸지 않았다.

Android 테스트는 ARM64 Google APIs 에뮬레이터에서 실행했다. Gboard를 터미널 탭으로 열고 단축키 바가 IME 바로 위에 있는지 화면 좌표로 검사했다. 조합 이벤트 테스트는 합성 DOM 이벤트이며 실제 한국어/일본어 키보드 입력 통과를 뜻하지 않는다. 회전·앱 복귀 후 같은 pane PID를 확인했다.

전체 앱 테스트는 접근성 노드를 통해 실제 GUI의 pane 분할·선택, 이름 입력을 통한 창 생성, 글자 크기 변경 후 WebView 교체·화면 복원도 검사한다. 이름 입력창이 열리기 전 terminal textarea를 잘못 선택하지 않도록 dialog 등장까지 기다린다. 키보드 resize와 관리 명령이 겹치는 회귀는 JVM 실제 SSH 테스트에도 추가했다.

## 핵심 회귀 검사

- SSHJ 비밀번호 인증과 잘못된 비밀번호, 생성 Ed25519 키, 파일 RSA/Ed25519 및 암호화 키, 잘못된 키 암호를 검사했다. 최초 호스트키와 변경된 지문은 인증 전에 차단한다.
- 파서는 모든 바이트 경계에서 분할 입력, UTF-8 분할, 응답 본문에 위조 알림, octal escape, 과대 메시지와 잘린 응답을 검사한다. capture-pane의 literal backslash와 live output escape가 다른 점도 검사한다.
- 실제 tmux에 이름·경로처럼 보이는 공격 문자열과 trailing semicolon을 전달해 추가 명령이 실행되지 않고 데이터로 처리되는지 확인했다. 실패한 snapshot 이후 응답 경계가 정상인지 확인했다.
- 연속 번호 출력을 진행하면서 10회 snapshot을 삽입해 복원 화면 이후 live tail에 중복·누락이 없는지 검사했다. 다중 클라이언트, 외부 split/rename, 세션 삭제와 일반 SSH PTY도 검사했다.
- 3회 강제 연결 단절 후 같은 PID, 복구 중 입력 폐기, 삭제된 세션 자동 대체 금지를 검사했다. 복구 후 tmux control client가 하나, disconnect 후 0개로 회수되는지 확인했다. JVM thread/FD/장기 최대 메모리의 누수 검증은 별도이다.
- JediTerm headless 테스트에서 한글·일본어, cursor/color, alternate screen, 전체 폭 행, resize, 15,000줄 출력과 5,000줄 scrollback 상한을 확인했다. Windows 글리프·IME·시각적 TUI 충실도를 대신하지 않는다.
- 전체 Android 화면 검사 중 발견한 viewport 높이 붕괴, 복원 상태에 따른 resize 피드백, 폰트 변경 시 오래된 native view 재사용을 수정했다.

## 재현 명령

JDK 17과 Android SDK 환경 변수를 구성한 뒤 저장소 루트에서 실행한다.

```sh
python3 scripts/check_prompt.py
python3 scripts/verify_wrapper.py
./gradlew :app:desktopTest :androidApp:assembleDebug :androidApp:assembleRelease :androidApp:assembleDebugAndroidTest :androidApp:lintDebug :app:prepareThirdPartyNotices writeDependencyInventory
python3 scripts/build_test_tmux.py 3.3a
python3 scripts/build_test_tmux.py 3.7c
python3 scripts/integration.py --tmux .tools/tmux-3.3a/tmux-3.3a/tmux
python3 scripts/integration.py --tmux .tools/tmux-3.7c/tmux-3.7c/tmux
python3 scripts/scan_secrets.py
python3 -m unittest discover -s scripts/tests -v
python3 scripts/audit_dependencies.py
python3 scripts/verify_licenses.py
python3 scripts/verify_lint.py
python3 scripts/collect_artifacts.py
```

Android에는 API 34/35/36의 Google APIs ARM64 시스템 이미지와 `Dgl_API34`, `Dgl_API35`, `Dgl_API36` AVD를 사용했다. 다른 호스트에서는 호스트 CPU에 맞는 이미지를 선택한다. 각 AVD를 한 번에 하나씩 부팅하고 다음을 실행한다.

```sh
"$ANDROID_HOME/emulator/emulator" -avd Dgl_API34 -no-window -no-audio -no-boot-anim -no-snapshot -gpu software
# 다른 셸에서 부팅 완료(getprop sys.boot_completed = 1) 확인 후:
ANDROID_SERIAL=emulator-5554 python3 scripts/integration.py --command python3 scripts/android_integration.py
```

AVD를 35, 36으로 바꾸어 반복한다. 실제 adb serial을 사용한다. 스크립트는 임시 키를 adb stdin으로 앱 cache에 전송하고 종료 후 삭제하며, reverse 포트와 키보드 설정을 복구한다. 일반 `:androidApp:connectedDebugAndroidTest`에는 SSH fixture가 없으므로 SSH/UI 통합 두 테스트는 skip한다.

## 성능의 측정 범위

`artifacts/performance-3.3a.json`, `performance-3.7c.json`은 루프백 OpenSSH에서 SSH 연결→snapshot, 입력→control callback 20표본의 p50/p95, 20,000줄 지속 출력 처리량, 종료 시점 JVM used heap을 기록한다. 렌더러 화면 표시 지연·앱 cold start·최대 RSS·Windows/Android 기기 성능은 측정하지 않았다. 정량 값은 `artifacts/DELIVERY.md`에 기록한다. JCEF의 메모리·패키지·지연 비교는 미실행이다.

## 인수 A–F

| 시나리오 | 확인한 부분 | 남은 인수 검증 |
|---|---|---|
| A Windows→Android 동일 Codex | JVM·Android 각각 SSH 종료/재접속 후 동일 pane PID | 실제 Windows 앱→Android 기기, 실행 중 Codex |
| B 모바일 네트워크 변경 | 강제 transport 단절·백오프·입력 폐기·복원, 앱 복귀 | Wi-Fi↔셀룰러 전환, 절전·장시간 백그라운드 |
| C 표준 SSH/tmux | 일반 SSH exec로 만든 세션과 Control Mode 상호 조작, 외부 변경, 동시 연결 | Windows·기기에서 일반 terminal attach와 GUI 교차 인수 |
| D GUI 관리 | 공유 GUI 구현, 명령 통합 검사, Android GUI 분할·선택·창 생성·폰트 변경 | 전체 GUI 생성·이름 변경·resize·삭제 확인의 Windows/기기 점검 |
| E 프로세스 안전 | 정상 종료·강제 단절·반복 복구 후 동일 PID, control client 회수 | 앱 프로세스 강제 kill·OS 종료·장시간 에이전트 |
| F Codex·Claude 호환 | Unicode·ANSI·alternate screen·IME 합성·연속 출력 | 실제 Codex·Claude 로그인·권한 요청·TUI·실행 중 재접속 |

어느 A–F도 전체 플랫폼 인수 완료로 표시하지 않는다.

## 실기와 릴리스 체크리스트

- [ ] Windows 10/11 x64 설치·실행·제거, 사용자 DPAPI 저장·tamper·다른 사용자 접근 거부
- [ ] Windows 한글/일본어 IME, 선택·복사·붙여넣기, 폰트 변경, JediTerm 필수 호환 판정
- [ ] Android 14/15/16 실기 Gboard·삼성 키보드, 조합/삭제/붙여넣기 중복 없음, selection·scroll·IME 단축키
- [ ] 실제 네트워크 전환·절전·회전·폴더블 화면 크기 변경 후 같은 프로세스 복구
- [ ] Codex·Claude Code의 실제 권한 요청·대화형 탐색·대량 출력·alternate screen·실행 중 재접속
- [ ] JCEF 대안의 실행 비교가 필요한지 Windows 결과로 결정
- [ ] 현재 SGR pen 등 tmux snapshot 한계가 실제 TUI 필수 동작을 방해하는지 판정
- [ ] 빌드 도구 advisory와 전체 전이 의존성 라이선스, 배포 서명, GitHub 보안 설정 확인

세부 보안 결과는 [SECURITY.md](SECURITY.md), 기술 한계는 [화면 복원 결정](decisions/002-terminal-restoration.md)에 기록했다. 에뮬레이터나 합성 테스트 결과를 실기/실제 에이전트 통과로 대체하지 않는다.

P4-03의 전용 AVD 자동 실행: `python3 scripts/verify_android_matrix.py` (API 34/35/36, tmux 3.7c). 결과는 `artifacts/android-matrix.json`에 저장한다.


## P4-03 원격 CI 후속 (2026-10-23)

사용자 요청에 따라 Windows 실검증은 사용자가 직접 수행한다. 당시 일반 push는 Linux 기본 검증,
tmux 3.3a/3.7c, Android API 34/35/36의 **6개 실행 단위**를 실행했다. 현재 실행 범위는
[README의 검증 안내](../README.md#검증)를 따른다. Windows job은
`workflow_dispatch`의 `verify_windows=true`로 선택 실행할 수 있게 준비했으나,
실행하지 않은 Windows 결과를 통과로 간주하지 않는다. P4-03 전체 상태는 진행 중이다.

로컬 후속 검증: Python 보안/증거 회귀 20개 통과. 임시 Git 저장소에서 `core.autocrlf=true/false`
각각 실제 커밋·체크아웃을 수행해 원문과 라이선스 증거 전체의 바이트/해시 보존을 확인했다.
일반 소스는 LF, 배치 파일은 CRLF, 원문과 `licenses/**`는 `-text`를 적용한다.
[Git 속성 규칙](https://git-scm.com/docs/gitattributes)을 따른다.
JVM 기본 검사는 13 통과·예상된 플랫폼/SSH fixture skip 7개이며, 테스트 이름 전체를 대조한다.
SSH CI는 19 통과·Windows skip 1개, 선택 Windows CI는 14 통과·SSH fixture skip 6개가 필요하다.
Android는 지정된 계측 테스트 4개 모두 실행되어야 하며 skip/누락은 실패한다.

Windows 자동화 재현은 관리자 권한의 폐기 가능한 `windows-2022` Actions runner를 대상으로 한다.
`verify_dpapi_users.ps1`은 두 임시 사용자·프로필·공유 자료·자식 프로세스를 정리하며,
사용자별 왕복 및 상대 암호문 읽기/복호화 거부를 양방향 검사한다.
`verify_windows_installers.py`는 MSI/EXE를 순차 설치하고 등록/런타임/Temurin 원본 소스·고지·해시,
설치된 JediTerm recipe로 두 JAR 재빌드·교체 후 기동, 제거 후 잔여 파일을 검사한다.
8초 프로세스 생존은 GUI/IME 인수 결과가 아니다. 이 자동화는 로컬에서 구문/경계만 검증했고
Windows 실행 결과는 아직 없다.

Windows 패키지를 직접 빌드할 때는 Eclipse Temurin JDK 17, Android SDK 36/build-tools 36.0.0,
WiX 3 도구를 준비하고 아래 순서로 실행한다. 소스 확보가 실패하면 패키징도 실패한다.

```powershell
python scripts/check_prompt.py
python scripts/verify_wrapper.py
python -m unittest discover -s scripts/tests -v
.\gradlew.bat :app:desktopTest :app:prepareDpapiProbe writeDependencyInventory
python scripts/verify_test_results.py --mode windows --output artifacts/windows-tests.json
python scripts/audit_dependencies.py
python scripts/verify_licenses.py
python scripts/prepare_temurin_source.py
.\gradlew.bat :app:packageMsi :app:packageExe
# 다음 두 명령은 폐기 가능한 Actions runner 전용이다.
./scripts/verify_dpapi_users.ps1
python scripts/verify_windows_installers.py
python scripts/collect_artifacts.py --platform windows
```

Windows collector는 APK를 요구하지 않으며 세 Windows 보고서가 동일 커밋·실행에 대해 완료/통과이고
검증한 설치물 해시와 일치해야 수집한다. `windows-build-manifest.json`, `windows-SHA256SUMS`,
`windows-third-party.zip`으로 Android 산출물과 분리한다. 무인자 collector는 기존 Android 동작을 유지한다.
원격 실패 시에도 명시한 JSON/해시 보고서만 업로드하며 원시 로그·임시 계정/키 자료는 포함하지 않는다.
설치물 업로드는 Windows 검증 성공 후에만 허용한다.

원격 실행 URL·커밋·통과 수·다운로드 해시는 실제 실행 후 아래에 기록한다.
Windows 10/11 실기·IME, 실제 DPAPI 키 손실, Android 실기, 실제 에이전트, 서명 배포는 후속이다.


### 원격 실행 확인

- 실행: [https://github.com/jujinkim/dgl-client/actions/runs/39898373064](https://github.com/jujinkim/dgl-client/actions/runs/39898373064), 커밋 `899ad82b540a5b284c59630b0713a8950b09de4a`, attempt 1. 6개 필수 작업 모두 성공, Windows는 사용자 담당으로 의도한 job skip.
- 환경: GitHub Actions `ubuntu-24.04`, Ubuntu 24.04.5 x86_64, runner image `20261012.334.1`, Temurin `17.0.20+1`, Node `22.23.4`, Gradle `9.7.0`.

| 실행 단위 | 통과 | 실패 | 예상 skip |
|---|---:|---:|---:|
| Linux 기본 JVM | 13 | 0 | 7 (SSH fixture 6, Windows 1) |
| tmux 3.3a JVM | 19 | 0 | 1 (Windows) |
| tmux 3.7c JVM | 19 | 0 | 1 (Windows) |
| Android API 34 | 4 | 0 | 0 |
| Android API 35 | 4 | 0 | 0 |
| Android API 36 | 4 | 0 | 0 |

원격 보안 회귀는 당시 19개 통과, OSV는 371버전/경고 0, npm audit 0,
Gitleaks 작업 트리·전체 index·전체 이력 통과. Lint 승인 조건 검사와 APK 정책 검사 통과.
현재 로컬은 Temurin sidecar 검증 회귀를 추가한 20개 통과다.
고지 묶음은 해당 Linux JDK 기준 485개 파일이며 로컬 macOS의 477개와 구분한다.

다운로드한 Actions artifact ZIP 7개의 SHA-256을 GitHub API digest와 대조했고,
각 테스트 JSON의 커밋·run ID·attempt 일치를 확인했다. APK 36,166,932 bytes의
SHA-256 `ecdb1b5b68894efe738641278c23dfa7a6e4f5b708e092ad149b0e6808df4ea6`는 수집 manifest와 다운로드 파일에서 일치했다.
로컬 보존 경로는 `artifacts/remote-ci-39898373064/`이며 Actions 보존 기간은 7일이다.
이 실행은 고지 ZIP을 업로드하지 않았으므로 고지의 다운로드 대조는 아직 수행하지 않았다.
후속 수집 보완은 APK·고지 ZIP·manifest·SHA256SUMS를 함께 업로드하고,
해당 최종 커밋의 별도 실행 및 다운로드 검증을 `artifacts/REMOTE_CI.md`에 기록한다.

처음 실행의 cold-cache parent POM/BOM 메타데이터 누락과 AVD 검색 경로 불일치는 수정했다.
추가 metadata 3개는 공식 Maven Central에서 독립 재수신해 해시를 확인했으며 잠금 버전은 바꾸지 않았다.
빈 Gradle 캐시의 별도 checkout에서도 desktop test, debug/release/계측 APK, lint, inventory를
실행하고 엄격한 무결성 검사로 다시 통과했다.

GitHub Secret scanning / Push protection: **확인 불가**. GitHub 연결 도구의 정규화된
저장소 응답과 인증 없는 REST 조회에서 `security_and_analysis`가 제공되지 않았다.
CLI는 미인증 상태이며 SSH push 권한은 확인했다. 설정은 변경하지 않았다.
[GitHub 저장소 조회 API](https://docs.github.com/en/rest/repos/repos#get-a-repository)의 가시성 제한과 구분해 기록한다.

## P3-03 자원 측정 도구

`python3 scripts/verify_resources.py --platform all --profile soak`는 macOS JVM
× tmux 3.3a/3.7c, 전용 Dgl_API34/35/36 AVD × tmux 3.7c를 순차 실행한다.
기본값은 `all`, `soak`이며 변경 사항을 커밋한 깨끗한 작업 트리가 필요하다.
`--profile smoke`는 예열 20초·각 부하 30초·반복 2회·종료 관찰 10초로
수집기와 수명주기를 검사한다. smoke에는 2분 비교 구간이 없으므로 메모리
합격 판정을 하지 않으며 `qualifiesAsSoak=false`를 기록한다.
smoke는 각 부하 구간에서 지표가 실제 수집됐는지 확인하며, 장기 검증의 2분
비교 창과 최소 표본 수 조건은 soak에만 적용한다. 화면 복원 중에도 앱과
렌더러 메모리는 유효 표본이며, 연결 종료/화면 교체 경계를 가로지른 조회는 제외한다.

본 측정은 환경마다 예열 5분, 동일 유휴 상태 기준선 20초, 출력·강제 단절 복구·
정상 종료/터미널 재생성 각 10분, 종료 후 관찰 2분이다. 5환경 본 부하는
150분이며 예열·관찰·기준선·빌드·부팅 시간은 별도다. 복구와 재생성은 각각
20회다. 전용 원격 Python 프로세스는 Unicode/ANSI 행을 약 64 KiB/s로 출력하고
PID·진행 번호·수신 입력 바이트 수만 덮어쓴다. 복구 중 입력은 차단되어야 하고
같은 원격 PID가 유지되어야 한다. 회복 후 control client 1개, 종료 후 0개를
매번 검사한다. 소비자를 잠시 지연시켜 출력 제어도 실행한다.

JVM은 Gradle daemon과 분리한 프로세스에서 실제 ClientModel → DesktopConnector
(256 KiB pipe) → JediTerm 스트림 파서/터미널 버퍼를 사용한다. 그래픽 디스플레이는
headless이며 Swing 페인팅·Windows GUI/IME를 검증한 것으로 해석하지 않는다.
Android는 별도 `-PdglResourceProbe=true` 계측 APK로 실제 MainActivity/Compose와
AndroidTerminal/WebView/xterm을 실행하고 ActivityScenario로 화면을 재생성한다.
일반 JVM 20개·Android 4개의 정확한 테스트 목록과 기본 runner는 그대로 유지한다.
앱 배포 코드·공개 API·저장 형식에는 측정용 API를 추가하지 않는다.

앱 힙·GC·FD·스레드·scrollback·출력 버퍼 지표는 2초 간격으로 수집한다.
스레드 판정은 macOS `ps -M` 및 Android `/proc/self/status`의 OS 스레드 수를
사용한다. JVM의 Java 스레드 수는 별도 `javaThreads` 지표로 남긴다.
Android 상세 메모리는 10초 간격이며, `dumpsys activity services`의 호스트 앱
바인딩에서 렌더러 PID를 찾아 각 PID의 RSS/PSS를 별도로 합산한다. 패키지의
`meminfo`만으로 렌더러 메모리를 대체하지 않는다.
[Android WebView 메모리 측정 지침](https://developer.android.com/develop/ui/views/layout/webapps/manage-webview-memory)을 따른다.
프로세스 교체·누락·수집 공백·실패·중단은 실패 근거로 기록한다. 정상 화면 재생성
중 렌더러 PID 교체는 기록하되 앱 PID 교체는 실패다. 샘플 최대 RSS와 OS 최대
RSS(macOS time, Android VmHWM)를 구분한다. 수집할 수 없는 값은 null/누락이며
0으로 보완하지 않는다. 단말 앱 ANR/crash 이벤트도 확인한다.
상세 메모리 조회는 단일 별도 worker에서 수행하여 2초 앱 지표 수집을 막지 않는다.
조회 시작·종료 시각을 기록하며 메모리 비교 구간은 결과 병합 시각 대신 조회
시작 시각을 사용한다. 조회 중 단계나 연결 상태가 바뀐 결과는 구간 비교에서 제외한다.
Android UI 지표 조회는 Main dispatcher에서 실행한다. 테스트 helper의
`onActivity`를 작업 스레드에서 호출하면 `waitForIdleSync`를 기다려 연속 출력
중 수집과 단계 시작을 지연시킬 수 있기 때문이다.
[AndroidX ActivityScenario 구현](https://github.com/android/android-test/blob/main/core/java/androidx/test/core/app/ActivityScenario.java)을 확인하고 이 대기를 제거했다.

고정 기준: 각 10분 단계의 첫/마지막 2분 중앙값 RSS/PSS 증가가
`max(32 MiB, 시작값 × 10%)` 이하여야 한다. 힙·GC는 함께 제시하되 RSS 증가만으로
누수를 단정하지 않는다. 예열 후와 종료 후 유휴 FD·스레드 중앙값 증가는 각각
4 이하여야 한다. 이전 터미널은 약한 참조로만 추적하며 종료 후 남으면 실패다.
유휴 기준선에는 Disconnected 상태만 포함하며 다음 연결 준비는 별도 구간이다.
터미널 객체 회수는 관찰 종료 시점의 0개 조건으로 판정한다. 비동기 GC가 완료되기
전후 객체 수의 중앙값을 최종 잔존 객체 수로 사용하지 않는다.
SSH 소켓은 유휴 상태의 관찰용 admin 연결 1개를 제외하고 회수되어야 한다.
scrollback 10,000줄과 출력 버퍼 상한, 반복 완료, 출력 진행과 속도도 확인한다.
메모리 판정 수치는 결과에 맞추어 완화하지 않는다.

실행별 `artifacts/resources-{profile}-{UTC}/`에 `matrix.json`, 환경별 `summary.json`,
`samples.jsonl`, 프로세스 진단을 저장한다. 요약에는 코드 커밋·작업 트리 변경 여부,
환경·시간·반복 횟수·기준·판정이 포함된다. 터미널 원문과 실제 자격 증명은
기록하지 않는다. 임시 SSH 키/서버/tmux socket, 자체 생성한 AVD/앱 프로세스,
adb reverse 포트는 종료·실패·중단 시 정리한다.

현재 도구 구현과 smoke/회귀 확인 단계이며, 아래에 최종 커밋의 본 측정 결과가
기록되기 전에는 P3-03 장시간 검증 통과로 취급하지 않는다. Windows는 사용자
검증 담당, Android 실기는 후속 범위로 유지한다.

### 부하 smoke에서 재현한 복구 결함

API 35에서 출력이 밀린 상태로 SSH를 강제 종료하면 소켓이 사라진 뒤에도
`Ready` 상태에서 과거 버퍼를 처리하여 복구 전환이 지연됐다. 기존 RecoveryTest에
수신 버퍼를 채운 뒤 연결을 닫는 검사를 추가해 수정 전 실패를 확인했다.
SshStream은 자신을 만든 SSH 연결의 동일성/연결 상태를 읽기 전후에 확인하여
닫힌 연결의 버퍼를 더 전달하지 않도록 수정했다.

또한 제어 레코드 중간의 EOF가 `ProtocolException`으로 분류되어 재접속을 차단하는
경우를 별도 fake stream으로 재현했다. EOF는 이전 파서 상태를 폐기하고 네트워크
단절로 복구하며, 완성된 잘못된 레코드의 파싱 오류와 원격 `%exit` 처리는 유지한다.
재현 검사는 기존 RecoveryTest 안에 두어 정확한 JVM 20개 목록을 유지한다.

후속 API 35 smoke에서는 정상 종료 시 약 30초 멈춰 재생성 구간이 초과했다.
출력을 읽지 않는 실제 SSH/tmux 소비자로 회귀를 재현했으며, 수정 전 테스트는
33.5초 후 종료 지연 assertion으로 실패했다. 연결을 먼저 끊는 초기 수정은
tmux 3.3a에서 원격 control client를 남겼으므로 최종 방식으로 채택하지 않았다.
최종 수정은 소비자 coroutine을 취소하고 stdin EOF를 보낸 뒤, 남은 수신 데이터를
작은 고정 버퍼로 버리고 EOF 응답 후 채널을 닫는다. 1.5초 안에 정상 종료하지
못하면 연결을 종료한다. 양 버전에서 2초 이내 종료와 원격
control client 0개를 함께 검사한다. fixture 정리도 kill-server 반환만 믿지 않고
이번 임시 경로/소켓의 프로세스와 자식을 확인하여 잔여 프로세스를 종료한다.
EOF 응답을 기다리기 전 중간 구현(`d9b3df1`)은 로컬 5환경 smoke와 회귀를
통과했으나 Linux tmux 3.3a CI에서 종료 지연이 다시 재현됐다. 최종 구현은
EOF 응답을 기다리며 종료 대기 상한도 적용한다. 이 중간 결과는 최종 근거로
대체하지 않고 실패 기록으로 보존한다.
PTY 셸에는 채널 종료로 hangup을 전달하여 셸 종료 후에도 다른 SSH 채널을
사용할 수 있게 한다. 이 차이는 기존 일반 SSH 셸 회귀 검사로 확인한다.
이전 실패 결과는 보존하며 시간 기준을 늘려 통과시키지 않는다.
최종 5환경 장시간 측정은 이 수정 이후 코드로 수행해야 한다.


## Android 볼륨 키 단축키·매크로 (2026-10-23)

`main` 작업 트리에 Android 전용 설정과 입력 등록을 추가했다. 기본값은 비활성 및
위·아래 모두 기본 음량 조절이다. 단축키와 매크로는 같은 순서 있는 텍스트/키 단계로
저장되며, 편집기에서 저장하기 전에는 기존 설정을 바꾸지 않는다. 기존 설정 파일도
기본값으로 읽고, 저장 후 새 저장소 인스턴스에서 동일한 값을 복원한다.
공통 단축키 바의 구성·순서는 그대로이며 키 목록과 바이트 변환을 공유한다.

[Android KeyEvent](https://developer.android.com/reference/android/view/KeyEvent#getRepeatCount())의
반복 횟수와 기기·키·누름 시작 시각을 함께 확인한다. 최초 누름만 실행하고 같은 누름의
반복·해제는 소비한다. 화면이 바뀌면 이전 실행 대상 등록을 해제하고 누름의 소비 여부만
해제 이벤트까지 보존한다. 포커스 상실·백그라운드 이동·Activity 파괴 시 누름 상태도 정리한다.
전면 복귀 때는 현재 화면의 등록만 복구한다. 소프트 키보드 여부는 실행 조건에 넣지 않는다.
대화상자·호스트 목록만 보이는 화면·입력 불가능 상태에서는 기본 처리로 넘긴다.
[Activity.dispatchKeyEvent](https://developer.android.com/reference/android/app/Activity#dispatchKeyEvent(android.view.KeyEvent))의
공개 재정의 지점에서 처리하며 미처리 키는 AndroidX superclass로 전달한다.
해당 메서드에만 AndroidX Core의 상속된 `RestrictedApi` 진단을 억제했고 이유를 코드에 남겼다.

매크로는 실제 줄바꿈을 Enter로 바꾼 UTF-8 바이트 배열 하나로 `ClientModel.input()`에
전달한다. 기존 입력 대기열·세대 검사·실패 입력 재전송 금지는 유지한다. 빈 내용과
64 KiB 초과는 저장 전에 오류를 표시한다. 단계별 JSON 부피를 수용하도록 설정 파일의
읽기/쓰기 상한을 동일한 16 MiB로 두고, 저장 전에 크기를 검사한다.

볼륨 키 변경 당시 명시적 테스트 목록은 JVM 26개, Android 8개였다. JVM 기본 검사는 18개 통과와
SSH fixture 7개·Windows 1개의 예상 skip, SSH fixture 검사는 25개 통과와 Windows 1개의
예상 skip을 요구한다. Android 결과 검증기와 AVD matrix는 8개 전부의 실행·통과를 요구한다.
테스트 결과 검증기 회귀를 포함한 Python 테스트 48개도 통과했다.

검사 범위는 전체 지원 키 바이트, 기존 바 순서, 혼합 단계 순서, 한글·Unicode,
실제/문자열 줄바꿈 구분, Enter 자동 추가 없음, 변환 후 크기 경계, 구형 설정과 저장·복원,
위·아래 독립 지정, 비활성·미지정 처리, 중복 down·반복·up, 실행 대상 교체와 수명주기다.
Android 화면 검사는 활성화 스위치, 편집기 오류·추가·수정·삭제·순서 변경·저장·취소,
설정·호스트 편집·삭제 확인·알림 대화상자, 호스트 목록, 복구·해제, 회전·백그라운드 복귀를 포함한다.
실제 SSH/tmux 화면 검사는 키보드 표시·숨김 상태에서 선택한 패널의 카운터가 한 번만 증가하고
다른 패널은 그대로인지 확인한다. JVM에서는 tmux 3.3a/3.7c의 선택 패널과 실제 일반 SSH PTY의
매크로 전송을 확인한다. Android 키 이벤트는 Activity에 계측 주입하며, 실기 물리 키 검증은 미실행이다.

로컬 결과: `artifacts/volume-jvm-base.json`, `volume-jvm-3.3a.json`, `volume-jvm-3.7c.json`.
Android matrix와 최종 빌드 결과는 아래에 기록한다. 이번 결과는 로컬 작업 트리 검증이며
새 원격 CI 실행이나 Windows 실기 검증을 의미하지 않는다.

| 검증 | 결과 |
|---|---|
| JVM 기본 / tmux 3.3a / tmux 3.7c | 18 / 25 / 25 통과, 예상 skip 8 / 1 / 1 |
| Android API 34 / 35 / 36 ARM64 | 각각 8 통과, 실패·skip 0 |
| Android Debug / Release / 계측 APK | 빌드 통과 |
| Android lintDebug 및 lint 결과 검증기 | 통과, 기존 검토된 경고 12개 |
| Python 회귀 | 48 통과 |

Android 증거는 `artifacts/android-matrix.json` 및 `android-tests-{34,35,36}.json`에 있다.
첫 API 34 실행에서 스위치 조작 테스트가 실패하여 스위치와 설명을 하나의 접근 가능한
행으로 구성한 뒤 세 API 전체를 다시 통과했다. 복구 애니메이션 중 계측의 idle 대기를
피하도록 테스트의 Activity 조회도 Main dispatcher에서 수행한다.
최종 빌드 로그는 `.tools/volume-release-lint-final.log`, 전체 Android 로그는
`.tools/volume-android-matrix-final.log`다. 실행한 전용 에뮬레이터와 임시 SSH/tmux fixture는 종료했다.

### 2026-10-23 tmux 관리 영역 표시 수정

일반 SSH 모드에서도 세션·창·패널 목록 영역과 조작 버튼을 유지하고, 사용할 수 없는
조작만 비활성화한다. `현재 세션`은 관리 영역을 접어도 남으며, 연결 안 됨·복구 중·
일반 SSH·세션 미선택 상태를 구분한다. 목록에 `세션`, `창`, `패널`, `조작` 표제를 추가했다.

JVM 기본 테스트 18개 통과·예상 skip 8개, Debug 및 계측 APK 빌드, `lintDebug`와
기존 경고 12개 검증기를 통과했다. Android API 36 ARM64와 격리 OpenSSH/tmux 3.7c에서
8개 계측 테스트를 모두 통과했다. 일반 SSH의 버튼 표시·비활성화와 접기·펼치기,
tmux의 현재 세션 표시 유지·분할·창 생성·키보드·회전 검사를 포함한다.
로그는 `.tools/tmux-ui-validation.log`, 결과는 `artifacts/android-tests-36.json`이다.
이번 UI 변경에 대해 API 34/35 및 물리 기기는 재검증하지 않았다.

APK의 앱 ID `kim.jujin.rovessh`, 버전 `0.1.0`/`1`, 출력 경로와 문서 등록 경로는
기존 대시보드 메타데이터와 일치하며 대시보드 검증기를 통과했다.

### 2026-10-23 로그인 PATH의 tmux 탐지 수정

로컬 macOS의 tmux 3.7c는 `/opt/homebrew/bin/tmux`에 설치되어 있지만 SSH 기본 PATH
`/usr/bin:/bin:/usr/sbin:/sbin`의 비로그인 zsh에서는 `tmux -V`가 exit 127로 실패했다.
같은 PATH에서 `zsh -lc`로 실행하면 `.zprofile`의 Homebrew 설정을 읽고 정상 응답했다.
절대 경로 실행으로 기존 서버의 세션 조회가 성공하는 것까지 읽기 전용으로 확인했다.
이 차이는 [zsh의 로그인 파일 처리](https://zsh.sourceforge.io/Doc/Release/Files.html)와
[sshd의 명령 실행 방식](https://man.openbsd.org/sshd.8#LOGIN_PROCESS)에 부합한다.

SSH 기본 PATH에서 먼저 실행 파일을 찾고 실패하면 로그인 셸의 exported PATH로 재탐색한다.
배너 출력과 경로를 구분하며, 절대 경로만 인용하여 버전 조회·세션 생성·목록·Control Mode
연결에 재사용한다. 경로 캐시는 SSH 연결별로 유지하고 재연결 시 다시 찾는다.
경로 미발견·실행 실패·버전 미지원 메시지도 구분하며, 일반 SSH 전환 시 마지막 탐색 위치를
지우지 않는다. 호스트의 셸 설정이나 tmux 설정은 수정하지 않았다.

`scripts/integration.py --tmux-login-path`는 비로그인 PATH를 비우고 별도 로그인 셸에서만
격리 tmux 래퍼를 찾게 한다. 공백·작은따옴표가 포함된 설치 경로, 로그인 배너,
세션 생성·새로고침·선택·입력·분할·재접속을 실제 SSH로 통과했다.
이 변경 시점의 JVM 테스트 목록은 29개였다. tmux 3.3a/3.7c의 전체 통합 검사는 각각 28개 통과와
Windows 전용 1개 skip이며, Android API 36의 8개 계측 테스트와 Debug/계측 APK 빌드,
lint 및 기존 경고 12개 검증, Python 회귀 48개를 통과했다. API 34/35와 물리 기기는
이번 변경에서 재검증하지 않았다.

로그: `.tools/tmux-login-path-test.log`, `.tools/tmux-discovery-integration.log`,
`.tools/tmux-discovery-3.3a.log`, `.tools/tmux-discovery-android.log`.
결과: `artifacts/tmux-discovery-jvm-{3.3a,3.7c}.json`, `artifacts/android-tests-36.json`.

### 2026-10-23 tmux 다중 경로 탐지와 실패 상세 안내

SSH 기본 PATH, 로그인 셸 PATH, 대표 설치 경로 순서로 탐지를 확장했다. 대표 경로는
Homebrew·Linuxbrew·MacPorts, 표준 시스템 bin, 사용자 `~/.local/bin`·`~/bin`, Nix·Snap을
포함한 12곳이다. 각 탐지 단계에 4초 응답 제한을 두고 경로 미발견·명령 실패·응답 오류·
시간 초과를 기록한다. 네트워크 오류와 외부 취소는 탐지 실패로 바꾸지 않는다.

일반 SSH 상태의 실패 라벨 바로 옆에 접근성 설명이 있는 ⓘ 버튼을 추가했다.
대화상자에는 호스트, 실패 이유, 실제 시도한 방법과 결과, 대표 검사 경로, 발견된 실행
파일과 버전(있는 경우), 확인 명령과 다음 조치를 표시한다. 원격 표준 출력·오류 전체를
진단 내용에 저장하지 않는다. 관리 영역을 접어도 상세 안내를 열 수 있고, 안내 중에는
볼륨 키 매크로를 터미널에 전달하지 않는다. 재연결하면 이전 진단을 지우고 다시 탐지한다.

JVM 목록은 30개이며 기본 21개 통과·예상 skip 9개, 격리 OpenSSH/tmux 3.7c 전체 검사
29개 통과·Windows 전용 skip 1개다. 별도 로그인 PATH 회귀는 공백·따옴표 경로와 실제
후보 경로 루프(없는 파일·실행 불가 파일 건너뛰기)도 검증했다. Android API 36의 8개
계측 검사가 모두 통과했고, 상세 안내 열기·스크롤·닫기와 매크로 차단·입력 복귀를 포함한다.
Debug/계측 APK, lint와 기존 경고 12개 검증, Python 회귀 48개도 통과했다.
이번 변경에서 API 34/35, tmux 3.3a 및 물리 기기는 재검증하지 않았다.

로그: `.tools/tmux-details-{build,login-path,integration,android,python}.log`.
결과: `artifacts/tmux-details-jvm-base.json`, `artifacts/tmux-details-jvm-3.7c.json`,
`artifacts/android-tests-36.json`. 대시보드 등록 경로·앱 ID·파일 버전은 기존 값과 일치하며
메타데이터 검증기를 통과했다.

### 2026-10-24 R1 API36 화면 표시 계측 훅

실제 앱·xterm의 테스트 전용 입력/그리기 표식과 Perfetto SurfaceFlinger 표시 시각을
연결했다. 최종 짧은 검증은 입력 8회·스크롤 표시 간격 80개, 연결 종료 후 control client 0,
원격 입력 0, 전용 AVD 정리로 통과했다. Python 관련 20개·JVM fixture 6개와 Android
debug/계측 APK 빌드도 통과했다. 종료 후 기본 `AndroidJUnitRunner`를 확인했다.

증거: `artifacts/migration-r1/android-presentation-20261023T195311Z/summary.json`.
입력 값은 첫 표시 시각의 확정치가 아닌 drawable-frame 표시 상한이며, 정식 표본 수·
resize/탭 전환·오버헤드와 Mac GUI 계측은 남아 있다. R1 전체 통과나 R2 채택을 의미하지
않는다. 실패 시도와 세부 결과는 [R1 보고서](migration/R1_BASELINE.md), 이어갈 작업은
[NEXT_WORK.md](../NEXT_WORK.md)를 따른다. 중단된 장시간 soak나 전체 matrix는 재개하지 않았다.


## 2026-10-24 Rust Mac·Android 후보

R1 성능 계측과 기능 검증을 분리하고 Rust/UniFFI/GPUI 후보를 추가했다.
120 공통 fixture, 제한 큐·취소·종료, 격리 OpenSSH/tmux 3.3a·3.7c 인증·자동 복구·동일 PID,
API36 Rust 후보의 5개 계측 검사를 실행했다. 명령·산출물·정확한 범위와 미검증 항목은
[migration/CANDIDATE.md](migration/CANDIDATE.md)를 따른다. 이 기록은 기존 R1 성능 결과,
Windows/실기/실제 에이전트 미완료 상태를 변경하지 않는다.

Mac GUI 창 조회와 Keychain 쓰기(-25308)는 현재 세션에서 검증을 진행할 수 없어 대기다.
개발용 번들 생성이나 합성 IME를 실제 GUI/물리 입력 인수로 대체하지 않는다.
