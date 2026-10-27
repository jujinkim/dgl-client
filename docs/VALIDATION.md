# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

Android 840dp 미만 헤더를 두 줄로 구성했습니다. 윗줄은 로고·호스트/터미널 전환·기타 메뉴·알림,
아랫줄은 현재 호스트와 tmux 버튼입니다. 설정은 기타 메뉴로 옮기고 단색 벡터 아이콘을 사용합니다.

| 검사 | 이번 결과·범위 |
|---|---|
| JVM | `:app:desktopTest --tests kim.jujin.dglc.MobileChromeTest`: 1 통과·skip 0. 알림 팝업의 버튼 기준 위치·화면/IME 경계 확인 |
| Android 빌드 | `:androidApp:assembleDebug :androidApp:assembleDebugAndroidTest :androidApp:lintDebug` 통과. Rust 바인딩·네 ABI 포함. lint 오류 0·기존 검토 진단 12개이며 `verify_lint.py` 대조 통과 |
| API36 선택 계측 | 3 통과·skip 0. 320dp 두 줄 헤더·긴 호스트명·설정 이동·연결 없음·알림 입력 차단/포커스 복원, 411dp 호스트/터미널 전환·tmux 시트·선택 호스트 재접속/종료, 렌더러 IME·붙여넣기·크기 변경 확인. 명시적 검사 이름 목록 대조 통과 |
| 화면 | 라이트·다크 헤더와 알림/IME 화면의 계측 스크린샷 확인. 에뮬레이터 화면 크기·밀도·글꼴·IME·애니메이션 설정은 실행 뒤 복원 |
| 정적 검사 | 번역 생성 일치·변경 문서의 로컬 링크·diff 공백 검사 통과 |

계측 로그·이름별 결과·화면은 `artifacts/mobile-header/`에 있습니다. 렌더러 검사에서 발견한 초기 실패 로그도 보존했습니다.
계측 코드에 남아 있던 이전 붙여넣기 API·키보드 입력 콜백·크기 변경 표시 기대값을 현재 동작에 맞춘 뒤 통과했습니다.
이번 변경에서 실제 SSH/CLI·물리 Android·macOS GUI·Windows 네이티브·전체 API matrix·장시간 검사는 수행하지 않았습니다.
Windows 빌드·패키징 경로는 유지했습니다.

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
