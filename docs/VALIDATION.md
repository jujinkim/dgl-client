# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-28)

Android 너비 840dp 미만의 하단 단축키는 기본 두 줄(70dp)로 표시하며, 설정을 끄면 기존 한 줄(36dp)로 복원합니다.
Kotlin·Rust의 `mobileShortcutBarTwoRows`는 누락 시 `true`이고 기기에만 저장합니다. 넓은 작업 공간은 한 줄을 유지합니다.

| 검사 | 이번 결과·범위 |
|---|---|
| JVM | `:app:desktopTest`: 92 통과·환경별 예상 skip 9. 신규/이전 설정 기본 ON·OFF 저장/재시작과 공유 설정 fixture 검증, 명시적 이름 목록 대조 통과 |
| Rust 코어·UniFFI | `cargo test --locked -p dgl-core -p dgl-uniffi`: 42 통과·SSH/tmux ignore 6. 설정 round trip·127개 공유 contract case·클라우드 제외 검사 포함. contract 검사명 갱신 후 관련 5개 재검사 통과 |
| Android 빌드 | Debug·계측 APK 컴파일·lint 통과. Rust 네 ABI 포함 원본 Debug APK 서명/payload 검사 통과. lint 오류 0·기존 검토 경고 12개 검증 통과 |
| Python·웹 | `python3 -m unittest discover -s scripts/tests -v`: 98 통과. `npm test --prefix web`: 36 통과 |
| 정적 검사 | 번역/자리표시자·킥오프 체크섬·Gradle wrapper·변경 Rust 파일 포맷·문서 로컬 링크·diff 공백·기존 대시보드 메타데이터 검증 통과 |

기존 Android 배치 검사는 위→아래→다음 열 순서·같은 열 너비·start 정렬·고정 버튼·아랫줄 CLI 메뉴·공동 가로 스크롤,
빈/한 개/홀수 목록 높이와 OFF 복원, 839/840dp·넓은 작업 공간의 한 줄 유지 및 전체 오른쪽 열 위 메뉴 배치를 보강했습니다.
연결된 Android 기기/에뮬레이터가 없어 계측은 실행하지 않았으며 컴파일 결과를 실기 통과로 계산하지 않습니다.
실행 로그는 `.tools/shortcut-two-rows-{gradle,rust,python,web}.log`와 최종 `.tools/shortcut-two-rows-final-gradle.log`에 남겼습니다.
저장소 전체 `cargo fmt --all --check`는 기존 `crates/dgl-core/tests/password_shell.rs` 포맷 차이로 실패했고 변경한 Rust 파일은 개별 검사에서 통과했습니다.
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
