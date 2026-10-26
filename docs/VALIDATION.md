# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

터미널 크기 측정·resize 완료 처리를 보완하고, 연결별 글자 크기 초기화 FAB와 데스크톱 Ctrl+휠을 추가했습니다.
Android IME 세대는 재사용 렌더러가 소유하며 조합 글자는 스냅샷 복원 중 기존 위치를 유지합니다.
이전 변경의 검증 이력은 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust core·UniFFI | 35 통과. 세션 폰트 격리·범위 제한·최신 기본값 초기화와 기존 계약 확인. 격리 SSH 통합 6개는 기본 ignore |
| Rust 데스크톱 | 63 통과, 대화형 Keychain 1개는 기본 ignore. resize 최신값 병합·실패 후 동일 크기 재시도, 실제 Ctrl+휠 확대/축소·원격 전송 차단, 좁은 창/분할 FAB 위치·클릭·포커스와 기존 회귀 확인 |
| JVM·결과 검증기 | 76 통과, 외부 SSH/Windows 환경 9개는 명시적 예상 skip. 취소·재부착 IME 세대, 실패한 중간 크기의 최신값 재시도, 폰트 격리·최신 기본값 초기화와 기존 회귀 확인 |
| 터미널 JavaScript | 25 통과·skip 없음. 늦은 폰트 측정·숨김/재표시·IME 취소/재부착, 조합 중 스냅샷 교체와 표시 좌표·캐시 보조 요소 제외, 기존 확정 단일 전송·대상 세대·복구 회귀 포함 |
| Android 빌드·lint | `:androidApp:assembleDebug :androidApp:assembleDebugAndroidTest :androidApp:lintDebug` 통과. 네 ABI Rust/UniFFI와 원본 Debug APK 갱신. 계측 APK 컴파일은 실기 통과로 계산하지 않음. 기존 lint 진단 12개는 검증기의 명시적 목록과 일치 |
| Python·기본 검사 | 스크립트 98 통과, 번역/자리표시자·Wrapper·원문 체크섬·diff 공백·문서 로컬 링크·대시보드 등록 검증 통과 |

UI 동작 결과는 GPUI 이벤트 주입 검사와 웹 브리지의 좌표 검사입니다.
연결된 Android 기기가 없어 Android 실제 GUI/IME·계측은 수행하지 않았습니다.
삼성 키보드를 열자마자 한글을 입력할 때의 조합 위치·최종 문자열, 영문/숫자 비교와 FAB의 실제 키보드 유지 확인은 남아 있습니다.
Windows 네이티브 빌드·DPAPI·GUI, 실제 SSH/TUI, 전체 Android API 계측·장시간 반복은 이번 범위에 포함하지 않았습니다.

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
