# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

Android 복귀 시 연결 응답 확인·터미널 복원과 Android/데스크톱 공통 3회·1초 재시도 정책 검사입니다.
이전 작업 공간·연결 수명 검사는 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust 전체 | 코어/UniFFI 31·데스크톱 60, 총 91 통과. 외부 SSH 6·Keychain 1은 기본 ignore이며 통과로 계산하지 않음 |
| Rust 복구 | 가상 시계로 999ms 동안 재시도 없음/정확히 1초마다 최대 3회·소진 후 복귀 재시작·성공 후 예산 초기화·사용자 정지 보존·중복 검사 병합·별도 제어 슬롯·검사 중 입력 차단/종료 확인 |
| 격리 SSH/tmux 3.7c | 새 복귀 검사 1개 통과. 정상 복귀에서 SSH/tmux client 유지·별도 exec 확인·출력 큐 포화와 열린 소켓 무응답 중 3초 판정·동일 pane PID/화면/입력 복구·오래된 구독/대상/검사 결과와 입력 재전송 거부 |
| 일반 SSH | 로컬 russh 서버의 비밀번호/일반 shell 검사 통과. 응답 확인 뒤 연결·pane 세대 유지와 입력 재개 포함 |
| JVM | 67 통과·외부 SSH/Windows 예상 skip 9개. 새 3개는 중복 복귀·현재 구독/모든 pane 스냅샷 완료·이전 구독/주기 거부·shell·실패 dim 유지·resize 단독 제외를 검사. 명시적 결과 검증기 갱신 |
| Python·JavaScript | Python 98·터미널 JavaScript 13 통과. 격리 실행기의 Cargo `--` 구분자 전달도 실제 SSH 명령으로 확인 |
| Android 빌드 | Debug·계측 APK·lint 통과. 기존 검토 진단 12개 일치. 네 ABI Rust payload·Debug 서명 확인과 전달용 APK 수집 |
| API36 선택 계측 | `AndroidConnectionLifetimeTest`와 `AndroidPlatformTest`의 창 렌더러 선택, 총 2개 통과. IME·홈 복귀/응답 확인·화면 준비 후 dim 해제 상태·Activity 재생성·알림 Intent·task 제거와 원격 PID 보존·새 실행 시 빈 연결 목록·실제 HOME 런처에서 시작한 루트 뒤로가기/복귀 확인. 기존 45% dim 아래 창 전체 보관·원래 pane 삭제/부분 복원/스냅샷 취소 중 기존 화면 유지·표시 완료 뒤 보관 해제 확인 |
| 무결성·문서 | 한국어/영어·생성 번역/자리표시자, 요구사항 SHA-256·Wrapper·diff 공백·로컬 대시보드 등록 검증. JSON Git 제외 유지 |

데스크톱 첫 전체 실행의 런처 메뉴 검사는 가짜 연결의 재시도 세대가 클릭 중 바뀌어 실패했습니다.
해당 레이아웃 fixture를 종료된 연결로 고정한 뒤 전체 60개가 통과했습니다.
API36 초기 뒤로가기 검사는 instrumentation 시작을 HOME 시작으로 취급해 실패했습니다.
[Android 16의 기본 루트 뒤로가기](https://android.googlesource.com/platform/frameworks/base/+/refs/heads/android16-release/services/core/java/com/android/server/wm/ActivityClientController.java)는 HOME 실행 출처를 검사하므로 실제 런처 아이콘에서 시작하도록 검사를 고쳤습니다.
제품의 뒤로가기 처리는 재정의하지 않았습니다.

API36은 전용 `Dgl_API36` 한 대만 사용했으며 IME·글꼴 설정과 fixture를 실행기의 `finally`에서 원복했습니다.
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
