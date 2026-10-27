# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

Android 840dp 미만 화면에 상단 알림·설정 아이콘과 독립 `tmux` 버튼, 버튼 아래 알림 팝업,
터미널 아래의 36dp 열린 연결 탭·36dp 단축키 줄을 적용했습니다. 탭 닫기는 기존 연결 제거 경로를 사용합니다.
알림 표시 중 입력 차단·키보드 유지·바깥 터치 소비·닫은 뒤 포커스 복원을 확인했으며,
폭 전환 시 IME 복원 캡처가 해제 콜백 순서에 따라 덮어써지지 않도록 보존합니다.
대화면 그룹·배치와 연결별 WebView를 유지합니다. 외부 API·Rust/UniFFI·설정 저장 형식은 바꾸지 않았습니다.
이전 변경의 검사는 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Kotlin/JVM | `:app:desktopTest`: 82 통과·환경상 예상 skip 9. 팝업 오른쪽 정렬·화면 여백·키보드 경계와 기존 회귀 검사. `verify_test_results.py --mode linux`의 명시 목록 대조 통과 |
| Android 빌드 | JDK 17·SDK 36의 `assembleDebug`·`assembleDebugAndroidTest`·`lintDebug` 통과. `verify_lint.py`: 오류 0·기존 검토 진단 12개 일치 |
| API36 전용 에뮬레이터 | `workspace_android.py` 최종 구간별 검사: workspace 2·mobile-input 2·compact 6, 총 10 통과·skip 0. 명시적 이름 목록 대조. 320dp 상단 버튼·36dp 줄과 배치 순서, 수동 탭 스크롤·선택·닫기, 빈/긴 알림·목록 스크롤·읽음/이동·닫기/바깥 터치/뒤로 가기·입력 차단/포커스·IME 유지, 839↔840dp·분할/드래그·WebView 동일성 및 기존 입력/볼륨키/계층/폰트 회귀 포함 |
| 계측 fixture | 구형 셸 fixture에 현재 렌더러의 대상·의미 키 입력 계약을 제공하고, WebView 준비·IME 조합 완료·설정 스크롤·키보드 표시 상태를 기다린 뒤 검사하도록 갱신. 숨긴 측정 셸 overflow는 초기 자동 맞춤에 의존하지 않는 고정 폭으로 재현 |
| 원본 APK | `collect_rust_android.py`: `androidApp/build/outputs/apk/debug/androidApp-debug.apk`의 ID `kim.jujin.dglc`, 버전 `0.1.0`/`1`, min/target SDK 34/36, 네 ABI·Rust payload·v2 서명 검증 통과. 경로·해시는 `artifacts/rust-candidate/android/build.json` |
| 정적 검사 | Python 98개, 번역·번호 자리표시자, 원문/Wrapper 무결성, 변경 문서 로컬 링크·diff 공백·대시보드 메타데이터 검증 통과 |

계측 결과는 `artifacts/android-workspace/`에 남기며 화면 크기·밀도·IME·글꼴·애니메이션 설정은 복원했습니다.
Rust·데스크톱 검사·실제 SSH/CLI 수신·물리 Android·macOS GUI/패키징·Windows 네이티브·전체 API matrix·장시간 검사는 이번 변경에서 수행하지 않았습니다.
환경상 예상 skip은 통과로 계산하지 않습니다.

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
