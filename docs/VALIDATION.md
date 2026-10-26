# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

Android 터미널 전환 애니메이션을 재접속 복구에만 적용하도록 제한했습니다.
이전 SSH 복귀·작업 공간·글자 크기 검사는 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| 터미널 JavaScript | 18 통과·skip 없음. 일반 크기 변경은 설정 켬/끔 모두 dim·fade·blur 없이 렌더 후 교체. 재접속 기본 fade·선택형 효과·동작 줄이기·재접속 fade를 중단하는 크기 변경과 ACK 보존 확인 |
| Android 빌드 | `:androidApp:assembleDebug` 통과. 원본 Debug APK 한 개를 갱신했으며 계측·lint는 재실행하지 않음 |
| 번역·문서 | 한국어/영어·생성 번역/자리표시자, diff 공백·로컬 대시보드 등록 검사 통과. 설정과 사용 문서를 재접속 전용 효과로 갱신 |

이번 변경은 JavaScript 단위 검사로 검증했으며 Android 계측·GUI·실제 SSH 재접속은 실행하지 않았습니다.
Rust/UniFFI·JVM 단위 검사, Windows 네이티브 빌드·DPAPI·GUI와 macOS GUI는 재실행하지 않았습니다.

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
