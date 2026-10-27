# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

데스크톱 하단 오류 바와 Android 입력 안내에 3초·5초·10초·영구 프리셋과 사용자 지정 ms API를 적용했습니다.
기본값은 5초이며 현재 호출부는 중요도에 따라 3/5/10초로 배정했습니다. 영구 프리셋은 API로 제공하며 기존 호출부에는 배정하지 않습니다.
자동 종료 스낵바 아래에 남은 시간 게이지를 추가하고 같은 문구 재호출·교체 시 시간을 다시 시작합니다.
Android 일반 오류 대화상자는 별도 경로입니다. 이전 변경의 검사 이력은 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust 데스크톱 | `cargo test --locked -p dgl-desktop`: 73 통과·대화형 Keychain 1 ignore. 프리셋·기본값·사용자 지정 ms·만료 경계·같은 문구 교체·오래된 닫기 거부와 GPUI 게이지 높이/위치·축소·만료·닫기/포커스·영구 노출 시 게이지 없음 포함 |
| Kotlin/JVM | `:app:desktopTest`: 81 통과·환경상 예상 skip 9. 프리셋·사용자 지정 ms·만료 경계·동일 문구 교체·오래된 만료/닫기 거부·영구 표시와 기존 회귀 검사. `verify_test_results.py --mode linux`로 명시적 이름 목록과 대조 |
| Android 빌드 | JDK 17·SDK 36 환경의 `assembleDebug`·`assembleDebugAndroidTest`·`lintDebug` 통과. `verify_lint.py`: 오류 0·기존 검토 진단 12개 일치. 계측 APK는 컴파일만 수행 |
| 원본 APK | `collect_rust_android.py`: `androidApp/build/outputs/apk/debug/androidApp-debug.apk`의 ID `kim.jujin.dglc`, 버전 `0.1.0`/`1`, min/target SDK 34/36, 네 ABI·Rust payload·v2 서명 검증 통과. 원본 경로·현재 해시는 `artifacts/rust-candidate/android/build.json`에 기록 |
| 정적 검사 | 결과 검증기 관련 Python 6개, 번역·번호 자리표시자, 데스크톱 Rust 서식·diff 공백·변경 문서 로컬 링크·원문/Wrapper 무결성 검사 통과 |

Android 계측 실행·실제 SSH/CLI 수신·macOS GUI/패키징·Windows 네이티브·장시간 반복 검사는 이번 변경에서 수행하지 않았습니다.
Rust 서식 검사는 데스크톱 패키지에 적용했습니다. 예상 skip·ignore와 계측 APK 컴파일은 실기 통과로 계산하지 않습니다.

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
