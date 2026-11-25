# 남은 작업

2026-11-25 기준. 현재 실행 구조는 Rust 단일 엔진이며 기본 검증은 기능 검사와 유닛 테스트입니다.
작업·커밋은 `main`에서 진행합니다. 완료 이력을 여기에 누적하지 않고 [HISTORY.md](docs/HISTORY.md)와 Git 이력을 사용합니다.

## 기능 인수

- [ ] SFTP 탐색기와 Finder·Windows 탐색기·Linux 파일 관리자(X11·Wayland) 사이에서 일반 파일을 양방향으로 끌어 놓고, 폴더나 심볼릭 링크가 섞인 업로드의 전체 거절과 기존 파일 보존을 확인. 터미널 분할과 파일 탐색기를 함께 열고 초점·키 입력도 실제 창에서 확인. Windows 실기 확인은 사용자 담당.
- [ ] Android 시스템 선택기가 업로드에서 일반 파일 여러 개를 선택하도록 표시하는지 확인하고, 다운로드 저장 폴더의 기존 파일에 대한 덮어쓰기·취소를 실기에서 확인.
- [ ] Ubuntu 24.04 x86_64 네이티브 빌드·단위 검사·AppImage 패키징/추출/실행 및 X11·Wayland 각각의
  시작→Local 연결→입력→종료 인수([Linux 절차](docs/LINUX.md)). 이 macOS 작업 환경에서는 미수행.
- [ ] Windows psmux 3.4 이상 실제 여러 세션 조회·전환·이름 변경·화면 복원·출력 흐름·작업 유지 인수.
  공통 fixture와 Windows MSVC 빌드 경로 유지는 실제 psmux·DPAPI·GUI 통과를 뜻하지 않음.

- [ ] macOS 제목 표시줄 배경·기본 제목/신호등 버튼·드래그·resize·전체 화면 전환과 시스템 강조색 변경을 실제 창에서 확인.
  자동 UI 검사로 네이티브 인수를 대체하지 않는다.
- [ ] Windows 10/11 시스템 강조색 갱신·고대비 전환·기본 창 버튼/Snap과 Android 동적 강조색·API 35 이상 창 모드 제목 표시줄을 실제 환경에서 확인.

- [ ] macOS 실제 GUI에서 호스트 등록→지문 확인→인증→tmux 선택→단절/재접속→종료 확인.
  실제 키보드·IME·clipboard·TUI·탭/resize와 Keychain 저장 후 재실행 포함.
- [ ] (우선) Windows MSVC x64 네이티브 빌드·포터블 ZIP 실행·개인키 선택·DPAPI·알림·GUI 확인.
  사용자가 먼저 직접 사용하는 배포 형태다. 사용자 담당이며 [Windows 체크리스트](docs/WINDOWS.md)를 따른다.
- [ ] Android 물리 기기에서 실제 한국어/일본어 키보드, 선택·스크롤, Wi-Fi↔셀룰러 전환과 태블릿·폴더블 창 크기/마우스 조작 확인.
  삼성 키보드를 열자마자 한글을 입력할 때 조합 글자의 커서 부착·최종 문자열을 영문/숫자와 비교하고, 폰트 초기화 FAB가 키보드·포커스를 유지하는지 확인.
  길게 눌러 선택·끌어서 확장·복사·붙여넣기·링크 열기를 일반 셸·마우스 모드 TUI·비활성 pane·키보드 표시 상태에서 확인하고,
  haptic, IME가 뜨지 않는지, 마우스 모드에서 선택이 화면에 그려지는지 확인.
  키보드가 떠 있을 때 길게 눌러도 IME·선택이 유지되는지, 시스템 길게 누르기 지연 시간(짧게·길게)별로 손을 떼도 선택이 남는지,
  화면 폭으로 나뉜 긴 URL의 링크 인식과 `줄바꿈 없이` 복사를 확인.
  키보드가 닫힌 상태에서 길게 눌러도 키보드가 뜨지 않고 선택이 유지되는지, 탭은 손을 뗄 때 키보드를 여는지,
  선택 시작·끝 핸들을 끌어 범위를 고칠 때 핸들 위치·손가락 오프셋·haptic·메뉴 재표시가 자연스러운지, 출력이 흐를 때 핸들이 따라가는지,
  `화면 터치 시 키보드 열기`를 끄면 탭이 키보드를 열지 않고 하단 키보드 버튼으로만 열리는지(실제 Chromium `inputmode=none` 동작) 확인.
  홈 이동·화면 잠금·기기별 절전 정책과 최근 앱 카드 swipe 후 연결/서비스 종료도 실기에서 확인.
  API36 단일 task 제거 API 검사는 물리 기기 인수를 대신하지 않는다.
- [ ] Android 가로 화면에서 IME가 남긴 높이가 고정 헤더·탭·입력 도구의 합보다 작아도 하단 입력 도구에 접근하도록 배치 보완.
  키보드가 닫힌 760×320dp에서 패널 도구를 펼칠 때도 발생한다. 패널 도구를 접으면 공간을 확보할 수 있다.
  API36/Gboard의 800×411dp 가로 화면에서 하단 도구가 키보드에 가려진다.
- [ ] 같은 tmux 창을 데스크톱과 Android 실기에서 함께 열고 창 크기가 마지막 포커스·입력 기기를 따르는지 확인.
  데스크톱 창 활성화·탭 전환·절전 복귀, Android 앱 복귀·IME 입력·마우스 모드 탭, 한쪽 Wi-Fi 단절 후 자동 재접속이 크기를 빼앗지 않는지 포함.
  격리 SSH/tmux 통합 검사는 두 control client 수준의 근거이며 GUI 포커스 인수를 대신하지 않는다.
- [ ] 각 플랫폼의 생성·이름 변경·삭제 취소, 빈 목록·오류·복구 화면과 보호 저장 실패 상태를 실제 화면에서 인수.
- [ ] 데스크톱 OS 접근성 브리지·스크린리더 지원 검토. GPUI 자동화를 실기 접근성 통과로 계산하지 않는다.

## 에이전트 상태 HUD

단위 검사는 합성 입력과 임시 HOME만 사용했습니다. 아래 실기 항목은 확인 전까지 미검증으로 둡니다([에이전트 상태](docs/AGENT_STATUS.md#에이전트-상태-hud)).

- [x] 2026-11-22 로컬 macOS 확인(임시 HOME·전용 tmux 소켓, `observer.py install --status` 실제 경로, 모델 호출 소량): Claude Code 2.1.330 구독 계정에서
  `env.CLAUDE_CODE_PLUGIN_DIRS`의 DGL mod가 대화형 TUI에 로드되어 `$.fs.write`로 `<pid>.json`을 썼다. 모델·컨텍스트 %·카테고리 분해 9행·캐시(1h TTL, 두 번째 요청 적중률 99.8%)·비용·
  5시간/7일 한도·구독 추정이 들어왔고, `serve` poll이 네이티브 바이너리 PID로 pane에 붙였으며 훅의 effort와 합쳐졌다. Rust `Snapshot` 역직렬화·validate 통과.
- [ ] Claude 남은 실기: 원격 SSH 호스트, npm 런처 설치, API 키·Bedrock 계정, project·local·managed 설정의 `env`가 같은 키를 덮는 경우, 해제 후 원래 값 복원.
- [x] 2026-11-22 Codex 0.171.0 데몬 귀속 수정·로컬 확인: 훅을 실행하는 `codex app-server` 데몬을 CLI 관측에서 빼고, 스레드를 작업 디렉터리가 같은 TUI에 묶는다.
  두 pane·고아 데몬 모두 자기 pane에 붙었고 `PostModelRequest`의 모델·컨텍스트·캐시가 들어왔다.
- [x] 2026-11-22 같은 디렉터리의 여러 Codex TUI 구분(로컬 macOS 실기): 데몬 소켓 연결 끝점으로 클라이언트 TUI를 좁히고, 스레드 UUIDv7 생성 시각을 TUI 시작 시각과 맞춰 시작 스레드를 연결한다.
  TUI 3개가 각자 자기 pane에 붙고 `/new` 스레드는 버려짐을 확인했다.
- [ ] Codex 남은 실기·개선: 원격 Linux 호스트(`/proc` 작업 디렉터리, `ss -xp` 소켓 대응은 합성 출력 검사만), 같은 디렉터리 여러 TUI 중 한 곳의 `/new`·`/resume`(현재 버림),
  `codex --cd`의 이후 `/new`, IDE·데스크톱 앱이 같은 데몬을 쓰는 경우, 추론 수준을 직접 고른 스레드의 state DB 값.
- [x] 2026-11-22 로컬 macOS 확인: AGY 1.5.14(Google AI Pro)에서 statusline 래퍼가 호출되고 자식 프로세스가 `TMUX_PANE`을 상속해 pane에 연결됐다.
  모델·effort·컨텍스트·캐시·`plan_tier`·quota 버킷(`gemini-5h`, `gemini-weekly`, `3p-5h` 등, `remaining_fraction`)이 들어오고 훅 이벤트도 기록됐다.
- [ ] AGY 남은 실기: 사용자 statusline이 있을 때 출력 유지, 해제 후 복원, quota 버킷 ID의 사람이 읽을 이름 표시.
- [ ] 실제 Grok Build 세션 폴더의 `signals.json`·`usage.json` 구조와 `StopFailure`의 `errorType`/`error` 키 확인.
- [ ] 상태바 칩·팝오버·사이드바 패널·설정 카드와 Android 칩·바텀시트를 macOS·Windows·Android 실제 화면에서 확인.
  Windows MSVC 빌드와 좁은 폭에서 항목 생략 순서 포함. 자동 UI 검사로 실기 표시를 대체하지 않는다.
- [ ] 상세 상태 수집을 켠 실제 구독 계정에서 Codex app-server 한도와 Grok ACP 크레딧의 잔여량·초기화 시각을 확인.
- [ ] 추가 선택 소스 검토. AGY `agy -p /usage` 수동 조회, Grok statusline, Codex MCP statusline 공급자가 대상이다. 대화 파일 접근을 도입할 때는 별도 동의와 문서 공개가 필요하다.

## 배포 전

- [ ] 현재 Rust/Gradle/npm 의존성의 라이선스 검토 및 실제 전달물 고지 확인.
- [ ] 의존성 예외를 2026-12-15까지 재검토(만료 2027-01-27). GPUI의 유지보수 중단 간접 의존성(pin-utils 포함)·
  호환 수정판 제약, uniffi 코드 생성의 basic-toml, AGP의 guava 선택을 추적하고 상위 갱신 시 예외를 제거한다.
- [ ] RSA 수정판 `>=0.10.0`을 허용하는 russh 릴리스로 갱신. 현재 `russh 0.65.0`은 `rsa =0.10.0-rc.18`을
  고정하므로 기존 RSA 키 지원을 유지하며 임의 교체하지 않는다.
- [ ] Android 정식 서명, macOS Developer ID·공증 방식 결정과 산출물 검사.
- [ ] Windows Store 제출(포터블 ZIP 이후 후속): Partner Center 앱 이름 예약 후 `AppxManifest.xml` Identity 교체,
  개인정보처리방침 URL 준비, `-Msix` 빌드와 개발자 모드 등록으로 실행·DPAPI·알림 확인 후 제출.
  ZIP은 미서명으로 유지한다([Windows 안내](docs/WINDOWS.md#4-microsoft-store-msix)).
  **Identity 교체 상태: 미완료.** 사용자가 완료했다고 명시적으로 알리기 전까지 완료로 바꾸지 않는다.
- [ ] GitHub Secret scanning·Push protection 상태 확인. 미확인 상태를 활성으로 기재하지 않는다.

## 별도 요청 시 진행

- 정확한 첫 표시 프레임 계측·성능 인수·장시간 자원 검사: [성능 계약](docs/PERFORMANCE.md).
  현재 Android 계측은 SSHJ 참조 엔진으로 WebView를 구동하므로 Rust 엔진 수치가 필요하면 하네스를 Rust 연결로 옮긴다.
  API36 첫 표시 진단의 `proven=0`, `unresolved=17`, `invalid_trace`는 미해결 상태다.
- 실제 CLI 전체 로그인·승인 흐름, 장시간 재접속, macOS/Windows OS 알림 실기.
  AGY·Grok Build는 실험 지원이며 실제 로그인·유료 호출 인수는 남아 있다.
- WSL 연동·사용자 데이터 이전. 현재 실행·자동 이전 기능으로 안내하지 않는다.

## 검사 보완

- [ ] `AgentNotificationTest#channelPermissionAndClickIntentRetainOnlyEventId`는 시작 시 알림 권한이 허용돼 있으면
  권한 회수로 앱 프로세스가 종료되어 실패한다. 시작 상태와 무관하게 회수 경로를 검사하도록 고친다.

전체 Android API matrix나 장시간 계측은 기본 작업에 자동으로 추가하지 않습니다.
현재 검사 근거와 제약은 [VALIDATION.md](docs/VALIDATION.md)를 따릅니다.
