# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-28)

Android 탭 재부착·tmux 창 전환의 행별 전달을 최대 16 KiB 묶음으로 변경했습니다.
모든 보이는 pane의 최신 캡처와 렌더가 끝날 때까지 불투명 로딩 표시와 입력 차단을 유지합니다.
첫 캡처도 렌더·paint 기회 후 ACK하며, 이전 화면·일반 버퍼 복원 중간 화면을 노출하지 않습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust | core·UniFFI 45 통과·환경 ignore 6. 16 KiB 경계·UTF-8/escape 순서·출력 예산·구독 교체 검사 포함 |
| JVM | `:app:desktopTest`: 95 통과·환경별 예상 skip 9, 명시적 이름 목록 대조 통과 |
| 웹·Python | 웹 42·Python 99 통과. 첫 렌더 ACK·취소 프레임·새 명시 목록 검사 포함 |
| Android 빌드 | Debug·계측 APK·`lintDebug` 통과. lint 오류 0·기존 검토 경고 12개, 진단 목록 검증 통과 |
| API36 실제 SSH·WebView | `tabRebindAndWindowSwitchWaitForFreshVisiblePanes` 통과. 두 pane 일반/alternate 버퍼, 마지막 ACK 지연 중 불투명 픽셀·입력 차단, pane당 캡처 write 1회, 탭 재부착·창 변경·원격 PID 유지 확인 |
| API36 회귀 | 출력 timeout·재시도·오래된 ACK, WebView 입력·IME·snapshot, 작업 공간 탭·분할·IME 유지의 3개 이름별 완료 확인. 작업 공간은 초기 System UI ANR 실패 후 재시작한 별도 실행에서 통과 |
| 정적·산출물 | 로컬 문서 링크·diff 공백·대시보드 메타데이터·원본 Debug APK/4 ABI 검사 통과 |

JVM 결과는 `artifacts/tab-presentation/jvm-tests.json`, 빌드와 계측 원문은 `.tools/tab-presentation-*.log`에 있습니다.
초기 API36 실행은 핵심 3개 통과·작업 공간 1개 실패였으며, 실패한 실행 전체를 통과로 계산하지 않습니다.
작업 공간만 재시작 후 실행한 결과는 `.tools/tab-presentation-workspace.log`에 있습니다.
초기 빌드는 기존 coroutines BOM 1.6.4 POM의 검증 해시 누락으로 실패했습니다. Maven Central 원본·공개 SHA-256과
로컬 파일의 일치를 확인해 해당 항목만 추가한 뒤 최종 빌드를 통과했습니다.

전용 API36/software GPU에서 요청부터 복원 완료 콜백까지 탭 재부착 248ms·창 전환 675ms를 기록했습니다.
40행·두 버퍼의 xterm 파서 단독 실험은 162회 write 약 206ms, 묶음 write 약 2ms였고 최종 버퍼·모드가 같았습니다.
이 값들은 실제 첫 표시 프레임·물리 기기의 체감 속도나 동일 환경의 제품 전후 비교를 증명하지 않습니다.
에뮬레이터 설정은 검사 후 복원하고 종료했습니다.
APK는 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 원본을 사용합니다.
물리 Android·macOS GUI/Keychain·데스크톱 재패키징·Windows 네이티브·전체 API matrix·장시간 반복 검사는 수행하지 않았습니다.

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
