# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-27)

dagali의 앱 아이콘을 둥근 ㄱ자 엔터 머리와 두 pane의 터미널 얼굴로 갱신했습니다.
공통 도형·디자인 명세에서 원본·미리보기·Android 컬러/단색·macOS ICNS·Windows ICO를 재생성했습니다.
이전 변경의 검증 이력은 [HISTORY.md](HISTORY.md)와 Git 이력에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| 원본·재생성 | `generate_app_icons.py --check` 통과. 1024px RGBA 원본과 master 일치, 플랫폼 출력 갱신 확인 |
| 정적 미리보기 | 128·48·32·16px에서 ㄱ자 윤곽·눈·입 확인. 16px에서는 눈의 세부 형태가 단순화됨. Android 원형·둥근 사각형·단색에서 잘림·얼굴 소실 없음. [크기별](../assets/app-icon/preview-sizes.png)·[Android](../assets/app-icon/preview-android.png) 미리보기 참고 |
| Android 안전 영역·패키지 | 공통 컬러/단색 윤곽 반경 31.956dp 미만, 다섯 밀도의 불투명 영역(alpha ≥240) 반경 32dp 이내. 리샘플링 경계의 반투명 픽셀은 제외. APK에 포함된 다섯 컬러 전경의 픽셀이 생성 파일과 일치 |
| Android Debug·lint | `:androidApp:assembleDebug :androidApp:lintDebug` 통과. lint 오류 0·기존 의존성/대상 API 경고 12. 원본 `androidApp/build/outputs/apk/debug/androidApp-debug.apk` 갱신, `collect_rust_android.py`의 ID·버전·SDK·Rust 네 ABI·서명 검사 통과 |
| macOS 패키징 | ARM64 Debug 빌드·로컬 ad-hoc 서명·strict 검증 통과. 번들의 ICNS 바이트·`CFBundleIconFile` 연결과 16·32·64·128·256·512·1024px/Retina 항목 확인 |
| Windows ICO | 16·24·32·48·64·128·256px의 7개 항목과 이미지 디코딩 확인. 네이티브 빌드·GUI는 수행하지 않음 |
| Python·문서·대시보드 | 스크립트 98 통과. 변경 문서의 로컬 링크·diff 공백·대시보드 스키마 검사 통과. 아이콘·문서 경로·앱 버전이 같아 등록 정보 변경 없음 |

이번 아이콘 검사는 생성된 정적 이미지·패키지 리소스를 대상으로 했습니다.
Android 실제 런처·계측과 macOS Dock/Finder 표시는 수행하지 않았습니다.
삼성 키보드를 열자마자 한글을 입력할 때의 조합 위치·최종 문자열, 영문/숫자 비교와 FAB의 실제 키보드 유지 확인은 남아 있습니다.
Windows 네이티브 빌드·DPAPI·GUI, 실제 사용자 서버/TUI, 전체 Android API 계측·장시간 반복은 이번 범위에 포함하지 않았습니다.

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
