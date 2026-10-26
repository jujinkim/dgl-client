# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

Android 프로세스 공유 엔진·백그라운드 SSH 유지 서비스, 입력 오류 분류와 비차단 안내,
IME 하단 정렬·최종 resize와 화면 전환 시간 변경 검사입니다.
이전 언어·실시간 제목 검사 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| 연결 수명 단위 | 서비스 시작/중단·복귀/재생성 시 연결 재사용·여러 연결 중 마지막 종료·재시도 소진·설정 해제·시작 실패/서비스 손실·task 제거 후 재시작 차단 확인 |
| 입력 단위 | NotReady·QueueFull·StaleGeneration·Cancelled/Closed 구분, 같은 복원 주기 안내 합치기, 작성창 실패 이유·초안 유지·무재전송, 같은 대상 재탐색 생략 확인 |
| 설정·동기화 | Kotlin/Rust 기본 true·예전 설정·저장/재실행, 클라우드 문서와 캐시 제외 확인. 공통 설정 fixture 갱신 |
| Rust | 코어·공통 계약·UniFFI 30 통과, 기본 SSH 통합 5 ignore는 통과에 포함하지 않음 |
| 실제 SSH/tmux | 격리 OpenSSH·tmux 3.7c의 `isolated_auth_management_input_reconnect` 1 통과. 인증·관리·입력·단절 후 재접속과 원격 프로세스/세대·실패 입력 무재전송 확인 |
| JVM | 57 통과·외부 SSH/Windows 환경의 예상 skip 9개. 추가 6개 테스트를 명시적 결과 목록에 등록하고 검증기 통과 |
| 웹 | 12 통과. IME 표시/숨김·취소/역전의 하단 정렬·최종 resize만 전달·과거 출력 열람 위치, 연속 스냅샷 캐시/ack 정리, dim 80·fade 90·선택형 blur/확대축소 120ms와 동작 줄이기 확인 |
| Python | 98 통과. JVM/Android 검사 이름 목록 일치 포함 |
| Android 빌드 | 네 ABI Rust payload·Kotlin 컴파일, Debug·미서명 Release·계측 APK 빌드 통과. lint 기존 검토 진단 12개 일치 |
| API36 단일 계측 | `AndroidConnectionLifetimeTest` 1 통과(약 12–13초). 실제 Rust SSH 연결에서 IME 표시/숨김·짧은 백그라운드 복귀·Activity 재생성·알림 앱 열기 인텐트 뒤 SSH 대상과 tmux 클라이언트 동일성 확인. `finishAndRemoveTask()` 뒤 연결/알림 정리·입력 거절·서비스 자동 재시작 없음과 원격 pane PID 보존 확인 |
| 무결성·문서 | 번역 목록·번호 자리표시자·생성 파일, 원문 요구사항 SHA-256·Gradle wrapper·diff 공백 검사 통과. 로컬 대시보드 등록 검증기 통과·Git 제외 유지 |

API36 계측의 최초 실행은 새 알림 인텐트 때문에 ActivityScenario 종료 대기에서 실패했습니다.
테스트가 원래 launch intent를 복원하도록 수정한 뒤 통과했습니다. 실패·skip을 통과로 계산하지 않습니다.
서비스에는 `specialUse` 용도와 권한을 등록하고 `START_NOT_STICKY`를 사용합니다.
partial wake lock은 10분 제한·5분 갱신이며 유지 중단/연결 종료/재시도 소진/task 제거 때 갱신 작업과 함께 해제합니다.

API36에서 task 제거 API를 검사했으며 최근 앱 카드의 실제 손가락 swipe·화면 잠금·물리 기기 절전 정책 인수는 하지 않았습니다.
새 인텐트의 다른 실제 pane 이동은 기존 연결 조작 경로와 단위 검사 범위이며 계측에서는 현재 연결 열기를 확인했습니다.
전체 Android API matrix·장시간 백그라운드/배터리·실제 한국어/일본어 키보드·Windows 네이티브·macOS GUI는 실행하지 않았습니다.
Windows 기본 MSVC 빌드·ZIP·수동 CI 경로는 유지합니다.

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
