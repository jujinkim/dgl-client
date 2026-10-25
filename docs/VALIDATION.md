# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

시스템·DGL 기본·사용자 지정 강조 색상과 플랫폼 창 스타일, 동기화 v2 변경의 검사입니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust workspace (macOS ARM64) | 코어·UniFFI 27 통과/SSH 환경 필요 5 ignore, 데스크톱 51 통과/Keychain 환경 필요 1 ignore |
| 색상·설정 | 이전 설정 기본값, 잘못된 값의 국소 복구, HEX 정규화·저장 복원, 두 밝기 테마·8개 프리셋·검정/흰색의 대비, 중립 배경 유지, 조회 실패 대체색 |
| 동기화 | v1 공유 파일·캐시와 직접 serde 역직렬화 경로 전환, 데이터셋 ID·이력 유지, 지연 v1/v2 병합, 강조 선택의 원자적 전달·동시 충돌 선택, OS 실제 색상 필드 제외 |
| 데스크톱 UI | 프리셋 즉시 저장, HEX Enter 적용·오류 시 기존 값 유지, 모드 전환·재실행 복원, 입력 컴포넌트·포커스 유지, 모달 Tab 순환·터미널 입력 차단. OS 변경·복귀 갱신 조건 단위 검사 |
| Kotlin/JVM | `:app:desktopTest` 및 명시적 이름 검증기: 46 통과·예상 skip 9 |
| Android | `:androidApp:assembleDebug :androidApp:lintDebug` 성공. lint 오류 0·기존 검토 경고 12, 진단 검증기 통과. 전체 API 계측·실제 동적 색상 변경은 실행하지 않음 |
| macOS 패키징 | `scripts/package_rust_mac.py`로 개발 .app 생성·로컬 ad-hoc 서명·검증. Developer ID·공증은 수행하지 않음 |
| 기본 회귀·무결성 | Python 98·웹 8 통과, 원문 요구사항 SHA-256·Gradle Wrapper·rustfmt·Markdown 로컬 링크·`git diff --check`·대시보드 메타데이터 검증 |

macOS 네이티브 GUI 확인은 시도했으나 computer-use의 앱 식별자 연결 실패
(`Running application not found` / `Invalid app`)로 창을 관찰하지 못했습니다.
제목 표시줄 색·기본 버튼·드래그·resize·전체 화면 전환과 OS 설정 변경의 실제 반영은 **미검증**입니다.
격리 설정·검증용 복사본을 사용했으며 검증 프로세스는 종료했습니다. GPUI 이벤트 주입은 실제 OS 키보드·IME 인수 결과가 아닙니다.
Windows MSVC 네이티브 빌드·고대비·DPAPI·GUI 검사는 사용자 담당이며 이번에 실행하지 않았습니다.
Windows 조건부 의존성·IME 메시지 처리·빌드·패키징 진입점은 유지합니다.

Gradle 10 대비 deprecated 기능과 Rust 의존성 `block`/`proc-macro-error2`의 향후 호환성 경고는 남아 있습니다.
로컬 실행 로그는 `/tmp/dgl-accent-*.log`, JVM 이름별 결과는 Git 제외 `artifacts/accent-jvm-tests.json`입니다.
과거 검사는 [HISTORY.md](HISTORY.md)와 Git 이력에서 확인합니다.

## 남은 한계

- macOS 실제 GUI·네이티브 선택창·IME·Keychain 저장/재기동·OS 알림 인수.
- Windows MSVC 네이티브 빌드·DPAPI·파일 선택·GUI·알림: 사용자 확인 범위.
- Android 물리 기기·실제 한국어/일본어 키보드·Wi-Fi↔셀룰러 전환·장시간 백그라운드 연결.
- 실제 에이전트 훅의 서비스별 범위는 [AGENT_STATUS.md](AGENT_STATUS.md). 전체 로그인·승인·장시간 인수와 실험 서비스 검증은 남음.
- tmux capture/format은 모든 VT 내부 상태·현재 SGR pen을 직렬화하지 않음.
  로컬 기록·선택·스크롤 위치는 기기 상태이며 원격 tmux 전체 히스토리 복원을 보장하지 않음.
- tmux prefix·키 바인딩·상태줄·팝업·copy-mode 화면 재현은 GUI 범위 밖.
- 정확한 첫 표시 프레임: 17개 unresolved, proven 0, invalid_trace.
  성능·자원 수집 도구의 존재와 일부 수집 성공은 전체 성능 인수 완료가 아님.
- 공개 배포용 서명·공증·현재 의존성 전체 감사는 미완료. [SECURITY.md](SECURITY.md) 참고.
