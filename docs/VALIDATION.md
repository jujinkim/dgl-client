# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

iOS 구현 유무 확인과 Kotlin 패키지·소스 경로·계측/검증 도구의 `kim.jujin.dglc` 통일 검사입니다.
이전 Android 연결 수명·IME 개선 검사는 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| iOS 대상 확인 | Git 관리 소스·Gradle 모듈·플랫폼 경계를 확인. iOS/iPadOS 앱, Swift 소스, Xcode 타깃·submodule이 없어 iOS 연결 수명·IME 수정/실기 검사 대상이 없음. Android 동작을 iOS에서 검증했다고 기록하지 않음 |
| 패키지 정리 | 제품·JVM/Android 검사와 참조 엔진의 Kotlin/Java 루트·import·디렉터리를 `kim.jujin.dglc`로 이동. 모바일 Rust 어댑터도 같은 루트 사용. Gradle 리소스 namespace는 `.shared`·`.mobile`·`.bindings` |
| 실행·수집 경로 | Manifest·테스트 provider/runner·CLI main class·계측 선택/이름 검증기·렌더러/Perfetto 레이어 식별·번역 생성 경로·사용 문서 갱신. APK 수집기는 새 Rust 클래스와 이전 Kotlin namespace·참조 엔진 제외를 검사 |
| 저장 호환성 | applicationId·데이터 경로·설정 형식 유지. 기존 Android Keystore 별칭·macOS Keychain 서비스·Windows DPAPI entropy 바이트는 변경하지 않음. 보존 이유를 소스·아키텍처/코어 문서에 명시 |
| Rust workspace | 코어·계약·UniFFI 30·데스크톱 60 통과. 기본 SSH 통합 5 ignore·대화형 Keychain 1 ignore는 통과에 포함하지 않음 |
| JVM | clean 후 57 통과·외부 SSH/Windows 환경의 예상 skip 9개. 새 클래스명으로 명시적 결과 검증기 통과 |
| Python | 98 통과. 소스와 JVM/Android 결과 이름 목록·새 DEX 클래스 요구·이전 namespace와 참조 엔진 거절·분석 fixture 경로 확인 |
| Android 빌드 | 네 모듈 clean 후 Debug·미서명 Release·계측 APK 빌드 통과. lint 기존 검토 진단 12개 일치. Debug/Release Manifest·백업/네트워크 정책·assets·네 ABI Rust payload 검사 통과 |
| API36 선택 계측 | 새 패키지에서 연결 수명·IME·짧은 복귀·Activity 재생성·알림 인텐트·task 제거와 원격 프로세스 보존, Keystore 왕복·변조 거절·백업 제외를 확인하는 2개 선택 검사 통과(약 14초) |
| 무결성·문서 | 번역 목록/생성 파일·자리표시자, 원문 요구사항 SHA-256·Gradle wrapper·diff 공백 검사. 로컬 대시보드 등록 검증·Git 제외 유지 |

보존 원문과 보호 저장 호환 식별자, 이전 APK 차단용 검사 문자열에는 예전 이름을 남깁니다.
제품 패키지·import·실행 경로에는 예전 Kotlin 패키지를 사용하지 않습니다.
Windows MSVC 네이티브·DPAPI·GUI와 macOS 실제 GUI·Keychain 접근, 물리 Android의 잠금·절전·최근 앱 swipe는 이번에 실행하지 않았습니다.
iOS 앱이 이 저장소에 없으므로 iOS의 실제 연결 동작을 평가하거나 수정하지 않았습니다.
전체 Android API matrix·장시간 계측은 수행하지 않았으며 Windows 빌드·ZIP·수동 CI 경로는 유지합니다.

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
