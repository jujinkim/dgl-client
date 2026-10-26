# 작업 지침

## 프로젝트 기준

- DGL은 SSH 호스트·세션 관리, 네트워크 단절 후 재접속, tmux 세션·창·pane 조회와 조작을 제공하는 GUI 터미널이다.
- 랜딩 URL: `https://ddal.ggak.life`. 슬로건: **Reconnect less. Resume more.**
- 짧은 설명: “Let DGL handle SSH reconnection and tmux reattachment, so you can pick up where you left off.”
- 제품 소개는 README, 현재 사용·개발 문서는 `docs/README.md`, 남은 작업은 `TASKS.md`를 기준으로 한다.
  과거 계획·검증 이력은 `docs/HISTORY.md` 하나로 요약하고 상세는 Git 이력에서 찾는다. 완료 기록을 여러 문서에 누적하지 않는다.

## 코드·스크립트 유지

- 공통 코어는 `crates/dgl-core`, 바인딩은 `crates/dgl-uniffi`/`coreBindings`, Android는 `mobileUi`/`app`/`androidApp`, 데스크톱은 `desktop`이다.
- Kotlin/Java 제품·참조·검사 패키지와 디렉터리 루트는 `kim.jujin.dglc`로 통일한다. 기존 보호 저장 식별자는 패키지 이름과 별개이며 자격 증명 호환성을 유지한다.
- Android·데스크톱 제품은 Rust 단일 엔진이다. `app/src/referenceTest`와 JVM 렌더러는 회귀·fixture용이며 제품 실행 경로로 되살리지 않는다.
- `web/`는 Android 터미널 assets 소스다. 랜딩 웹사이트 소스로 해석하지 않는다.
- tmux가 원격 상태의 기준이다. 연결 종료는 원격 작업 삭제가 아니며 복구 중 입력 차단·실패 입력 무재전송·대상 세대 검사를 유지한다.
- `scripts/README.md`와 `docs/DEVELOPMENT.md`의 실제 진입점을 유지한다. 삭제·이동 시 코드·CI·문서 참조를 함께 정리한다.
- JVM/Android 테스트 이름 변경 시 결과 검증기의 명시적 목록도 갱신한다. 미실행·skip·ignore를 통과로 기록하지 않는다.
- `KICKOFF_PROMPT.md`/체크섬과 제3자 라이선스 원문·해시 증거는 보존한다. 현재 기능·범위에는 후속 사용자 지시를 우선한다.

## 브랜치 운영

- 2026-10-23 사용자 요청에 따라 당분간 `main` 브랜치만 운영한다.
- 사용자가 브랜치 운영 방식을 변경하기 전까지 구현·수정·커밋은 `main`에서 진행하고, 새 작업 브랜치를 만들지 않는다.
- 과거 작업 기록에 있는 전용 브랜치 안내보다 이 지침을 우선한다.

## Windows 빌드 환경 유지

- Windows 실기 검증을 이번 작업에서 제외하더라도 Windows 빌드·패키징 환경은 유지한다. 검증 범위 제외를 플랫폼 구현·빌드 지원 제외로 해석하지 않는다.
- Rust 데스크톱을 수정할 때 macOS 전용 의존성·경로·API를 공통 코드에 무조건 적용하지 않고 Windows MSVC x64 빌드 경로를 함께 고려한다.
- Windows 빌드 진입점은 `scripts/build_windows.ps1`, 패키징은 `scripts/package_rust_windows.py`, 수동 CI는 `.github/workflows/windows-build.yml`이다. 절차·필수 도구·출력 경로가 바뀌면 `docs/WINDOWS.md`와 README도 함께 갱신한다.
- 기본 Windows 스크립트와 수동 CI는 빌드·ZIP 생성만 수행한다. 테스트는 `-Test`로 별도 선택하며, Windows 실기 검증은 사용자 담당으로 둔다. 실제 수행하지 않은 Windows 네이티브 빌드·DPAPI·GUI 검증을 통과로 기록하지 않는다.

## 대시보드 등록 유지

- 프로젝트 속성·앱 ID·문서·빌드 출력 경로·파일별 버전·LAN URL·배포 방식이 바뀌면 `.dashboard-props.json`을 갱신한다. 설명은 앱 목적과 핵심 기능을 담은 한 문장(가급적 120자 이내)으로 유지하고 작업 진행·이력을 덧붙이지 않는다.
- `/Users/jujin/workspace/dashboard/DASHBOARD_GUIDE.md`를 따르고 수정 후 대시보드 검증기를 실행한다. JSON은 Git 제외 상태를 유지하며 비밀정보를 기록하거나 서명·설치 가능 여부를 추측하지 않는다.

## 검증 범위

- 2026-10-25 사용자 요청에 따라 기본 기능 검사와 유닛 테스트를 중심으로 검증한다.
- Android API별 전체 계측이나 장시간 반복 계측으로 검증 범위를 자동 확장하지 않는다.

검사 명령과 예상 환경 skip은 `docs/DEVELOPMENT.md`를 따른다. 문서·스크립트 정리는 링크·구문·관련 유닛 검사로 먼저 검증하고 GUI를 불필요하게 실행하지 않는다.
