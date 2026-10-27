# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

같은 대상의 크기 변경 중 키·텍스트 입력을 기존 제한된 큐로 받고, 최초 크기 변경 즉시 제출·이후
최소 50ms 병합·실패 후 150ms 재시도를 적용했습니다. 실제 연결 확인·복구와 좌표 입력은 분리합니다.
첫 자동 재시도는 즉시, 이후 1초 간격으로 최대 3회이며 소진 시 터미널 중앙에서 다시 시도합니다.
Android는 실제 표시 전환, 데스크톱은 전면 전환·UI tick 간격으로 복귀를 검사합니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust·UniFFI·데스크톱 | `cargo test --locked --workspace`: 116 통과. 격리 환경/Keychain용 기본 ignore 7개는 통과로 계산하지 않음. 가상 시간 즉시·1초·1초 재시도, 중복 검사/접속·사용자 정지, 크기만 바뀐 입력 대상, 좁은 창·분할의 중앙 버튼 위치/연속 클릭, IME 조합·입력 모드 유지 포함 |
| 격리 SSH | tmux 3.7c의 `resume::healthy_resume_and_blackhole_recovery_preserve_work_without_replaying_input` 1개 통과. 정상 SSH 재사용, 무응답·출력 역압력 중 3초 판정, 동일 tmux PID, 실패 입력 미재전송, 크기 캡처 지연 중 한글·영문 입력 순서/중복 확인 |
| JVM | 83 통과·외부 SSH/Windows 환경 예상 skip 9. 결과 검증기 명시적 목록 대조 통과 |
| 웹 | 34 통과·skip 0. 연속 크기·IME 세대, 조합 위치, 스냅샷 중 방향키/붙여넣기 모드, 좌표 보고 차단 중 키·텍스트 수신과 무재전송 포함 |
| Android | Debug·계측 APK·lint 빌드 통과. 기존 검토 lint 진단 12개 일치. API36 전용 `AndroidConnectionLifetimeTest`와 `AndroidMobileInputTest#exhaustedRetryIsCenteredAndRetainsTheTerminal` 2개 통과·skip 0 |
| Python·정적 검사 | 98 통과. 번역 생성 일치·원문 체크섬·Wrapper·문서 링크·diff 공백·대시보드 메타데이터 검사 통과 |

API36 연결 수명 검사는 홈·백그라운드 복귀, IME, Activity 재생성, 유지 설정 켜짐/꺼짐,
수동 종료·재시도 정지·task 제거, 삭제된 세션·인증 실패 보존을 확인했습니다. 중앙 버튼 검사는
화면 안 중앙 배치·상단 중복 버튼 없음·클릭 후 연결/터미널 유지를 확인했습니다.
로그는 `artifacts/recovery-sync/`, 원본 APK 검사 결과는 `artifacts/rust-candidate/android/build.json`에 있습니다.
Windows 빌드 경로는 유지했으며 Windows 네이티브·물리 Android 키보드·macOS 실기 절전/GUI·전체 API matrix·장시간 검사는 수행하지 않았습니다.
데스크톱 절전 복귀는 3초 이상의 UI tick 간격을 사용하므로 긴 UI 정지도 연결 검사 계기가 됩니다.

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
