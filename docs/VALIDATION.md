# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-28)

Android 하단 CLI 실행 메뉴는 열린 키보드를 유지하고, 입력이 허용되는 크기 변경 복원 중에도 열린 상태를 유지합니다.
키보드 위 공간이 부족하면 메뉴 내부를 스크롤합니다. 실제 복구·대상 변경에 대한 폐기와 전송 직전 세대 검사는 유지합니다.

| 검사 | 이번 결과·범위 |
|---|---|
| JVM | `:app:desktopTest`: 90 통과·환경별 예상 skip 9. 크기 변경 전후 요청 유지·1회 전송·복구/대상 변경 시 거부 포함. 명시적 이름 목록 대조 통과 |
| Python | 관련 evidence 검사 6 통과. JVM/Android 검사 이름 목록과 실패·skip·누락 거부 포함 |
| Android 빌드 | Debug·계측 APK 컴파일·lint 통과. Rust 네 ABI 포함, 원본 APK 서명/payload 검사 통과. lint 오류 0·기존 검토 진단 12개 |
| API36 선택 계측 | 서로 다른 3개 검사 통과·skip 0: 기존 CLI 명령/배치/닫기/대상 검사와 알림/포커스 검사 2개, 새 CLI IME 회귀 검사 1개. 키보드 표시/숨김 유지·크기 변경 중 메뉴 유지·입력 차단·Esc/뒤로 가기/바깥 터치·포커스 복원·가로 화면 내부 스크롤 확인 |
| 정적 검사 | 변경 문서의 로컬 링크·diff 공백 검사 통과 |

실행 로그는 `.tools/cli-menu-*`와 `artifacts/android-cli-menu/`에 보존합니다.
새 계측의 첫 실행은 800×411dp 가로 화면에서 키보드가 고정 입력 도구를 가려 메뉴 버튼 대기 중 실패했습니다.
검사 내부에서 800×480dp 가로 화면을 만들고 크기·밀도를 복원하도록 보완한 뒤 해당 검사만 재실행해 통과했습니다.
초기 실패는 통과 수에 포함하지 않습니다. 더 낮은 가로 화면의 고정 UI 배치 문제는 [TASKS.md](../TASKS.md)에 남겼습니다.
에뮬레이터 크기·밀도·IME·글꼴·애니메이션 설정 복원을 확인하고 종료했습니다.
Rust 엔진·저장 형식·공개 API 변경은 없습니다. Rust 검사·실제 SSH/CLI·물리 Android·macOS GUI·Windows 네이티브·전체 API matrix·장시간 반복 검사는 수행하지 않았습니다.

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
