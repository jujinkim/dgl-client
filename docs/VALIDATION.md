# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

데스크톱 알림을 버튼 위의 패널로 분리하고, 반복 설명·진단 ID를 네이티브 툴팁으로 옮겼습니다.
기존 포커스 링·입력 차단·읽음·종료된 대상 검증을 유지합니다.
이전 변경의 검사 이력은 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust 데스크톱 | `cargo test --locked -p dgl-desktop`: 68 통과·대화형 Keychain 1 ignore. 패널 위치·리사이즈·320px 창·빈/100건 목록·스크롤·닫기·포커스·입력 차단·종료된 알림 대상과 기존 UI 회귀 검사 포함 |
| 툴팁 영역 | GPUI 영역 클리핑·스크롤·크기 변경·가림·제거·창 종료 해제와 한국어/영어 문자열 검사 통과. 기존 `.tooltip` 호출 제거, 드래그 미리보기 유지 |
| AppKit 브리지 | 네이티브 NSView 영역 등록·UTF-8 소유자 콜백·문구/영역 갱신·해제·정리 호출 검사 통과. 실제 표시·OS 지연을 확인한 결과는 아님 |
| Windows 어댑터 | 정확히 같은 Win32 어댑터를 별도 최소 crate에서 `x86_64-pc-windows-gnu` 대상으로 타입 검사 통과. MSVC 전체 빌드·GUI 검사는 사용자 담당이며 미수행 |
| macOS 패키징 | `artifacts/rust-candidate/mac/DaGaLi.app` 빌드·로컬 ad-hoc 서명·strict 검증 통과. computer-use는 `cgWindowNotFound`로 창을 찾지 못해 대표 툴팁 표시 확인은 미완료 |
| 정적 검사 | 번역 목록·생성 Kotlin 일치·번호 자리표시자, 데스크톱 Rust 서식·diff 공백·문서 로컬 링크·대시보드 스키마 검사 통과 |

Kotlin/JVM·Android 빌드·계측·실제 SSH 연결·장시간 반복 검사는 이번 UI 변경에서 재실행하지 않았습니다.
전체 Rust 서식 검사는 기존 `password_shell.rs` 차이를 유지하고 데스크톱 패키지에만 적용했습니다.
예상 skip·ignore와 Windows 타입 검사를 네이티브 실기 통과로 계산하지 않습니다.

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
