# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 동작의 검사 근거

2026-10-25 창 동기화 구현(`5fa6b99`)의 기록입니다. 문서 정리 작업에서 재실행한 결과와 구분합니다.

| 검사 | 기록된 결과·범위 |
|---|---|
| Rust workspace | 63 통과, 환경 필요 6 ignore. 공통 계약·출력 예산/ACK·입력 세대·GPUI 자동화 포함 |
| Kotlin/JVM | 45 통과, SSH/Windows 환경 필요 9 skip |
| 웹 브리지 | 8 통과. pane별 화면·실제 grid·resize·ACK·입력·캐시 |
| Android 빌드·lint | Debug·미서명 Release·계측 APK 빌드; lint 오류 0/경고 12 |
| SSH/tmux | 3.3a·3.7c 각 5개. 실제 PTY/GUI 입력, 창·pane 변경, 재접속 후 ID/PID·작업 경로 보존 |
| API36 전용 AVD | 최종 계측 11개. 실제 SSH·WebView·키보드·회전·계층·모바일 입력 |
| macOS 패키징 | ARM64 Debug `.app` 빌드·로컬 ad-hoc 서명 검증 |
| Windows | Rust MSVC·ZIP·수동 CI 경로와 패키저 단위 검사. 네이티브 실행은 미검증 |

3.3a 공유 fixture 일괄 실행의 간헐적인 `InvalidArgument` 때문에 최종 SSH 판정은 검사마다
새 fixture를 만든 개별 실행을 사용했습니다. API34·35는 최신 창 동기화 변경에서 재실행하지 않았습니다.
GPUI 자동화나 에뮬레이터 검사를 실제 OS GUI·IME·물리 기기 검증으로 계산하지 않습니다.

## 문서·스크립트 정리 검사 (2026-10-25)

| 검사 | 이번 결과 |
|---|---|
| Python 도구 | 98개 통과. 현재 Windows ZIP 검사·공통 증거 검사 유지, 폐기된 JVM 패키징 4개 제거, 목록/선택 결과 회귀 2개 추가 |
| Rust workspace | 63 통과, SSH/Keychain 환경 필요 6 ignore |
| Kotlin/JVM | `:app:desktopTest` 성공; 이름별 검증기에서 45 통과·예상 skip 9 확인 |
| 웹 | 8개 통과 |
| Gradle inventory·라이선스 | 생성 성공; 선택 runtime 121개 버전과 native 고지 검사 통과 |
| 문서·구문 | 로컬 Markdown 링크 106개·Python 구문·삭제 경로 참조·`git diff --check` 통과 |
| 무결성·대시보드 | 원문·Wrapper checksum, 메타데이터 스키마·Git 제외 확인 |
| 비밀정보 | Gitleaks 작업 파일·전체 staged snapshot·Git refs 3범위 통과 |

JVM 테스트 목록 54개와 Android 목록 15개를 현재 소스와 대조합니다.
API36 후보 실행기는 기본 11개, 모바일 입력 실행기는 4개를 이름별로 판정하도록 갱신했습니다.
계측 실행 없이 결과 파서의 누락·중복·실패·skip 거절을 유닛 검사했습니다.

앱 배포 파일 재생성·실제 SSH·Android 계측·Windows 네이티브 빌드·실기는 이번에 수행하지 않았습니다.
Gradle 10 대비 deprecated 기능과 Rust 의존성 `block`/`proc-macro-error2`의 향후 호환성 경고는 남아 있습니다.
로컬 로그는 Git 제외 `artifacts/docs-cleanup/`에 저장합니다.

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
