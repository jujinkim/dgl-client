# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

데스크톱·Android CLI 실행 메뉴에서 AGY·Grok Build의 `test` 배지와 내부 구분선을 제거했습니다.
서비스명 바로 뒤에 번개 아이콘만 표시하고 가장 긴 행의 내용에 맞춰 메뉴 폭을 정합니다.
일반/Bypass 실행 명령·대상 세대 검사·비활성 처리·닫힘과 포커스 복귀를 유지합니다.
이전 변경의 검사 이력은 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust 데스크톱 | `cargo test --locked -p dgl-desktop`: 68 통과·대화형 Keychain 1 ignore. 명령 8종·단일 전송·대상 변경 차단과 밝은/어두운 테마의 내용 기준 폭·서비스명 직후 번개·44px 클릭 영역·배지 없음·비활성·닫힘·포커스 복귀 포함 |
| Kotlin/JVM | `:app:desktopTest`: 78 통과·환경상 예상 skip 9. 명령 8종·단일 전송·연결/pane/입력 세대 변경·복구/모달 차단과 기존 회귀 검사. `verify_test_results.py --mode linux`로 명시적 이름 목록과 대조 |
| Android 빌드 | JDK 17·SDK 36 환경의 `assembleDebug`·`assembleDebugAndroidTest`·`lintDebug` 통과. lint 오류 0·기존 검토 진단 12개 일치. 메뉴 계측 검사는 8종 명령·48dp 번개 영역·서비스명 직후 배치·배지/Bypass 텍스트 없음으로 갱신했으며 실행하지 않음 |
| 원본 APK | `collect_rust_android.py`: `androidApp/build/outputs/apk/debug/androidApp-debug.apk`의 ID `kim.jujin.dglc`, 버전 `0.1.0`/`1`, min/target SDK 34/36, 네 ABI·Rust payload·v2 서명 검증 통과. 복사본 없이 원본 경로와 해시를 `artifacts/rust-candidate/android/build.json`에 기록 |
| 정적 검사 | 번역 목록·생성 Kotlin·번호 자리표시자, 데스크톱 Rust 서식·diff 공백·문서 로컬 링크·대시보드 스키마·원문/Wrapper 무결성 검사 통과 |

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
