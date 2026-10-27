# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-28)

Android IME 높이를 layout에서 적용하고 WebView 논리 높이와 그룹별 표면을 유지합니다.
키보드 패널 이동에 관성을 추가하고, 하단 두 줄 단축키를 독립 너비·공동 스크롤로 바꿨습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| 웹 | `npm test --prefix web`: 40 통과·skip 0. 기존 36개와 IME 셀 측정 생략, 80ms 속도·250ms 감속·경계 정지·취소/지연 프레임·입력 누출 방지 추가 |
| JVM | `:app:desktopTest`: 92 통과·환경별 예상 skip 9, 명시적 이름 목록 대조 통과 |
| Android 빌드 | Debug·계측 APK 빌드와 `lintDebug` 통과. 원본 Debug APK의 서명·ID/SDK·Rust 네 ABI payload 검증 통과. lint 오류 0·기존 검토 경고 12개 검증 통과 |
| 정적 검사 | 킥오프 체크섬·Gradle wrapper·문서 로컬 링크·diff 공백·대시보드 메타데이터 검증 통과 |
| Python | `python3 -m unittest discover -s scripts/tests -v`: 98 통과. 새 계측 이름을 명시적 목록에 등록 |
| API36 분할 | 전용 `Dgl_API36`의 작업 공간 2개 계측 통과. 839/840dp·분할/전환·IME 전후 양쪽 WebView 크기 불변·폰트/스크롤 확인 |
| API36 모바일 | 모바일/렌더러 4개 계측 통과. 독립 폭·홀짝 순서·공동 스크롤·고정 도구 접근성·빈/홀수/한 줄·설정 복원·WebView 유지 확인. CLI 메뉴의 IME 유지·알림/포커스·원격 열/터치 스크롤 확인 |
| 사용자 실기 확인 | 사용자가 APK를 폰에 설치해 확인한 뒤 체감이 나쁘지 않다고 보고. 기종/OS·정량 프레임 측정은 제공되지 않음 |
| 짧은 IME 진단 | 애니메이션 1배·480×800dp·software GPU에서 수정 전후 열기/닫기 각 5회. WebView 크기 변경 62→0회, 원격 resize 모두 0회. 앱 창 `TOTAL_DURATION` p95 167.17→193.72ms(137/138 프레임), 개선 미확인, 누락 콜백 모두 0 |

측정 원시 ns·환경·APK SHA와 비교는 `artifacts/android-ime/before.json`, `after-final.json`, `comparison.json`에 있습니다.
작업 공간 계측 원문은 `artifacts/android-workspace/workspace.txt`, 모바일 원문은 `artifacts/android-ime/mobile.txt`, `controls.txt`에 있습니다. 7개 이름별 통과 대조는 `artifacts/android-ime/verification.json`에 있습니다.
최종 p95가 기준선보다 높아 이 조건에서 프레임 성능 개선은 확인하지 못했습니다.
원시 분포의 중앙값은 83.64→88.00ms이며 최종 138개 중 200ms 초과 6개가 있었습니다.
사용자 실기 체감과 AVD 수치를 별도로 기록하며, 이 표본만으로 실기 성능 악화를 판정하지 않습니다.
이 짧은 앱 프레임 처리 시간 진단은 SurfaceFlinger 표시 지연·물리 기기 성능·전체 성능 인수를 증명하지 않습니다.
320dp 높이의 가로 화면은 패널 도구를 접은 조건에서 검사합니다. 고정 도구의 합보다 창이 작을 때의 접근성은
[TASKS.md](../TASKS.md)에 남아 있으며 이번 검사로 해결됐다고 보고하지 않습니다.
APK는 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 원본을 사용합니다.
에이전트는 실제 SSH/CLI·물리 Android 정량 계측·macOS GUI·Windows 네이티브·전체 API matrix·장시간 반복 검사를 수행하지 않았습니다.

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
