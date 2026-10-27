# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-28)

Cargo workspace 버전을 단일 기준으로 Android 표시·APK·데스크톱·패키징에 연결했습니다.
두 플랫폼의 설정 마지막에 About app을 추가하고, 데스크톱 타이틀바와 Android 알림 아이콘을 갱신했습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| Rust 데스크톱 | 80 통과·Keychain ignore 1. 초기 제목·탭/그룹 선택·긴 원격 제목·마지막 탭 닫기와 About 왕복·설정 스크롤/포커스·입력 차단 포함 |
| JVM | `:app:desktopTest`: 92 통과·환경별 예상 skip 9, 명시적 이름 목록 대조 통과 |
| Android 빌드 | Debug·계측 APK·`lintDebug` 통과. 원본 Debug APK의 서명·ID/SDK·Rust 네 ABI payload 검증 통과. lint 오류 0·기존 검토 경고 12개 |
| API36 설정 | About 정보/설치 버전·두 ACTION_VIEW 링크·버튼/Android 뒤로 가기·스크롤/포커스 복원·WebView 유지·터미널/볼륨 입력 차단 1개 계측 통과. 브라우저 인텐트는 모니터로 검사 |
| API36 알림 | 에이전트 2개와 연결 수명 1개 이름별 완료 통과. 24/48px 흰색/투명 도형·smallIcon 리소스·에이전트 클릭·연결 유지 알림 클릭 확인. 연결 수명은 격리 OpenSSH/tmux 3.7c 사용 |
| Python | 99 통과. macOS Cargo 버전/target 디렉터리·번들/메타데이터 회귀 추가, Windows ZIP 메타데이터는 현재 workspace 버전과 일치하는 fixture로 검사 |
| 패키징·버전 | macOS `.app` 빌드·로컬 ad-hoc 서명/검증 통과. Cargo·생성 Kotlin·원본 APK·macOS 번들 두 필드/build.json이 모두 `0.1.0`, Android code `1` |
| 정적 검사 | 앱/UI 아이콘·번역 생성물·킥오프 체크섬·Gradle wrapper·변경 Rust 포맷·문서 로컬 링크·diff 공백·대시보드 메타데이터 검증 통과 |

로그는 `.tools/about-*.log`, API36 원문·이름별 결과는 `artifacts/about-app/`에 있습니다.
초기 설정 테스트의 메뉴 대상·포커스 부모/자식 판정을 수정하고, 터치 모드의 진입 버튼 포커스 허용을 보완한 뒤
About 단독 검사가 통과했습니다. 연결 수명 케이스는 앞선 동시 실행에서 통과했으며 해당 실행의 About 실패도
`settings-lifetime-attempt.txt`에 보존합니다. 이미 허용된 알림 권한을 계측 안에서 회수하면 프로세스가 종료되므로
에이전트 알림 검사는 앱을 정지하고 권한을 회수한 뒤 별도로 실행했습니다. 실패·skip을 통과로 계산하지 않습니다.
APK는 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 원본을 사용합니다.
Windows ZIP 버전은 Python fixture 검사이며 실제 Windows 네이티브 빌드·패키징 통과가 아닙니다.
물리 Android·macOS GUI/Keychain·Windows 네이티브·전체 API matrix·장시간 반복 검사는 수행하지 않았습니다.

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
