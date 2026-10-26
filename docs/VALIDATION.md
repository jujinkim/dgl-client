# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

Android의 정상 복귀 확인과 실제 복구 표시를 분리하고, 백그라운드 유지 설정을 끈 연결의
일회성 자동 재연결을 검사했습니다. 이전 복귀·아이콘 검사는 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust 코어/UniFFI | 31 통과. 외부 SSH 6개는 기본 ignore이며 통과로 계산하지 않음. 일반 SSH 비밀번호·shell·응답 확인 검사 포함 |
| 격리 SSH/tmux 3.7c | 복귀/무응답 검사 1개 통과. 정상 확인 시 SSH/tmux client·입력/렌더링 대상 세대 유지와 Reset 없음, 명시적 snapshot 유지. 출력 큐 포화·열린 소켓 무응답의 3초 판정, 동일 pane PID/화면 복구와 실패 입력 무재전송 확인 |
| JVM | 69 통과·외부 SSH/Windows 예상 skip 9개. 정상 확인의 화면/입력 분리, 현재 구독·주기·세대의 모든 pane 표시 완료, 오래된 구독/주기 거부, shell 표시 조건·실패 dim 유지·resize 단독 제외. 자동 복귀 예약의 일회성 소비·수동 취소·실패/삭제 세션 제외 포함. 명시적 결과 검증기 일치 |
| Python·JavaScript | Python 98·터미널 JavaScript 13 통과 |
| Android 빌드 | Debug·계측 APK·lint 통과. 기존 검토 진단 12개 일치. 원본 Debug APK 네 ABI Rust payload·개발용 서명 확인; APK 복사본 생성 없음 |
| API36 연결 수명 | 전용 `Dgl_API36` 한 대에서 `AndroidConnectionLifetimeTest` 1개 통과(39초). IME·정상 복귀의 복구 표시/버튼 없음·SSH/tmux client 유지·Activity 재생성·알림 Intent·유지 끔의 연결/서비스 종료와 같은 탭/세션/pane/PID 복귀·중복 복귀의 단일 client·수동 종료/재시도 정지/task 제거의 예약 취소·최근 앱 제거 후 빈 연결 목록·루트 뒤로가기·삭제 세션/인증 실패 보존 확인 |
| 무결성·문서 | 한국어/영어·생성 번역/자리표시자, 요구사항 SHA-256·Wrapper·변경 Rust 서식·diff 공백·문서 링크·로컬 대시보드 등록 검증. JSON Git 제외 유지 |

계측의 IME·글꼴 설정과 SSH fixture를 실행기의 `finally`에서 원복했습니다.
Android 일반 SSH의 새 셸 복귀·실제 출력 구독 실패 주입은 이번 기기 계측에 포함하지 않았습니다.
일반 SSH 엔진과 표시 완료 조건은 Rust/JVM 단위 검사 범위입니다.
실제 손가락 최근 앱 swipe·화면 잠금/기기 절전·물리 네트워크 전환, 전체 API matrix·장시간 반복 계측,
Windows 네이티브 빌드·DPAPI·GUI와 macOS GUI는 수행하지 않았습니다.

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
