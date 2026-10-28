# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-28)

Android 설정에 기기 로컬 ‘키보드 항상 보이기’를 추가했습니다. 기본은 꺼짐이며, 켜면 활성 터미널에서
키보드를 자동 표시·복원하고 리사이즈를 적용합니다. 크기 옵션과 키보드 닫기 버튼은 비활성화하며,
끄면 저장된 이전 크기 옵션을 복원합니다. 토글 해제 자체는 키보드를 닫지 않습니다.
렌더러 준비·Activity 수명·창/DOM 포커스·IME 애니메이션을 확인하고, 대화상자·작성 창·비활성 터미널에서는
포커스를 가져오지 않습니다. 지연된 요청을 무효화하고 복구 중 입력 차단·대상 세대 검사를 유지합니다.

| 검사 | 이번 결과·범위 |
|---|---|
| JVM | `:app:desktopTest`: 99 통과·예상 환경 skip 9. 명시적 이름 목록 대조 통과 |
| Rust | 코어·UniFFI 46 통과·환경 의존 ignore 6. 설정 왕복·127개 계약 fixture·동기화 로컬 보존 포함 |
| 웹·스크립트 | 웹 56·Python 99 통과. DOM 포커스 알림·기존 IME/입력 회귀와 검사 목록 대조 포함 |
| Android 빌드 | 원본 Debug APK·계측 APK·`lintDebug` 통과. 기존 검토 진단 12개 목록 검증 통과 |
| Android 계측 | 전용 API36에서 이번 기능 1개 통과·skip 0, 62.790초. 명시적 이름 목록 검증 통과 |
| IME·포커스 | 설정 토글·크기 옵션 잠금/복원·DOM 포커스 이탈·Back·복구 중 입력 차단·작성 창·pause/resume·활성 분할 터미널·해제 후 수동 닫기 확인 |
| 정적 | 번역·버전 대조·변경 문서 로컬 링크·diff 공백·대시보드 메타데이터 검증 통과. 변경 Rust 파일의 edition 2024 형식 검사 통과 |

최종 계측 결과·원문은 `artifacts/keyboard-always-visible/summary.json`·`instrumentation-final.txt`,
빌드는 `.tools/keyboard-always-android-check.log`에 있습니다. 초기 설정 fixture 예상값 실패,
접근성 선택 상태·해제 후 IME 준비 실패와 Back 전환 시 Activity 종료는 최종 통과에 포함하지 않았습니다.
Back이 Activity까지 전달될 때 소비하도록 보완했고, 초기 계측 원문은 같은 artifacts 폴더에 보존했습니다.
전체 `cargo fmt --all -- --check`는 이번 변경 밖 `crates/dgl-core/tests/password_shell.rs`의 기존 형식 차이로 실패했습니다.

APK는 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 원본입니다.
Android/IME가 강제로 숨긴 경우 재표시까지 잠시 사라질 수 있으며, 백그라운드·다른 창에서 강제 표시하지 않습니다.
삼성 키보드·물리 기기·실제 SSH·macOS GUI/Keychain·데스크톱 재패키징·Windows 네이티브·전체 API matrix·장시간 반복은 수행하지 않았습니다.
이전 변경의 실행 요약은 [HISTORY.md](HISTORY.md)에 있습니다.

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
