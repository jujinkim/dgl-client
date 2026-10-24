# dgl client 작업 목록

상태: 예정 / 진행 중 / 구현 완료·검증 대기 / 검증 완료 / 차단 / 후속 단계.
미실행 검증은 통과로 취급하지 않는다. 원격 상태는 tmux가 기준이며 정상 종료는 연결만 닫는다.

이어서 진행할 작업: [NEXT_WORK.md](NEXT_WORK.md). 2026-10-24 후속으로 API36 표시 계측 훅과
Perfetto frame 연결 수집기를 추가했다. 짧은 훅 검증과 정식 성능 인수는 구분한다.

2026-10-24 공통 코어·모바일 UI·데스크톱 UI 분리 전환은 [별도 작업 계획](docs/ARCHITECTURE_MIGRATION.md)의 R0–R7로 추적한다. **R1 진행 중**: 공통 fixture 120건과 JVM 소비 테스트 6개, 기능 대조표·API 계약, 고정 성능 계약·수집/판정 도구를 추가했다. 사용자 후속 지시로 이번 검증은 현재 Mac(macOS 26.6.2)과 API36 AVD만 수행한다. [R1 결과·남은 작업](docs/migration/R1_BASELINE.md)을 따른다. R1a 계약 120건은 Rust에서도 통과했고 R2–R5 후보를 구현했다. 성능은 R1b 후속으로 분리했다. Mac GUI·Keychain과 전체 기능 화면 인수가 남아 후보 완료·최종 채택은 미확정이다. 후속 지시로 Android는 Rust 단일 엔진으로 전환하고 이전 JVM 앱 실행·패키징은 제거한다. 아래 표는 기존 KMP/Compose 구현 당시의 기록이며 현재 실행·배포 설정은 README를 따른다.

| ID | 단계 / 작업 | 선행 | 완료 조건 | 상태 | 검증 결과 / 남은 문제 |
|---|---|---|---|---|---|
| P0-01 | 원문 보존·작업 문서 | 없음 | 원문 SHA-256 기록 및 변경 없음 | 검증 완료 | `ab825dd3b3021319df8f963edfd9aa49250b580a86e786c4832a0f4025e57ea8` |
| P0-02 | 공개 저장소 제외·비밀 검사 | P0-01 | 작업 트리·스테이징·전체 이력·CI 검사 | 검증 완료 | Gitleaks 8.30.1 로컬 3범위 통과; CI 동일 검사 정의, 원격 실행 별도 |
| P0-03 | KMP/Compose·Wrapper·실행 구조 | P0-02 | JVM 테스트 및 Android 빌드, 데스크톱 실행·패키징 구조 | 검증 완료 | 초기 Gradle 8.13 검증 후 P4-03에서 9.7.0/AGP 9.3.3/Kotlin 2.4.21 이관; Windows 실행은 P4-01 |
| P0-04 | 격리 SSH/tmux·Android 환경 | P0-03 | 임시 자격 증명, tmux 3.3a/3.7c, API 34/35/36 | 검증 완료 | 실제 OpenSSH + private socket; 새 Dgl_API34/35/36 AVD; 사용자 환경 미사용 |
| P1-01 | 바이트 증분 Control Mode 파서 | P0-03 | 경계·escape·응답·이벤트·오류·상한 테스트 | 검증 완료 | 프로토콜 9개, PTY 없는 exec, live/capture escape 분리 |
| P1-02 | SSH→tmux→입출력·복구 | P1-01,P0-04 | 실제 SSH 조회·선택·입력·종료·재접속 | 검증 완료 | 두 tmux 버전 JVM 및 Android 3버전 실제 SSH, 같은 PID 유지 |
| P1-03 | 스냅샷·출력 순서·세대 | P1-02 | pause/capture/resume, 오래된 결과 폐기 | 검증 완료 | 10회 연속 snapshot/live tail 중복·누락 검사; resize·pane·연결 세대. 전체 VT pen 복원 한계 ADR 002 |
| P1-04 | 렌더러 비교·결정 | P1-03 | Unicode·TUI·resize·IME·성능 비교 기록 | 구현 완료·검증 대기 | JediTerm 3.59/xterm 6.0 PoC 통과; Windows IME·실제 TUI 및 JCEF 실행 비교 미완료, 채택 잠정 |
| P2-01 | 호스트 관리·독립 연결 | P1-02 | 추가·편집·삭제·즐겨찾기·최근 연결 | 구현 완료·검증 대기 | 공유 GUI·설정 저장·독립 모델 구현; 동시 SSH 확인, 전체 호스트 GUI 인수 필요 |
| P2-02 | 인증·키·보호 저장소 | P0-03 | 비밀번호·키·보호 실패 시 차단 | 구현 완료·검증 대기 | 비밀번호/RSA/Ed25519/암호화 키/키 생성 및 Android Keystore 통과; Windows DPAPI test skip |
| P2-03 | 호스트키·재접속·fallback | P2-02,P1-03 | TOFU·변경 차단·백오프·입력 차단·일반 SSH | 검증 완료 | 변경 지문·오인증 차단, 3회 강제 단절, 입력 미재전송, 세션 소멸, SSH PTY 통과 |
| P3-01 | GUI 탐색·tmux 관리 | P2-01,P2-03 | 생성·이름·분할·선택·크기·삭제 확인·외부 변경 | 구현 완료·검증 대기 | 공유 GUI·이벤트 재조회·삭제 대상 고정; Android GUI 분할·선택·창 생성 검증, 전체 Windows GUI 인수 필요 |
| P3-02 | Android 입력·화면·설정 | P3-01,P1-04 | IME·단축키·회전·복귀·테마·폰트·선택 | 구현 완료·검증 대기 | API 34/35/36 각 4개 통과, 폰트 변경·renderer 재연결 포함; 실제 키보드 조합·기기 선택/scroll 별도 |
| P3-03 | 출력 흐름·자원 상한 | P1-03,P1-04 | 큐·scrollback 상한, 재접속 자원 회수 | 구현 완료·검증 대기 | 256 KiB desktop pipe, Android 16 KiB ack, parser/입력 상한. 별도 자원 수집·판정 도구 및 중단 정리 구현; 5환경 본 측정·판정 대기 |
| P4-01 | APK·Windows 패키징·CI | P3-01,P3-02 | APK, Windows 설치물, 최소 권한 CI | 구현 완료·검증 대기 | debug APK 생성·에뮬레이터 설치; MSI/EXE 작업과 CI 작성, 실제 Windows 빌드 미실행 |
| P4-02 | 인수 A–F·실제 에이전트 | P4-01 | 실제 수행 결과와 미실행 체크리스트 | 구현 완료·검증 대기 | 부분 자동 검증 연결, docs/VALIDATION.md에 남은 항목; 실제 Codex·Claude·실기 미실행 |
| P4-03 | 의존성·라이선스·무결성·보안 | P4-01 | 잠금·검증·취약점·비밀·라이선스 검사 | 진행 중 | 로컬: 모듈 분리·Kotlin 2.4.21/AGP 9.3.3/Gradle 9.7.0, 371버전 OSV 전 범위 0, 라이선스 182버전·네이티브/JVM 고지·JediTerm 재빌드·보안 12테스트 통과. API 34/35/36 각 4통과·APK/477개 고지 해시 확인. 원격 Linux·tmux·Android 6작업 동일 커밋 통과; Windows 사용자 검증 대기·서명/게시 별도 |
| P4-04 | 전달 문서·실행 산출물 | P4-02,P4-03 | 재현 명령·지원 범위·제한·산출물 | 검증 완료 | README, ADR 2개, 보안/검증 문서, artifacts/DELIVERY.md; 미실행을 통과로 처리하지 않음 |
| F-01 | 프로젝트·워크스페이스 메타데이터 | 없음 | 후속 요청 시 설계 | 후속 단계 | MVP 제외 |
| F-02 | Textbox·사용자 단축키·WSL·추가 플랫폼 | 없음 | 후속 요청 시 구현 | 후속 단계 | MVP 제외 |

## 검증 기록

- 시작: 저장소에 README와 원문 프롬프트만 있으며 애플리케이션 코드는 없다.
- 개발 호스트: macOS ARM64. Windows 및 Android 실기 결과를 추정하여 통과시키지 않는다.
- 2026-10-23: 실제 OpenSSH+tmux 3.3a와 3.7c 각각 JVM 테스트 19 통과, Windows DPAPI 1 skip.
- 2026-10-23: Android API 34/35/36 각각 4개 instrumentation 통과. 보호 저장소·브리지·합성 IME·실제 SSH·키보드·회전·복귀 포함.
- Android crypto provider 충돌, Control Mode capture backslash, tmux format/semicolon 이름 처리, snapshot/resize 경합, WebView 초기 높이·native view 수명주기 결함을 재현 후 수정했다.
- 로컬 로그는 `.tools/`, 테스트 리포트는 `app/build/`, 산출물은 `artifacts/`에 두며 공개 저장소에 포함하지 않는다.
- 검증 세부 사항과 남은 실기 체크리스트: `docs/VALIDATION.md`. 공개 배포 판정은 아직 완료하지 않았다.

## P4-03 로컬 최종 기록 (2026-10-23)

- `:androidApp` debug·unsigned release·계측 APK, lint 및 `:app` JVM 빌드 통과.
- 격리 tmux 3.3a/3.7c 각각 JVM 19 통과·Windows DPAPI 1 skip.
- 전용 API 34/35/36 AVD 각각 4개 계측 테스트 통과. System UI ANR은 소프트웨어 렌더러로 정비했고, API 35 초기 GUI timeout 이후 개별 및 최종 전체 실행에서 통과했다.
- 선택 그래프 54개/371버전 OSV runtime/build/test 0, 취약점 예외 없음. Lint 11건은 정확한 진단·조건·재검토일·만료일로 관리한다.
- runtime 라이선스 182버전, native 고지, JediTerm 3.59 소스 2개 재빌드, 실제 JDK legal 자료를 포함한 desktop 고지 477개 해시 확인.
- Wrapper/원문 SHA·Gitleaks 3범위·npm integrity/audit·보안 회귀 12개 통과. 신규 artifact 출처 대조 기록은 `security/artifact-origins.json`.
- CI에 새 모듈 명령·OSV/라이선스/lint 실패 기준과 API 34/35/36 계측 job을 정의했다. 원격 실행은 수행하지 않았다.
- `artifacts/build-manifest.json`, `artifacts/SHA256SUMS`, `artifacts/DELIVERY.md`를 갱신했다. P4-03 전체와 공개 릴리스는 검증 완료로 표시하지 않는다.

## 인수 시나리오 연결

| 시나리오 | 작업 | 판정 |
|---|---|---|
| A Windows→Android 동일 프로세스 | P1-02,P2-03,P4-01,P4-02 | JVM/Android 개별 PID 보존 통과; 실제 Windows→Android Codex 미실행 |
| B 모바일 네트워크 전환 | P2-03,P3-02,P4-02 | 강제 단절·복귀 통과; 실제 Wi-Fi/셀룰러 미실행 |
| C 표준 SSH/tmux 상호 운용 | P1-02,P3-01 | 실제 표준 명령·Control Mode 상호 조작 통과; 기기 인수 남음 |
| D GUI 세션·창·pane 관리 | P3-01 | 구현·명령 통합 검사; 전체 플랫폼 GUI 인수 남음 |
| E 종료·충돌 후 프로세스 유지 | P1-02,P2-03 | 종료·강제 단절·반복 복구 PID 통과; OS kill 실기 남음 |
| F Codex·Claude 터미널 호환 | P1-04,P3-02,P4-02 | 합성 렌더러·IME 통과; 실제 에이전트 미실행 |


## P4-03 원격 CI 후속 범위 조정 (2026-10-23)

- 사용자 요청: Windows 빌드/실검증은 사용자가 직접 수행. 전용 브랜치 push와 Linux·tmux·Android CI는 계속한다.
- 당시 일반 push의 필수 원격 실행 단위는 6개(기본 1, tmux 2, Android 3)였다. 2026-10-24부터 자동 실행은 기본 검증 1개로 축소하고 SSH·Android 통합 검사와 배포 검증은 수동 입력으로 선택한다. Windows도 기존처럼 수동 선택한다.
- 로컬 회귀 20개, 원문/라이선스 바이트 보존의 두 autocrlf 모드, JVM 기본 13 통과·예상 skip 7 확인.
- Windows DPAPI 두 사용자·설치/제거·Temurin 소스·JediTerm 교체 자동화와 수집 형식은 구현했으나 Windows에서 미실행이다.
- 원격 결과는 실제 실행 이후 기록하며 P4-03 전체를 검증 완료로 올리지 않는다.


### 원격 확인 결과

- [https://github.com/jujinkim/dgl-client/actions/runs/39898373064](https://github.com/jujinkim/dgl-client/actions/runs/39898373064), `899ad82b540a5b284c59630b0713a8950b09de4a`: 필수 6작업 성공. Windows는 사용자 요청으로 미실행.
- Linux JVM 13/0/7, tmux 각 19/0/1, Android API 34/35/36 각 4/0/0 (통과/실패/예상 skip).
- Actions ZIP 7개 digest 및 APK SHA-256 `ecdb1b5b68894efe738641278c23dfa7a6e4f5b708e092ad149b0e6808df4ea6`를 다운로드 후 대조했다.
- OSV 371버전 경고 0·npm audit 0·Gitleaks 3범위·lint·APK 정책 통과. Linux 고지 485파일.
- GitHub Secret scanning·Push protection 상태는 조회 응답 미제공으로 확인 불가. 설정 미변경.
- 최종 수집 보완 커밋의 실행/해시 기록: `artifacts/REMOTE_CI.md`. Windows·실기·실제 에이전트·서명은 여전히 후속이며 P4-03 전체는 진행 중.

## P3-03 시작 전 main 통합 (2026-10-23)

- `codex/p4-03-ci-windows`의 3개 커밋을 로컬·원격 `main`에 fast-forward 통합했다. 원격 SHA는 `3baeb566843bf9db6d27cb8bf1c6d6c92b89b94d`다.
- main CI: https://github.com/jujinkim/dgl-client/actions/runs/39904361260 . 최초 API 35 GUI pane split 접근성 시간 초과 1건, 해당 작업 재실행 후 필수 6개 성공. 최초 실패를 안정성 통과 근거에서 제외한다. Windows는 미실행이다.
- 후속 구현은 `codex/p3-03-resource-soak`에서 수행한다. smoke는 장기 메모리 합격을 의미하지 않으며 P3-03 전체 상태를 올리지 않는다.

## Mac·Android 후보 전환 — 2026-10-24 우선순위 변경

성능 계측을 기능 구현의 선행 게이트에서 분리하고 기존 MVP 기능·기본 디자인을 먼저 이식한다.
Rust 코어/UniFFI/GPUI와 Android 후보 연결·기본 화면을 구현했고 tmux 3.3a/3.7c 및 API36 검증을 진행했다. Android는 Rust만 빌드·실행하며 별도 테스트 저장 영역을 쓴다. 이전 엔진은 테스트 소스에만 보존한다. 실제 진행 상태와 검증 명령은
[후보 구현 기록](docs/migration/CANDIDATE.md), [다음 작업](NEXT_WORK.md)을 따른다.
