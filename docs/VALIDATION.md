# 검증 범위와 현재 상태

기본 검증은 기능 검사·유닛 테스트입니다. 전체 Android API 계측·장시간 soak·실기 인수는 별도 범위입니다.
검사 명령은 [DEVELOPMENT.md](DEVELOPMENT.md), 남은 작업은 [TASKS.md](../TASKS.md)를 따릅니다.
과거 실행 로그·시도별 결과는 [HISTORY.md](HISTORY.md)와 Git 이력에 요약·보존합니다.

## 현재 변경 검사 (2026-10-26)

앱 아이콘 제작과 Android·macOS·Windows 리소스 연결의 검사입니다.
이전 강조 색상·동기화 검사 요약은 [HISTORY.md](HISTORY.md)에 있습니다.

| 검사 | 이번 결과·범위 |
|---|---|
| 원본·축소 미리보기 | built-in imagegen 투명 원본·최종 프롬프트 보존, 1024·128·48·32·16px 정적 확인. 32px에서 Return 화살표와 라임 결과물 구분 |
| Android 마스크·단색 | 원형·둥근 사각형 정적 확인. 전경의 의미 있는 픽셀은 모든 밀도에서 중심 반경 33dp 이내, 별도 단색 벡터의 화살표·슬라임·반짝이 확인 |
| Android 빌드 | `:androidApp:assembleDebug :androidApp:lintDebug` 성공. lint 오류 0·기존 검토 경고 12, `verify_lint.py` 통과. Debug 전달용 복사본 갱신 |
| macOS | `cargo check --offline -p dgl-desktop`와 `package_rust_mac.py` 성공. ICNS 해상도별 추출, 번들 ICNS 일치·`CFBundleIconFile`·앱 ID 확인, 로컬 ad-hoc 서명·strict 검증 |
| Python | 기존 Windows 패키징 3개를 포함해 98개 단위 검사 통과, 변경 스크립트 구문 검사 통과 |
| 재생성·메타데이터 | `generate_app_icons.py --check` 일치, PNG 원본 alpha 보존·ICO 7개 크기 확인, 로컬 대시보드 PNG 등록·스키마 검증·Git 제외 확인 |
| 변경 파일 무결성 | 변경 Rust build.rs 서식·Markdown 로컬 링크·`git diff --check`·원문 요구사항 SHA-256 확인 |

처음 Android 빌드에서는 런처와 알림이 공유하던 `ic_terminal`을 삭제해 알림 참조가 실패했습니다.
알림용 리소스를 복원한 뒤 위 Debug·lint 명령을 다시 실행해 성공했습니다.
전체 `cargo fmt --all -- --check`는 이번에 수정하지 않은
`crates/dgl-core/tests/password_shell.rs`의 기존 서식 차이로 실패했습니다.
변경한 `desktop/build.rs`의 `rustfmt --check`는 통과했습니다.

Windows MSVC 네이티브 빌드·창/작업 표시줄 아이콘·DPAPI·GUI 검사는 이번에 실행하지 않았습니다.
macOS Dock·Finder와 Android 실제 런처 표시를 GUI로 확인하지 않았으며,
Android API별 계측·전체 Rust/JVM·SSH 통합 검사도 이번 리소스 변경에서는 재실행하지 않았습니다.
Developer ID·공증은 수행하지 않았습니다. 기본 Windows 빌드·ZIP·수동 CI 경로는 유지합니다.

Gradle 10 대비 deprecated 기능과 Rust 의존성 `block`/`proc-macro-error2`의 향후 호환성 경고는 남아 있습니다.
실행 로그는 Git 제외 로컬 `/tmp/dgl-icon-*.log`입니다. 재생성·미리보기 안내는
[앱 아이콘 README](../assets/app-icon/README.md)를 따릅니다.

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
