# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

Windows·macOS 공통 데스크톱의 호스트 순서 고정·기기별 드래그 정렬 저장과 세션 탭 재사용 검사입니다.
Android·코어·UniFFI·클라우드 공유 형식은 변경하지 않았습니다.
이전 세션 목록·상태줄 검사 요약은 [HISTORY.md](HISTORY.md)에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| 호스트 순서 단위 검사 | 최초 연결 시작 A→B와 반복 시작의 순서 유지, 수동 정렬 우선순위, 메타데이터/저장 배열 변경·재로드·추가/삭제, 누락/잘못된 JSON/중복 ID/잘못된 시작 구간의 fallback 검사 |
| 세션 경로 단위 검사 | 호스트 ID·세션 ID 구분, 미선택 탭 예약, 반복 선택·복구 중 기존 탭 검색, 신규 생성 차단, 닫힌 탭 재생성 경로 검사 |
| GPUI 이벤트 검사 | 호스트 드래그 삽입 표시·드롭 전 불변·드롭 확정·Esc/목록 밖 취소·순서 파일 재로드. 세션 A/B 반복 선택·다른 그룹 이동·호스트의 최근 활성 탭 선택·탭/SSH/터미널 Entity 유지·이름 변경/세션 소멸 시 ID 유지·닫은 뒤 새 Entity·복구 중 알림 이동 검사 |
| Rust 데스크톱 | `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer cargo test --locked -p dgl-desktop`: 59 통과, 대화형 Keychain 1 ignore. ignore는 미실행이며 통과에 포함하지 않음 |
| 문서·메타데이터 | 사용 안내·작업 공간·동기화/저장 설명 갱신, 기존 로컬 대시보드 등록 검증기 통과·Git 제외 유지 |
| 무결성 | 변경 Rust 파일 서식·문서 로컬 링크·`git diff --check`·원문 요구사항 SHA-256 검사 통과 |

GPUI 검사는 합성 상태와 실제 이벤트 디스패치를 사용합니다. 실제 서버의 창·pane 전환이나 네트워크 복구 인수를 대신하지 않습니다.
Windows MSVC 네이티브 빌드·DPAPI·GUI와 macOS 실제 GUI·Keychain·OS 알림,
실제 SSH/tmux 통합·Android 빌드/계측은 이번에 실행하지 않았습니다.
Windows 기본 빌드·ZIP·수동 CI 경로는 유지합니다.
기존 Rust 의존성 `block`/`proc-macro-error2`의 향후 호환성 경고는 남아 있습니다.

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
