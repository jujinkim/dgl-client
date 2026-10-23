# dgl client

**Keep the CLI. Simplify the workflow.**

SSH와 표준 tmux를 사용하는 Windows·Android 터미널 클라이언트입니다. 원격 데몬 없이 기존 세션을 탐색하고, 연결을 닫은 뒤 같은 프로세스로 돌아갑니다. 현재 **개발용 MVP**이며 Windows 실기·서명 배포와 실제 Codex/Claude 인수 검증은 아직 완료되지 않았습니다.

## 현재 구현

- 호스트 추가·편집·삭제, 별칭·즐겨찾기·최근 접속, 독립된 여러 연결
- 비밀번호, RSA/Ed25519 및 암호화 개인키 가져오기, Ed25519 생성·공개키 복사
- 최초 호스트키 지문 확인, 저장된 키 변경 시 차단, 신뢰 키 삭제
- Android Keystore / Windows 사용자 DPAPI 보호 저장. 실패 시 저장 중단
- tmux Control Mode 세션·창·패널 조회·선택, 생성·이름 변경·분할·크기 변경·삭제 확인
- 외부 tmux 변경 이벤트 반영, 연결 복구 중 입력 차단, 입력 재전송 금지
- 화면 캡처·실시간 출력 순서화, 연결·패널·크기 세대 검사, bounded output / scrollback
- Android xterm.js/WebView, 데스크톱 JediTerm, 단축키 바, 앱 테마·폰트 크기
- tmux 미설치/버전 미지원 시 일반 SSH 셸. **일반 셸의 작업은 연결 종료 후 보존되지 않을 수 있습니다.**

## 플랫폼 범위

| 대상 | 설정 | 실제 확인 |
|---|---|---|
| Android 14 / 15 / 16 | `minSdk=34`, `compileSdk=36`, `targetSdk=36` | ARM64 에뮬레이터에서 APK·Keystore·SSH·합성 IME·키보드·단축키 바·회전·복귀 검증 |
| Windows 10/11 x64 | JVM 17, MSI/EXE 작업·CI | 로컬 Windows 실행·DPAPI 실기·설치 검증 **미실행** |
| macOS ARM64 | 개발용 JVM 실행 | 공통 코드·실제 SSH/tmux 통합 테스트. 자격 증명은 메모리에만 보관 |
| tmux | 3.3–3.7 범위만 Control Mode 사용 | **3.3a, 3.7c** 실제 서버 검증 |

앱 ID: `kim.jujin.rovessh`. 추가 플랫폼과 WSL 제품 연동은 후속 단계입니다.

## 빌드·실행

JDK 17, Android SDK(플랫폼 36, Build Tools 36.0.0), Node.js와 npm이 필요합니다. `JAVA_HOME`과 `ANDROID_HOME`을 각 개발 환경에 맞게 설정하세요. Gradle 9.7.0 Wrapper, Kotlin/Compose compiler 2.4.21, AGP 9.3.3을 사용합니다. `:app`은 공통 코드·Android KMP library·desktop, `:androidApp`은 Android 앱 진입점과 APK를 담당합니다.

```sh
cd web
npm ci --ignore-scripts
npm run bundle
cd ..
./gradlew :app:desktopTest :androidApp:assembleDebug :androidApp:assembleRelease :androidApp:lintDebug
./gradlew :app:run
```

APK: `androidApp/build/outputs/apk/debug/androidApp-debug.apk` (개발용 debug 서명).

Windows에서:

```powershell
.\gradlew.bat :app:run
.\gradlew.bat :app:desktopTest :app:packageMsi :app:packageExe
```

설치 파일: `app/build/compose/binaries/main/msi/`, `exe/`. Windows 패키징은 Windows에서 실행해야 합니다. 사용자 릴리스 서명 키는 저장소에 포함하지 않습니다.

## 연결 방법

1. 원격 서버에 SSH와 tmux를 설치하고 UTF-8 로케일을 구성합니다.
2. 앱에서 호스트·포트·사용자와 인증 정보를 저장합니다. 개인키는 파일에서 가져올 수 있습니다.
3. 처음 연결할 때 표시된 SHA-256 지문을 서버에서 독립적으로 확인하고 신뢰합니다.
4. 기존 세션을 선택하거나 새 세션을 만듭니다. 창·패널을 GUI로 관리합니다.
5. `연결 종료`는 SSH와 해당 tmux 클라이언트만 닫습니다. `삭제`는 이름을 확인한 뒤 원격 대상을 종료합니다.

서버에서 `tmux new-session`, `tmux attach`, `tmux list-sessions`를 그대로 사용할 수 있습니다. 앱은 tmux 서버의 전역 설정·사용자 셸·키 바인딩을 변경하지 않습니다.

## 검증

```sh
./gradlew :app:desktopTest
python3 scripts/integration.py
python3 scripts/integration.py --tmux /path/to/tmux-3.3a
python3 scripts/build_test_tmux.py 3.3a
python3 scripts/integration.py --tmux .tools/tmux-3.3a/tmux-3.3a/tmux
./gradlew :androidApp:connectedDebugAndroidTest
python3 scripts/integration.py --command python3 scripts/android_integration.py
python3 scripts/check_prompt.py
python3 scripts/verify_wrapper.py
python3 scripts/scan_secrets.py
./gradlew writeDependencyInventory
python3 -m unittest discover -s scripts/tests -v
python3 scripts/audit_dependencies.py
python3 scripts/verify_licenses.py
python3 scripts/verify_lint.py
./gradlew :app:prepareThirdPartyNotices
python3 scripts/collect_artifacts.py
```

통합 테스트는 로컬 OpenSSH와 임시 키·독립 tmux 소켓을 사용합니다. 기존 `authorized_keys`, 사용자 tmux 서버와 호스트 설정을 사용하지 않습니다. Android 통합 실행에는 실행 중인 에뮬레이터가 필요합니다(`ANDROID_SERIAL` 기본값 `emulator-5554`). 기본 JVM 테스트는 외부 SSH 환경이 없으면 해당 테스트를 **skip**합니다.

테스트용 tmux 빌드는 Python 3.9 이상, C 컴파일러, make, libevent·ncurses 개발 헤더가 필요합니다. `.tools/`에만 빌드하며 사용자 tmux를 덮어쓰지 않습니다. CI에는 같은 두 tmux 버전의 통합 테스트와 Windows 패키징 작업을 정의했습니다. 원격 CI 실행 결과는 아직 없습니다.

`collect_artifacts.py`는 APK의 앱 ID·SDK·번들 파일·민감 파일 이름을 확인하고 `artifacts/`에 APK, 라이선스/source 묶음, SHA-256을 모읍니다. Windows 설치물이 있을 때만 함께 수집합니다.

검증 결과·실기 체크리스트는 [docs/VALIDATION.md](docs/VALIDATION.md), 단계별 상태는 [TASKS.md](TASKS.md)에 기록합니다.

## 남은 한계

- Windows 실제 설치·한글 IME·DPAPI, 물리 Android 기기·실제 한글/일본어 키보드, 실제 Wi-Fi↔셀룰러 전환은 별도 검증이 필요합니다.
- 실제 Codex·Claude의 로그인·권한 요청·장시간 실행 중 재접속은 미검증입니다. 이 단계 전에는 에이전트 호환성을 확정하지 않습니다.
- 스냅샷은 화면·alternate screen·커서·주요 모드를 복원합니다. tmux가 제공하지 않는 현재 SGR pen, 모든 VT 상태와 이전 로컬 scrollback의 완전 복원은 보장하지 않습니다. 필요하면 터미널에서 다시 그리기(Ctrl+L)를 사용합니다.
- 패널 전환·재접속 시 로컬 scrollback은 초기화됩니다. tmux Copy Mode는 별도 상태로 표시하며 해당 화면을 일반 pane 출력으로 오인하지 않습니다.
- 자동 재접속은 최대 8회 시도합니다. 네트워크 오류만 재시도하며 인증·호스트키·프로토콜 오류는 멈춥니다. Android의 지속적인 백그라운드 연결은 보장하지 않습니다.
- 원격 pane 크기는 최대 500×250셀입니다. JCEF 비교는 설계 비교까지만 수행했으며 실행 비교와 최종 Windows 렌더러 판정은 남아 있습니다.
- 2026-10-23 선택 의존성 371개 버전의 OSV 런타임·빌드·테스트 경고는 모두 0건입니다. 공개 릴리스 전 [보안 검토](docs/SECURITY.md)를 완료해야 합니다.

원문 요구사항은 [KICKOFF_PROMPT.md](KICKOFF_PROMPT.md)와 SHA-256으로 보존합니다. 기술 결정: [ADR 001](docs/decisions/001-architecture.md), [ADR 002](docs/decisions/002-terminal-restoration.md). 라이선스: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
