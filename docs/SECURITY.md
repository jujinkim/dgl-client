# 보안 모델과 공개 전 확인

## 앱 경계

- 호스트키는 `SHA256:` 지문으로 확인한다. 처음 보는 키는 인증 전에 연결을 중단하고 사용자 확인을 받는다. 기존 지문 불일치는 자동 재시도하지 않는다.
- Android는 no-backup 디렉터리의 AES-GCM ciphertext와 Android Keystore 키를 사용한다. Windows는 CURRENT_USER DPAPI를 사용하며 MACHINE scope를 사용하지 않는다. 보호 실패 시 평문 저장을 만들지 않는다.
- macOS 개발 실행의 MemoryCredentialStore는 명시적 개발 모드이다. Windows/Android 보호 저장 실패의 대체 수단으로 사용하지 않는다.
- 비밀번호·개인키·키 암호는 메모리에 존재한다. JVM 문자열의 완전한 메모리 삭제를 보장하지 않는다. 디스크에는 보호된 자격 증명만 저장한다.
- 원격 출력·사용자 입력·자격 증명을 기본 로그로 기록하지 않는다. SLF4J no-op을 사용한다. 진단 코드는 예외 클래스 이름만 포함한다.
- WebView는 고정 HTTPS appassets origin의 allowlist 파일만 읽는다. file/content 접근·외부 탐색·네트워크 요청·권한 요청·OSC52 clipboard 접근을 차단한다. bridge는 main frame/origin/type/크기/범위를 검사한다. 출력은 base64로만 JS 함수에 전달한다.
- 클립보드 복사는 사용자가 직접 생성 공개키 복사 또는 터미널 선택 복사를 실행한 경우에 한한다.
- `disconnect()`에는 kill-session/window/pane 명령이 없다. 삭제 확인은 표시 시점의 대상 ID를 고정한다.

## 저장소·CI

`scripts/scan_secrets.py`는 Git이 관리할 작업 파일, staged diff, 전체 refs 이력을 Gitleaks 8.30.1로 검사한다. 결과에는 규칙과 위치만 출력하며 탐지 값은 출력하지 않는다. 테스트 SSH 키는 임시 디렉터리에서 생성·폐기한다. xterm 6.0.0의 두 exported class 이름(`FourKeyMap`, `SequencerByKey`)만 파일 경로와 함께 좁게 예외 처리했다. npm integrity와 번들 비교를 별도로 수행한다.

CI는 contents:read, Action SHA 고정, persist-credentials:false, PR에 비밀정보·쓰기 권한 없음, APK/MSI/EXE 경로만 업로드를 사용한다. `pull_request_target`과 release 자동 게시를 사용하지 않는다. Wrapper 배포 checksum과 wrapper JAR 공식 SHA-256을 확인했다. Gradle artifact SHA-256 verification, dependency lock, npm lock을 커밋 대상으로 둔다. 검증 메타데이터 재생성은 자동 업데이트로 승인하지 말고 변경 원인을 검토한다.

## 선택 의존성과 실패 기준

2026-10-23 P4-03 로컬 갱신: Kotlin/Compose compiler **2.4.21**, AGP **9.3.3**, Gradle **9.7.0**, JDK **17**, Build Tools **36.0.0**. Compose Multiplatform 1.9.3과 앱 직접 의존성은 유지했다. `:app`은 Android KMP library·desktop, `:androidApp`은 AGP 내장 Kotlin 앱이다. 앱 ID·SDK·저장 형식·Keystore 별칭은 유지했다.

공식 근거: [AGP 9 KMP 이전](https://kotlinlang.org/docs/multiplatform/multiplatform-project-agp-9-migration.html), [AGP 9.3 호환성](https://developer.android.com/build/releases/agp-9-3-0-release-notes), [Compose 1.9.3의 AGP 9 지원](https://kotlinlang.org/docs/multiplatform/whats-new-compose-110.html), [Kotlin 캐시 수정 advisory](https://github.com/advisories/GHSA-r937-wjx7-w2jp). 빌드 캐시·Kotlin incremental·configuration cache는 비활성 상태다.

```sh
./gradlew writeDependencyInventory
python3 -m unittest discover -s scripts/tests -v
python3 scripts/audit_dependencies.py
python3 scripts/verify_licenses.py
```

`artifacts/dependency-inventory.json`은 Gradle `ResolutionResult`의 **선택된 버전**만 기록한다. 모듈·configuration·runtime/build/test·플랫폼·선택 사유·artifact SHA-256과 요청→선택 그래프의 모든 간선을 보존하므로 유입 경로를 추적할 수 있다. compiler plugin, plugin classpath, 별도 `androidLintTool`, 테스트와 AAPT2 세 OS 분류를 포함한다. npm v3 lock의 실제 이름(중첩·alias 포함), 버전, dev 구분과 integrity도 검사한다. 검증 XML은 무결성 확인용이며 과거 버전을 OSV 검사 입력으로 사용하지 않는다.

Windows x64·Linux x64·macOS ARM64 목록은 실제 desktop runtime과 일관된 버전 선택을 사용하고 각 OS의 Skiko native만 포함한다. AAPT2는 build 범위다. 누락된 필수 그래프·패키지·잘못된 artifact 해시·입력/lock 변경 후 오래된 inventory·OSV 응답 개수 누락/오류/통신 실패는 실패 처리한다. OSV 결과는 ecosystem·이름·버전·advisory로 합치되 모든 사용 범위를 보존한다.

현재 검사: **371개 패키지 버전, 런타임 0건·빌드/테스트 0건, 예외 0건**. 기존 92건은 과거 메타데이터 기반 수치이므로 현재 선택 그래프와 직접 비교할 수 없다. 중간 검사에서 빠져 있던 lint 전용 그래프를 추가했을 때 발견한 7건도 수정 후 재검사했다. 검사 시점의 OSV DB 결과이며 미래 취약점 부재를 보증하지 않는다.

수정 범위는 다음과 같다. 광범위한 application configuration 강제 치환은 없다.

| 구성 | 수정 및 유입 경로 |
|---|---|
| root plugin `classpath` | AGP의 BC 1.79→1.86, jose4j 0.9.5→0.9.7, JDOM 2.0.6→2.0.6.1; Commons Compress→Commons Lang 3.16.0→3.18.0 |
| `:app`/`:androidApp` `androidLintTool` | Lint의 BC 1.79→1.86, Commons Lang 3.16.0→3.18.0, HttpClient 4.5.6→4.5.14 |
| application runtime | 기존 BC 1.86 등 직접 라이브러리 유지; Kotlin stdlib는 compiler와 함께 2.4.21로 선택 |

`security/dependency-exceptions.json`은 비어 있다. 향후 예외는 ecosystem·이름·버전·advisory와 모든 module/configuration/platform/build 또는 test 사용 범위를 정확히 지정해야 한다. 근거·적용 조건·검토일·재검토일·최대 90일 만료일이 필수다. runtime에 하나라도 사용되거나 범위가 늘어난 항목, 만료/재검토 기한 경과, 근거 누락은 실패한다. build/test 경고도 예외 없이 통과하지 않는다.

## 라이선스와 무결성

`licenses/dependencies.json`에 선택 runtime **182개 버전**의 POM/상위 POM, LICENSE·embedded NOTICE·출처·artifact 해시를 기록했다. `licenses/evidence/`는 SHA-256으로 식별한 원문이다. `licenses/native-components.json`은 Skiko/Skia 고정 revision·archive와 네이티브 전이 구성, JNA/libffi의 고지를 연결한다. 알 수 없는 라이선스·선택 artifact 미등록·자료 누락/변조는 검사 실패다. 라이선스 갱신 명령은 CI에서 자동 실행하지 않는다.

JediTerm 3.59는 POM의 LGPL 취급을 유지한다. source JAR 2개, LGPL/GPL, 실제 컴파일을 확인한 `licenses/jediterm-rebuild/` 재빌드·교체 절차를 함께 전달한다. 데스크톱 JAR은 분리되어 교체 가능하며 재빌드 recipe는 upstream과 byte-identical함을 주장하지 않는다. 패키징 JDK의 `legal/` 전체와 `release` 식별도 전달물에 복사하고 모든 파일의 SHA-256을 기록한다. 로컬 Homebrew JDK 고지 확인을 Windows Temurin 설치물 확인으로 대체하지 않는다.

공식 Gradle 배포 SHA-256 `84fbba45c7f4c64abc77460e1c00f541e9f960e3c7ed2538f1ede19eacd873ae`, Wrapper JAR SHA-256 `7a9ce74cff467ca1bf60a4fcd9f05185acceda4d0f382434d393e17864262c5d`를 고정했다. 출처: `https://services.gradle.org/distributions/gradle-9.7.0-{bin.zip,wrapper.jar}.sha256`. 신규 artifact는 Google Maven/Maven Central/JetBrains 저장소/plugin portal에서 다시 내려받아 생성된 해시와 대조했으며 `security/artifact-origins.json`에 실제 다운로드 URL을 기록했다. AndroidX variant 파일 이름과 실제 `.module` URL 차이도 반영한다. 이후 빌드에는 strict dependency verification을 적용한다. lock/verification 갱신 옵션은 일반 CI 명령에 넣지 않는다.

## Lint 검토

현재 Android lint는 오류 0·경고 11이다. SDK 37·라이브러리 새 버전 안내 9건은 사용자가 고정한 SDK 36/기존 앱 라이브러리 범위를 따르며 OSV와 별도로 검토한다. BC 1.86 `JcaJceUtils.getTrustAllTrustManager()` 관련 1건은 EST/TLS helper이며 앱은 이 API를 호출하지 않고 SSHJ의 명시적 host-key verifier를 사용한다. WebView 외부 네트워크도 차단한다. `ObsoleteSdkInt` 1건은 adaptive icon 폴더 한정자 안내다. 경고를 숨기지 않았다. Kotlin/Gradle의 DSL deprecation과 Android host test 미활성 안내는 취약점 예외가 아니며 commonTest는 JVM에서, 기기 동작은 androidApp instrumentation에서 검사한다. 정적 경고의 비적용 판단은 API/의존성 변경 시, **2026-11-22**에 재검토하고 **2026-12-22**에 만료한다. `security/lint-reviews.json`과 `scripts/verify_lint.py`는 진단 ID·메시지 해시·위치가 달라지거나 기한을 넘기면 CI를 실패시킨다. OSV 예외와 별개인 정적 진단의 비적용 검토이다.

## 릴리스 전 체크리스트

- [ ] Windows CURRENT_USER DPAPI round trip·다른 사용자 접근 거부·tamper·키 손실 검사
- [ ] Windows 설치/삭제, Android 실기, 실제 한국어/일본어 IME, 실제 에이전트 인수
- [x] 로컬 선택 의존성 OSV 0 및 라이선스/소스 고지 검사 구현
- [ ] Windows 실제 설치물의 JVM 고지·교체 가능성 최종 확인
- [ ] GitHub Secret scanning·Push protection 활성 상태 확인
- [ ] 서명된 배포 산출물에 키·개인 설정·터미널 기록이 없는지 검사

이번 로컬 작업에서는 원격 저장소 보안 설정과 Windows CI 실행 여부를 확인하지 않았다. 원격 설정을 변경하거나 코드·산출물을 게시하지 않았다. 노출이 발견되면 먼저 해당 자격 증명을 폐기·교체한다.


## 원격 CI 후속

원격 Linux·tmux·Android 6작업의 동일 커밋 결과와 다운로드 해시는 [VALIDATION.md](VALIDATION.md)에 기록한다.
Windows 실검증은 사용자 담당으로 남겼으며 수동 `verify_windows` workflow로 자동화도 선택 실행할 수 있다.
Windows 패키징은 실제 Temurin 버전에 대응하는 vendor 소스 archive와 공개 SHA-256 sidecar를 확보해야 한다.
일반 소스 줄바꿈과 원문/라이선스 바이트 보존을 분리했고, 전체 staged snapshot을 파일 경로를 유지한 채 검사한다.
GitHub Secret scanning·Push protection은 API 응답에 상태가 없어 확인 불가이며 설정을 변경하지 않았다.
