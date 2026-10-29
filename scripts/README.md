# 스크립트 안내

저장소 루트에서 실행합니다. 일반 Python 도구는 3.9 이상을 사용하며 필수 SDK·JDK·Rust 설정은
[개발 안내](../docs/DEVELOPMENT.md), Windows 도구는 [Windows 안내](../docs/WINDOWS.md)를 따릅니다.
기본 검사에 에뮬레이터·장시간 성능 측정·실제 CLI 호출을 자동으로 추가하지 않습니다.

## 빌드·패키징

| 도구 | 역할·출력 |
|---|---|
| `build_rust_android.py` | Gradle에서 호출하는 Rust 네 ABI 빌드·UniFFI 생성 |
| `collect_rust_android.py` | 원본 Debug APK의 ID·SDK·네 ABI·Rust payload·서명 확인, `artifacts/rust-candidate/android/build.json` 기록 |
| `verify_app_versions.py` | Cargo 버전과 선택 플랫폼의 패키지 비교. 기본/`--android`는 Kotlin·Debug APK, `--mac`은 `.app`, `--windows`는 ZIP 내부·외부 메타데이터 검사; 플래그 조합 가능 |
| `collect_artifacts.py` | 원본 Android Debug/Release APK 정책·assets·라이선스 검사, `artifacts/` manifest·해시 생성 |
| `package_rust_mac.py` | GPUI Debug/`--release` 빌드, `.app`·Rust 고지·ad-hoc 서명 |
| `run_windows.cmd` / `windows_menu.ps1` | Windows 빌드·실행·검사 메뉴 |
| `windows_tools.ps1` | Windows 필수 도구 전체 점검·선택적 설치/업데이트 |
| `build_windows.ps1` | Windows MSVC 환경 구성·빌드. `-CheckOnly`, `-Test`, `-Run` 지원 |
| `package_rust_windows.py` | x64 PE 검사·고지·해시를 포함한 미서명 ZIP 생성 |
| `collect_rust_notices.py` | 배포 대상(Android 네 ABI·macOS host·Windows MSVC)으로 걸러낸 Cargo registry 고지·manifest·의존성 목록 수집, 패키저와 `:coreBindings`에서 사용 |
| `generate_app_icons.py` | 공통 도형에서 투명 원본·Android 전경/단색/알림 벡터·모바일 Compose 벡터·macOS ICNS·Windows ICO·미리보기 생성. `--check`로 저장 파일 비교; [아이콘 안내](../assets/app-icon/README.md) |

`collect_artifacts.py`와 `collect_rust_android.py`는 각각 전체 배포 정책 검사와 Debug 서명·Rust payload 검사를 담당합니다.
`collect_artifacts.py`는 APK의 `assets/third-party/`가 참조·테스트 전용 라이브러리(JediTerm·SSHJ·Bouncy Castle·SLF4J) 원문과 JediTerm 재빌드 자료를 뺀 `licenses/` 선택과 정확히 같은지 확인합니다.
둘 다 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 원본을 사용하고 APK 복사본을 만들지 않습니다.
이전 JVM 엔진이 제품 APK에 들어가면 실패합니다.
현재 Windows 배포 경로는 Rust ZIP입니다. 폐기된 JVM MSI/EXE 도구는 [이력](../docs/HISTORY.md)에서 찾을 수 있습니다.

## 기본 검사·유지보수

| 도구 | 용도 |
|---|---|
| `generate_ui_icons.py` | 공통 SVG에서 Compose 벡터·Rust 포함 목록 생성. `--check`로 SVG 형식·생성물 일치 검사; [UI 아이콘 안내](../assets/ui-icons/README.md) |
| `check_prompt.py` / `verify_wrapper.py` | 원문 요구사항·Gradle bootstrap 무결성 |
| `verify_test_results.py` | JVM의 모든 지정 테스트 결과·예상 skip 검사 (`linux`, JVM 참조 SSH용 `ssh`) |
| `verify_android_results.py` | 계측의 지정 테스트 이름·완료·실패/skip 검사; 실행 도구에서 import |
| `verify_lint.py` | lint 진단·검토 조건·만료 확인 |
| `scan_secrets.py` | Gitleaks로 작업 파일·전체 staged snapshot·Git refs 검사 |
| `audit_dependencies.py` | Gradle 선택 inventory·Cargo.lock(crates.io)·npm lock의 OSV 검사 |
| `verify_licenses.py` | 선택 inventory의 라이선스·원문 해시·고지 확인 |
| `refresh_licenses.py` | 명시적으로 요청할 때 라이선스 근거 갱신; 생성 결과 검토 필요 |
| `verify_dependency_origins.py` | 의존성 출처와 다운로드 해시 대조 |
| `evidence.py` | 검사 문맥·안전 경로·해시·보고서 공통 함수 |

```sh
python3 -m unittest discover -s scripts/tests -v
```

JVM·Android 테스트를 추가/삭제하면 `verify_test_results.py` / `verify_android_results.py`의 명시적 목록도 갱신합니다.
`test_evidence.py`가 소스 목록의 누락과 이름·결과 위조를 검사합니다. 결과를 맞추려고 실패·skip을 허용하지 않습니다.

## 선택적 통합 검사

| 도구 | 범위 |
|---|---|
| `build_test_tmux.py` | 3.3a/3.7c를 `.tools/`에 빌드; 사용자 tmux는 변경하지 않음 |
| `integration.py` | 임시 OpenSSH·키·독립 tmux socket 생성/정리. 기본은 Rust `ssh_integration` 전체(`--agent-home` 필요), `--command`로 개별 Rust·JVM 참조·Android 지정 |
| `android_integration.py` | fixture 안에서 APK 빌드·설치·전체 기본 runner 실행·이름별 결과 검증 |
| `rust_android_integration.py` | 빌드된 APK의 API36 후보 검사·화면 수집. 명시적 이름 목록과 대조하며 fixture 안에서 실행 |
| `workspace_android.py` | 전용 API36 한 대에서 839/840dp·분할·WebView 동일성·IME·활성 입력/모달 차단·드래그·세션 폰트 핀치/초기화·가로 스크롤 경계와 기존 모바일 회귀 검사. 화면/키보드 설정 원복, `artifacts/android-workspace/` 출력 |
| `mobile_input_android.py` | 전용 API36의 모바일 탭·알림·입력·About·WebView 검사·화면 수집 |
| `android_device.py` | 위 Android 계측 도구가 공유하는 adb 경로·APK 설치 목록·fixture 전달·설정 원복·화면 수집 함수 |
| `verify_android_matrix.py` | API별 전용 AVD 실행·결과 수집. 명시적으로 전체 범위를 요청할 때만 실행 |
| `verify_agent_cli.py` | 격리된 환경에서 실제 Claude/Codex 훅 검사; 설치·인증·호출 범위 확인 필요 |

`rust_android_integration.py`의 `DGL_ANDROID_TEST_CLASSES`는 `Class` 또는 `Class#method`를 쉼표로 받습니다.
기대 결과는 공통 이름 목록에서 선택하며 알 수 없는 선택·누락·중복·실패·skip은 거절합니다.
`DGL_ANDROID_EXPECTED_TESTS` 숫자 override는 사용하지 않습니다.
`kim.jujin.dglc.AndroidConnectionLifetimeTest`를 선택하면 실제 Rust SSH 연결 하나로
IME·Activity 재생성·짧은 백그라운드 복귀·알림 인텐트·task 제거와 원격 프로세스 유지만 검사합니다.
`kim.jujin.dglc.AndroidOutputRecoveryTest`는 15초 ACK timeout·화면 구독 교체·모든 표시 pane의 ACK 대기,
수동 복구와 SSH 세대·tmux client·원격 프로세스 유지를 검사합니다. 두 클래스는 API36 선택 검사로 실행합니다.

## 선택적 성능·자원 도구

`migration_` 접두사와 `fixtures/migration/` 경로는 재현·테스트 참조 때문에 유지합니다.
현재 앱 실행에 필요한 도구는 아니며, 자동으로 성능 인수를 시작하지 않습니다.

| 도구 | 역할 |
|---|---|
| `generate_locales.py` | 한국어·영어 공통 UI 목록 검증과 Kotlin 생성; `--check`로 누락·자리표시자·생성 파일 일치 검사 |
| `migration_performance.py` | Mac CPU/RSS·패키지 크기 수집, 성능 계약 판정. Android 상대 비교는 기준 앱 제거로 판정하지 않음 |
| `migration_android_presentation.py` | API36 Perfetto 표시 trace 수집·상한/첫 표시 분석 |
| `migration_first_presentation.py` | 첫 표시 증명·누락 근거 오프라인 판정 |
| `verify_resources.py` | Android API34–36 격리 자원 부하 실행·보고서 수집. SSHJ 참조 엔진으로 WebView 터미널을 구동 |
| `resource_workload.py` / `resource_results.py` | 부하 시나리오·표본 판정 공통 코드 |

[성능 계약](../docs/PERFORMANCE.md)에 측정 정의·표본·한계가 있습니다.
`collected`, smoke 통과, xterm 콜백 시각을 전체 성능·표시 지연 합격으로 기록하지 않습니다.
