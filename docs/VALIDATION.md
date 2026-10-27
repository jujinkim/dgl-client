# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

텍스트 작성의 `Enter 함께 보내기`는 텍스트 전송 성공 후 별도 Enter 키 요청을 보냅니다.
붙여넣기 데이터에는 추가 Enter를 합치지 않으며 각 단계의 대상 검사·완료 확인과 실패 입력 무재전송을 유지합니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust 데스크톱 | `cargo test --locked -p dgl-desktop`: 77 통과·대화형 Keychain ignore 1. 텍스트 성공 뒤 독립 Enter 요청, 각 단계 완료까지 초안/편집 잠금 유지, 큐 거절·실패·취소·대상 변경 시 Enter 차단, 크기만 바뀐 대상 허용, 현재 pane의 붙여넣기 모드·64 KiB 경계 포함 |
| JVM | 86 통과·외부 SSH/Windows 환경 예상 skip 9. 텍스트/Enter 순서·각 단계 완료 대기·인코더 거절·큐 실패·대상 변경·초안 보존 포함. 결과 검증기 명시적 목록 대조 통과 |
| 웹 | 34 통과·skip 0. 일반/스냅샷 bracketed paste 데이터에 추가 Enter 없음, 작성한 줄바꿈 유지, 제어 문자 포함 64 KiB 경계 포함 |
| Android | `:app:compileAndroidMain :mobileUi:compileDebugKotlin :androidApp:compileDebugKotlin` 통과. Rust 바인딩 및 네 ABI 라이브러리 빌드 포함. APK 패키징·lint·계측은 이번 변경에서 실행하지 않음 |
| Python·정적 검사 | 98 통과. 번역 생성 일치·원문 체크섬·Wrapper·문서 링크·diff 공백·대시보드 메타데이터 검사 통과 |

JVM 결과는 `artifacts/jvm-tests.json`에 있습니다. 이번 변경에서 실제 SSH/CLI·Android 계측·물리 키보드·macOS GUI·Windows 네이티브·장시간 검사는 수행하지 않았습니다.
Windows 빌드·패키징 경로는 유지했습니다. 전송 완료는 기존 입력 큐가 보고하는 결과이며 원격 애플리케이션의 처리 완료를 뜻하지 않습니다.

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
