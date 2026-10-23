# 실행 검증과 인수 상태

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

사용자 요청에 따라 Windows 실검증은 사용자가 직접 수행한다. 일반 push는 Linux 기본 검증,
tmux 3.3a/3.7c, Android API 34/35/36의 **6개 실행 단위**를 실행한다. Windows job은
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
