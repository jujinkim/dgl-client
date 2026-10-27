# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-28)

공통 SVG 36개에서 Compose 벡터와 Rust 포함 목록을 생성하고 제품 UI의 문자·이모지 아이콘을 교체했습니다.
에이전트 표시는 사건 종류를 직접 사용하며, 읽지 않은 사건 우선순위·오래된 관측의 미확인 처리를 유지합니다.

| 검사 | 이번 결과·범위 |
|---|---|
| JVM | `:app:desktopTest`: 92 통과·환경별 예상 skip 9. 한국어·영어에서 사건 종류·읽지 않은 사건·다른 pane·오래된 관측/누락 선택 검사 포함, 명시적 이름 목록 대조 통과 |
| Rust 코어·UniFFI | 42 통과·SSH/tmux ignore 6. 기존 알림·관측·프로토콜 계약과 사건 종류 선택 검사 포함 |
| Rust 데스크톱 | 78 통과·Keychain ignore 1. 알림 패널·설정·작성창·분할·폰트 초기화·포커스 회귀, 읽지 않은 사건 우선순위·오래된 상태와 아이콘 매핑 검사 포함 |
| Android 빌드 | Debug·Release APK 빌드·계측 테스트 Kotlin 컴파일·lint 통과. 기존 접근성 설명 검사 참조 갱신. 원본 Debug APK 서명과 두 APK의 ID/SDK·백업 정책·번들 고지·Rust 네 ABI payload 검증 통과. Release는 미서명입니다. lint 오류 0·기존 검토 경고 12개 검증 통과 |
| Python | `python3 -m unittest discover -s scripts/tests -v`: 98 통과 |
| 아이콘 | 생성물 일치·SVG 제약 검사 통과. GPUI와 같은 resvg 의존성으로 36개 SVG를 16·20·24·96px에서 렌더링해 비어 있지 않은 마스크 확인, 크기별 도형 미리보기 확인 |
| 정적 검사 | 번역/자리표시자·킥오프 체크섬·Gradle wrapper·변경 Rust 파일 포맷·문서 로컬 링크·diff 공백·대시보드 메타데이터 검증 통과 |

제품 UI 소스를 감사하여 아이콘용 문자·이모지 잔여가 없음을 확인했습니다.
단축키 표기·터미널 데이터·사용자 문자열·Unicode 테스트 데이터는 유지합니다.
Android 계측은 실행하지 않았으며 컴파일 결과를 실기 통과로 계산하지 않습니다.
실행 로그는 `.tools/ui-icons-*.log`, 도형 미리보기는 `.tools/ui-icons-preview.png`에 남겼습니다.
APK는 `androidApp/build/outputs/apk/debug/androidApp-debug.apk`와
`androidApp/build/outputs/apk/release/androidApp-release-unsigned.apk` 원본을 사용합니다.
전체 `cargo fmt --all --check`의 기존 `crates/dgl-core/tests/password_shell.rs` 포맷 차이는 유지하고 변경 Rust 파일만 검사했습니다.
실제 SSH/CLI·물리 Android·macOS GUI·Windows 네이티브·전체 API matrix·장시간 반복 검사는 수행하지 않았습니다.

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
