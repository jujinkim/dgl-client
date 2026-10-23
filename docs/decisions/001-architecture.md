# ADR 001 — JVM 기반 KMP/Compose MVP

상태: 구현. Windows 렌더러의 실기 채택 판정은 보류.

## 결정

`commonMain`에는 JVM 타입 없는 모델, 네 경계(`SshTransport`, `TmuxController`, `TerminalSurface`, `CredentialStore`), 바이트 파서와 명령 인코더를 둔다. Android/JVM이 함께 쓰는 `jvmSharedMain`에는 SSHJ, 연결 수명 관리, tmux 제어, 로컬 저장과 Compose UI를 둔다. Android와 데스크톱은 보안 저장·터미널·파일 선택만 구현한다. iOS 확장 시 `jvmSharedMain`을 공통으로 이식해야 하므로 현재 iOS 지원을 주장하지 않는다.

tmux가 원격 상태의 기준이다. 디스크에는 호스트·신뢰 지문·자격 증명 참조·설정·마지막 탐색 위치만 저장한다. 원격 세션·프로세스를 로컬 데이터로 생성하거나 복구한 것으로 간주하지 않는다. 연결마다 독립 transport/controller/generation을 가진다.

## 대안

| 선택 | 평가 | 결정 |
|---|---|---|
| KMP + Compose + SSHJ | Android와 Windows JVM에서 프로토콜·UI 공유, 기존 SSH 키 지원 | 채택 |
| 완전한 common Kotlin SSH | 선택 가능한 라이브러리·키 형식 검증 부담 증가 | 사용하지 않음 |
| 플랫폼별 앱 두 개 | 입력 통합은 직접적이나 복구·프로토콜 중복 | 사용하지 않음 |
| JediTerm | JVM 내 Swing 임베딩, 별도 브라우저 런타임 없음, VT/Unicode 테스트 가능 | 데스크톱 PoC 채택, Windows IME 보류 |
| xterm.js + JCEF | Android와 같은 렌더러, Chromium 수명·배포·메모리 관리 추가 | 실행 비교 미실행; 필수 JediTerm 문제가 확인되면 대체 검증 |
| Android 시스템 WebView + xterm.js | OS 관리 WebView, 앱에 고정 assets 번들, 실제 IME 경로 제공 | 채택, 물리 키보드 확인은 남음 |

입력 지연·메모리·크기를 측정하지 않은 후보가 더 빠르다고 주장하지 않는다. 현재 측정값은 `artifacts/performance.json`의 loopback 환경에 한정한다.

## 고정 버전

- Gradle 9.7.0 / AGP 9.3.3 / Kotlin 및 Compose compiler 2.4.21 / Compose 1.9.3
- JDK 17 / Android API 34–36
- SSHJ 0.40.0 / Bouncy Castle 1.86 / JediTerm 3.59
- xterm.js 6.0.0 / addon-fit 0.11.0 / JNA 5.18.1

JediTerm 3.76을 처음 해석할 때 Kotlin 2.4 메타데이터 불일치가 발생했다. 컴파일러 검사 우회 대신 3.59를 고정하고 필수 모델 테스트를 실행했다. SSHJ의 낮은 BC 전이 버전은 1.86으로 올렸다. Android의 축소된 기본 `BC` provider와 이름이 충돌하는 실제 인증 실패를 발견하여 앱 프로세스 안에서 번들 BC를 명시적으로 등록한다. AndroidKeyStore provider는 유지한다.

Kotlin/AGP 업데이트는 `docs/SECURITY.md`의 빌드 도구 경고와 함께 후속 평가한다. Gradle cache와 Kotlin incremental cache를 비활성화했고 CI에서 외부 캐시를 복원하지 않는다.

## 근거

- [Compose 호환성](https://kotlinlang.org/docs/multiplatform/compose-compatibility-and-versioning.html)
- [AGP 8.11의 API 36·Gradle 8.13·JDK 17 기준](https://developer.android.com/build/releases/agp-8-11-0-release-notes)
- [SSHJ 기능·Terrapin 수정 버전](https://github.com/hierynomus/sshj)
- [JediTerm 소스와 라이선스](https://github.com/JetBrains/jediterm)
- [xterm 보안 지침](https://xtermjs.org/docs/guides/security/)

## P4-03 Android 모듈 분리 (2026-10-23)

AGP 9에서는 `:app`의 공통·JVM·Android 플랫폼 구현을
`com.android.kotlin.multiplatform.library`로 유지하고, Android 앱 진입점과
ViewModel·Manifest·아이콘·backup 정책·계측 테스트를 `:androidApp`으로 분리했다.
새 앱은 AGP 내장 Kotlin과 `:app` AAR을 사용한다. 터미널 assets는 AAR에서
APK로 전달된다. namespace만 shared library용으로 분리했으며 applicationId,
SDK 34/36, 저장 형식과 Keystore 별칭은 바꾸지 않았다.

공식 이전 근거: https://kotlinlang.org/docs/multiplatform/multiplatform-project-agp-9-migration.html
현재 검증과 보안 조건은 `docs/VALIDATION.md`, `docs/SECURITY.md`를 따른다.
위 AGP 8.11 링크는 초기 결정의 역사적 근거이다.
