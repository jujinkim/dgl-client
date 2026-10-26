# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

앱의 표시 이름·제목·탐색기·기본 강조 색상·훅 해제 메뉴를 DaGaLi로 통일했습니다.
Android 왼쪽 상단은 공통 원본 도형에서 생성한 24dp 앱 아이콘만 표시하며 연결 중과 대화면에서도 유지합니다.
Android 앱 라벨, macOS `DaGaLi.app` 번들 이름과 Windows 알림·시작 메뉴 이름도 갱신했습니다.
이전 변경의 검증 이력은 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust 데스크톱 | `cargo test --locked -p dgl-desktop`: 65 통과·대화형 Keychain 1 ignore. UI·작업 공간·입력·설정 회귀 검사 포함 |
| Kotlin/JVM | `:app:desktopTest`: 78 통과·예상 환경 skip 9. `verify_test_results.py --mode linux`의 명시적 목록·skip 검증 통과 |
| Android 빌드·lint | Debug APK·계측 Kotlin 컴파일·`lintDebug` 통과. 네 ABI Rust 엔진·UniFFI 바인딩 갱신. lint 오류 0·기존 진단 12를 `verify_lint.py`로 확인 |
| Android APK | 원본 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 갱신. aapt의 `DaGaLi` 라벨·앱 ID와 `collect_rust_android.py`의 버전·SDK·네 ABI·서명 검사 통과. APK 복사본 생성 없음 |
| macOS 패키징 | `artifacts/rust-candidate/mac/DaGaLi.app` 빌드·로컬 ad-hoc 서명·strict 검증 통과. 번들 표시 이름·기존 ID·아이콘 연결 확인 |
| 스크립트·정적 검사 | Python 98 통과. 번역 목록·생성 Compose 벡터를 포함한 아이콘 재생성 일치, 앱 UI의 이전 브랜드 문구 잔존 검사, 변경 문서의 로컬 링크·diff 공백·원문 요구사항 체크섬·대시보드 스키마 검사 통과 |

실제 Android 화면·런처 조작, macOS GUI·IME·Keychain과 Windows 네이티브 빌드·DPAPI·GUI는 수행하지 않았습니다.
전체 Android API 계측·장시간 반복은 수행하지 않았으며 예상 skip·ignore를 통과 수에 포함하지 않았습니다.

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
