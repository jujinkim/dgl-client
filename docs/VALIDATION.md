# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-28)

Android는 IME를 논리 크기에서 제외하고 가시 영역에서만 처리합니다. 키보드 표시·숨김·높이 변경은 원격 resize를
요청하지 않으며, 하단 정렬한 표면을 본문 드래그·우측 overlay 스크롤바로 이동합니다. 태블릿 분할도 논리 크기를 유지합니다.
데스크톱 재시도 버튼 검사는 임의 포트 loopback 실패 fixture와 명시적으로 진행하는 Tokio 시간으로 변경했습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| 웹 | `npm test --prefix web`: 36 통과. IME 무 resize·높이 변경·빠른 반전·오래된 세대/분리된 표면·패널 범위/비례/입력 차단·재개방 하단 정렬·이력 위치·실제 창/핀치 크기 반영 |
| JVM | `:app:desktopTest`: 92 통과·환경별 예상 skip 9. IME geometry 세대·태블릿 분할 projection 포함, 명시적 이름 목록 대조 통과 |
| Rust 데스크톱 | 수정한 재시도 테스트 단독 통과 후 `cargo test --locked -p dgl-desktop`: 77 통과·Keychain ignore 1. 정지/즉시 재시도/중복 클릭·가상 시간 소진·버튼 배치·탭/터미널/연결 동일성 포함 |
| Android 빌드 | Debug·계측 APK 컴파일·lint 통과. Rust 네 ABI 포함 원본 Debug APK 서명/payload 검사 통과. lint 오류 0·기존 검토 진단 12개 |
| Python | 관련 evidence 검사 6 통과. JVM/Android 검사 이름 목록과 실패·skip·누락 거부 포함 |
| 정적 검사 | 변경 문서의 로컬 링크·diff 공백 검사 및 기존 대시보드 메타데이터 검증 통과 |

휴대폰 키보드 반복 표시/숨김과 태블릿 분할의 크기 요청 없음·WebView 동일성 계측 assertion을 보강하고 컴파일했습니다.
연결된 Android 기기/에뮬레이터가 없어 이번 계측은 실행하지 않았습니다. 이 결과를 실기 통과로 계산하지 않습니다.
실행 로그는 `.tools/ime-gradle.log`, `.tools/ime-retry.log`, `.tools/ime-desktop.log`에 남겼습니다.
APK 서명 검사의 첫 실행은 JAVA_HOME 누락으로 실패했으며 JDK 17 환경을 지정한 재실행에서 통과했습니다.
Rust 엔진·저장 형식·UniFFI 공개 API와 제품 재접속 정책은 변경하지 않았습니다.
실제 SSH/CLI·물리 Android·macOS GUI·Windows 네이티브·전체 API matrix·장시간 반복 검사는 수행하지 않았습니다.

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
