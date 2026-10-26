# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

Android 창 크기에 반응하는 탐색기·세션 탭·분할 작업 공간과 입력/렌더러 수명 분리 검사입니다.
이전 패키지 정리와 연결 수명 검사 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Kotlin 작업 공간 | 분할·이동·합치기·선택/빈 그룹·최소 280×180dp/최대 16그룹·비율 제한·드래그 검증/취소·구조만 저장/잘못된 파일 대체 |
| 세션·알림 | 연결 전 호스트/세션 예약·그룹을 넘어선 기존 탭 재사용·독립 연결·창/pane 유지·오래된 연결 세대의 지연 선택 거부·실제 보이는 여러 연결의 알림 처리 |
| JVM | 64 통과·외부 SSH/Windows 환경의 예상 skip 9개. 작업 공간 새 검사 7개와 명시적 결과 검증기 포함 |
| Python·JavaScript | Python 98·터미널 JavaScript 12 통과. 새 계측 이름 목록과 소스 목록 일치 확인 |
| Android 빌드 | Debug·계측 APK·lint 통과. lint 기존 검토 진단 12개 일치. Debug 네 ABI Rust payload·서명 검사 및 전달용 APK 수집 |
| API36 작업 공간·회귀 | 작업 공간 1·기존 모바일 입력 1·볼륨키 5·계층 1, 총 8개 통과. 839/840dp·넓은 분할·탭 메뉴/드래그 이동 및 취소·WebView 동일성·키보드 유지·높이 부족 시 임시 확대/복원·활성 입력·전역 모달 차단 확인 |
| 실제 Rust 연결 수명 | 격리 SSH/tmux 3.7c fixture의 API36 선택 1개 통과(약 11초). IME·Activity 재생성·짧은 복귀·알림 Intent·task 제거와 원격 프로세스 보존 확인. 전체 API 계측으로 확대하지 않음 |
| 무결성·문서 | 번역 목록/생성 파일·자리표시자, 요구사항 SHA-256·Gradle wrapper·diff 공백 확인. 사용·작업 공간·입력·구조·검사 문서와 로컬 대시보드 등록 갱신/검증, JSON Git 제외 유지 |

API36은 전용 `Dgl_API36` 에뮬레이터 한 대만 사용합니다. UI 검사는 결정적 연결과 실제 Compose/WebView/IME를 사용하며,
이를 실제 원격 SSH나 물리 기기의 입력·백그라운드 정책 검사로 계산하지 않습니다.
화면 크기·밀도·IME·글꼴·애니메이션 설정은 실행기의 `finally`에서 원복합니다.
기존 계측 도구는 세로 스크롤 방향과 IME 커밋 뒤 접근성 선택 상태를 기다리도록 보완했으며, 실패·skip을 허용하거나 검사를 생략하지 않았습니다.
Rust wire·UniFFI 계약과 Windows 빌드·ZIP·수동 CI 경로는 유지합니다.
전체 Android API matrix·장시간 반복 계측·Windows/macOS GUI·물리 Android 인수는 수행하지 않았습니다.

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
