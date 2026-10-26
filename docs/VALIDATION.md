# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

탭 제목을 앞 20글자와 생략 기호로 제한하고 호스트 배지를 별도 줄로 분리했습니다.
Android 좁은 창의 중복 상태·관리 줄을 없애고 연결 메뉴에 tmux 관리·패널 도구 표시 전환을 넣었습니다.
세션·창 이름과 활성 pane 제목을 두 줄로 나누고 개수 배지·오른쪽 펼침 버튼·최상위 왼쪽 정렬을 적용했습니다.
이전 변경의 검증 이력은 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust 데스크톱 | `cargo test --locked -p dgl-desktop` 64 통과·Keychain 1 ignore. 19/20/21글자·한글·결합 문자·이모지/국기·제어 문자, 공통 계층 fixture, 활성 제목 갱신·복구 중 제목 유지 검사 포함 |
| GPUI 배치·조작 | 위 데스크톱 검사에 포함. 240/272/360px 목록의 경계·들여쓰기·메뉴, 2줄 탭·긴 호스트 배지, 분할 도구 고정, 탭 드래그·선택·연결/터미널 동일성, 복구 중 목록 위치·펼침·스크롤 유지 통과 |
| Kotlin/JVM | `:app:desktopTest` 77 통과·예상 환경 skip 9. 글자 단위 제한·공통 계층 fixture·제목 갱신·연결별 제목 캐시의 복구/대상 변경 검사 포함. `verify_test_results.py --mode linux` 결과 목록·skip 검증 통과 |
| Android 빌드·lint | Debug APK·계측 APK 컴파일·`lintDebug` 통과. lint 오류 0·기존 진단 12를 `verify_lint.py`로 확인. 계측 코드는 새 메뉴 경로·중복 정보 부재·2줄 목록에 맞춰 갱신 |
| Android APK | 원본 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 갱신. `collect_rust_android.py`의 ID·버전·SDK·Rust 네 ABI·서명 검사 통과. APK 복사본 생성 없음 |
| 문서·정적 검사 | UI 번역 목록·자리표시자, 변경 문서의 로컬 링크·diff 공백·대시보드 스키마 검사 통과. 문서/출력 경로·앱 버전·배포 방식이 같아 등록 정보 변경 없음 |

연결된 Android 기기가 없어 계측은 실행하지 않았습니다. 계측 APK 컴파일을 실제 조작·48dp 터치 영역·기기 접근성 검증으로 계산하지 않습니다.
macOS 실제 GUI·IME·Keychain, Windows 네이티브 빌드·DPAPI·GUI와 전체 Android API 계측·장시간 반복은 수행하지 않았습니다.
삼성 키보드의 한글 조합 위치·최종 문자열과 FAB의 실제 키보드 유지 확인은 남아 있습니다.

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
