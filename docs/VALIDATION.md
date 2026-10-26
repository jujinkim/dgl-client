# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

Android 터미널의 가짜 가로 overflow를 제한하고, 탭별 폰트 핀치와 최신 기본값 초기화를 검사했습니다.
이전 SSH 복귀·복구 표시·아이콘 검사는 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| JVM | 71 통과·외부 SSH/Windows 예상 skip 9개. 핀치 누적·10/28 제한·잘못된 배율 거부·탭별 독립성·기본값 변경 후 유지·최신 기본값 초기화·탭 수명 검사 포함. 명시적 결과 검증기 일치 |
| Python·JavaScript | Python 98·터미널 JavaScript 16 통과. 원격 창/일반 SSH 격자별 스크롤 경계, 창 축소 후 위치 보정, 모든 pane 폰트 반영·핀치 중 입력/resize 차단·종료 시 최종 resize와 IME 중첩 검사 포함 |
| Android 빌드 | Debug·계측 APK·lint 통과. 기존 검토 진단 12개 일치. 원본 Debug APK의 네 ABI Rust payload·개발용 서명 확인; APK 복사본 생성 없음 |
| API36 터미널·작업 공간 | 전용 `Dgl_API36` 한 대의 Compose/WebView와 결정적 연결 fixture로 작업 공간 2개·pane 렌더러 1개 통과. 숨긴 shell의 가짜 overflow 재현/차단·실제 넓은 원격 창 접근·실제 글자 크기 변화·핀치 취소·초기화 버튼·기본값 저장 불변·탭/Activity 수명·IME·분할/탭 이동·독립 pane 화면 확인 |
| 무결성·문서 | 한국어/영어·생성 번역/자리표시자, 요구사항 SHA-256·diff 공백·문서 링크·로컬 대시보드 등록 검증. JSON Git 제외 유지 |

계측의 화면 크기·밀도·IME·글꼴·애니메이션 설정은 실행기의 `finally`에서 원복했습니다.
초기 핀치 계측은 좌표가 시스템 최소 핀치 거리보다 작아 실패해 휴대폰 밀도/물리 좌표로 보정했습니다.
직접 실행한 기존 작업 공간 검사는 IME 전환에서 실패했고, 밀도·애니메이션을 먼저 고정하는 공식 실행기로 2개 모두 통과했습니다.
이번 계측에는 실제 SSH 서버·네트워크 재접속을 포함하지 않았으며 Rust/UniFFI 검사도 재실행하지 않았습니다.
실제 손가락 최근 앱 swipe·화면 잠금/기기 절전·물리 네트워크 전환, 전체 API matrix·장시간 반복 계측,
Windows 네이티브 빌드·DPAPI·GUI와 macOS GUI는 수행하지 않았습니다.

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
