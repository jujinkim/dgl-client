# 남은 작업

2026-10-25 기준. 현재 실행 구조는 Rust 단일 엔진이며 기본 검증은 기능 검사와 유닛 테스트입니다.
작업·커밋은 `main`에서 진행합니다. 완료 이력을 여기에 누적하지 않고 [HISTORY.md](docs/HISTORY.md)와 Git 이력을 사용합니다.

## 기능 인수

- [ ] macOS 실제 GUI에서 호스트 등록→지문 확인→인증→tmux 선택→단절/재접속→종료 확인.
  실제 키보드·IME·clipboard·TUI·탭/resize와 Keychain 저장 후 재실행 포함.
- [ ] Windows MSVC x64 네이티브 빌드·ZIP 실행·개인키 선택·DPAPI·알림·GUI 확인.
  사용자 담당이며 [Windows 체크리스트](docs/WINDOWS.md)를 따른다.
- [ ] Android 물리 기기에서 실제 한국어/일본어 키보드, 선택·스크롤, Wi-Fi↔셀룰러 전환 확인.
- [ ] 각 플랫폼의 생성·이름 변경·삭제 취소, 빈 목록·오류·복구 화면과 보호 저장 실패 상태를 실제 화면에서 인수.
- [ ] 데스크톱 OS 접근성 브리지·스크린리더 지원 검토. GPUI 자동화를 실기 접근성 통과로 계산하지 않는다.

## 배포 전

- [ ] 현재 Rust/Gradle/npm 의존성의 취약점·라이선스 검토 및 실제 전달물 고지 확인.
  과거 OSV 0건 기록을 현재 Rust 전체 그래프의 결과로 재사용하지 않는다.
- [ ] Android 정식 서명, macOS Developer ID·공증, Windows 서명·배포 방식 결정과 산출물 검사.
- [ ] GitHub Secret scanning·Push protection 상태 확인. 미확인 상태를 활성으로 기재하지 않는다.

## 별도 요청 시 진행

- 정확한 첫 표시 프레임 계측·성능 인수·장시간 자원 검사: [성능 계약](docs/PERFORMANCE.md).
  API36 첫 표시 진단의 `proven=0`, `unresolved=17`, `invalid_trace`는 미해결 상태다.
- 실제 CLI 전체 로그인·승인 흐름, 장시간 재접속, macOS/Windows OS 알림 실기.
  AGY·Grok Build는 실험 지원이며 실제 로그인·유료 호출 인수는 남아 있다.
- Linux 앱·WSL 연동·사용자 데이터 이전. 현재 실행·자동 이전 기능으로 안내하지 않는다.

전체 Android API matrix나 장시간 계측은 기본 작업에 자동으로 추가하지 않습니다.
현재 검사 근거와 제약은 [VALIDATION.md](docs/VALIDATION.md)를 따릅니다.
