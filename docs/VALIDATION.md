# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

Android 복귀 검사에서 기존 화면 위에 45% dim·`연결 확인 중…`을 즉시 표시합니다.
렌더링 timeout은 의도적인 취소와 구분해 출력 구독을 해제하며, tmux 화면 오류는 기존 SSH의 구독·스냅샷으로 복원합니다.
정상 연결 확인 뒤에도 바뀐 pane의 ACK가 남으면 dim과 입력 차단을 유지합니다. 자동 화면 복원 실패는 수동 재시도로 전환합니다.

| 검사 | 이번 결과·범위 |
|---|---|
| JVM | `:app:desktopTest`: 89 통과·환경별 예상 skip 9. timeout/취소 구분·취소된 교체 작업의 정리 순서·늦은 ACK·구독/세대·정상 검사 중 pane 재동기화 포함. 명시적 이름 목록 대조 통과 |
| Rust | `dgl-core`·`dgl-uniffi`: 42 통과·기본 ignore 6. 공개 인터페이스·저장 형식 변경 없음 |
| 격리 SSH | tmux 3.7c의 정상 복귀·소켓 무응답·출력 역압력 중 3초 판정·원격 작업 유지·입력 무재전송 검사 1 통과 |
| Python | 스크립트 단위검사 98 통과. 변경한 JVM/Android 검사 이름 목록 포함 |
| Android 빌드 | Debug·계측 APK 컴파일·lint 통과. Rust 네 ABI 포함, 원본 APK 서명/payload 검사 통과. lint 오류 0·기존 검토 진단 12개 |
| API36 선택 계측 | 서로 다른 4개 검사 통과·skip 0: 출력 복구·연결 수명·번들 렌더러 IME/ACK 3개를 함께 통과한 뒤, 정리 순서 보강에 대해 출력 복구·여러 pane/화면 보존 2개를 재확인. 15초 timeout 뒤 SSH 세대·tmux client·원격 프로세스 유지, 현재 표시 pane ACK 대기·늦은 ACK·표면 교체·수동 복구, 유지 끔·사용자 종료·인증 실패·삭제된 세션 보존 확인 |
| 정적 검사 | 번역 생성 일치·변경 문서의 로컬 링크·diff 공백 검사 통과 |

실행 로그는 `.tools/resume-fix-*`와 `artifacts/android-resume/`에 보존합니다.
초기 연결 수명 검사는 에뮬레이터의 System UI 응답 없음 대화상자가 포커스를 점유해 IME 단계에서 실패했습니다.
대화상자를 닫고 해당 검사를 통과했으며, 후속 pane 재동기화 보강에 맞춰 확인/복구 표시 기대값을 수정한 뒤 선택 검사 3개를 함께 통과했습니다.
실패 실행을 통과 수에 포함하지 않습니다. 검사 설정은 실행 도구의 `finally`에서 복원합니다.
물리 기기의 장시간 잠금·Doze 증상의 직접 원인은 확정하지 않았으며 실제 첫 표시 프레임의 지연을 측정한 결과도 아닙니다.
macOS GUI·Windows 네이티브·전체 API matrix·장시간 반복 검사는 수행하지 않았습니다. Windows 빌드·패키징 경로는 유지했습니다.

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
