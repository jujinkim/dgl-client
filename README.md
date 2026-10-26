# DGL

**Reconnect less. Resume more.**

Let DGL handle SSH reconnection and tmux reattachment, so you can pick up where you left off.

[Website · ddal.ggak.life](https://ddal.ggak.life)

DGL은 SSH 호스트 관리와 tmux 세션 조회·조작·재접속을 하나로 묶은 GUI 터미널 클라이언트입니다.
네트워크 설정을 바꾸거나 연결이 끊길 때마다 SSH에 다시 접속하고 `tmux attach -t`를
입력하던 번거로움을 줄입니다. 서버에 남아 있는 tmux 세션으로 돌아가 하던 작업을 이어갑니다.
별도 원격 데몬 없이 표준 SSH와 tmux를 사용합니다.

## 주요 기능

- **호스트 관리:** 별칭·즐겨찾기·최근 접속, 비밀번호·개인키 인증, 여러 독립 연결.
- **연결 복구:** 네트워크 단절 시 자동 재접속, 수동 재시도·정지, 기존 tmux 세션 재연결.
- **tmux GUI:** 세션·창 목록, 생성·이름 변경·삭제, pane 분할·선택·크기 조절.
  선택 창의 보이는 pane 전체를 서버의 실제 배치대로 표시하고 일반 SSH 클라이언트와 동기화합니다.
- **데스크톱 작업 공간:** 연결 시작 순서·호스트 드래그 정렬 저장, 호스트·세션별 탭 재사용, 중첩 화면 분할·탭 이동·그룹 배치 저장.
- **Android 대화면:** 840dp 이상에서 탐색기·세션 탭·중첩 화면 분할, 탭 이동과 배치 저장. 좁은 창에서는 활성 탭만 표시합니다.
- **모바일 입력:** xterm.js 터미널, 사용자 단축키 바, 여러 줄 텍스트 작성, 볼륨 키 매크로.
- **선택 기능:** Google Drive·iCloud Drive 파일 동기화, CLI 실행 메뉴, 에이전트 상태·알림.

## 시작하기

1. 서버에 SSH와 tmux를 준비하고 앱에 호스트·사용자·인증 정보를 저장합니다.
2. 처음 연결할 때 서버의 SHA-256 호스트키 지문을 독립적으로 확인합니다.
3. 기존 tmux 세션을 선택하거나 새로 만들고 창·pane을 GUI로 조작합니다.
4. 연결이 끊기면 자동 복구를 기다리거나 `새로고침 재시도`를 누릅니다.

자동 재시도는 네트워크 오류에만 **최대 3회**, 각 시도 전 **1초 대기**로 적용됩니다.
앱과 연결 탭이 유지되고 서버의 tmux 세션이 남아 있어야 작업을 이어갈 수 있습니다.
Android 앱 복귀 시 기존 연결의 응답을 확인하고 끊겼으면 복구합니다. 최근 앱 제거·프로세스 종료 후에는 직접 접속합니다.
인증 실패·호스트키 변경은 자동 재시도하지 않습니다. 복구 중 입력은 차단하고 실패한 입력을 재전송하지 않습니다.

`연결 종료`는 SSH와 tmux 클라이언트만 닫습니다. `삭제`는 확인 후 원격 대상을 종료합니다.
tmux가 없거나 지원 버전이 아니면 일반 SSH 셸로 연결되며, 일반 셸의 프로세스는 연결 종료 후 보존되지 않을 수 있습니다.
앱 재실행 시 화면 분할 배치만 복원하고 SSH 연결은 사용자가 다시 엽니다.

자세한 조작은 [사용 안내](docs/USAGE.md), pane 동기화 계약은 [tmux 창 동기화](docs/TMUX_WINDOW_SYNC.md)를 참고하세요.

## 플랫폼과 배포 상태

현재 버전은 **0.1.0 개발용 후보**입니다. 아래 빌드 경로와 실기 검증 범위를 구분합니다.

| 플랫폼 | 구현·산출물 | 확인 범위 |
|---|---|---|
| Android 14–16 | Rust + Compose/xterm.js, 네 ABI APK | 에뮬레이터 검사 기록 있음. 실제 기기·키보드·네트워크 전환 인수는 남음 |
| macOS ARM64 | Rust + GPUI/alacritty_terminal, 로컬 ad-hoc `.app` | 빌드·단위·SSH/tmux 검사. 실제 GUI·Keychain 인수는 남음 |
| Windows x64 | Rust + GPUI/alacritty_terminal, MSVC 빌드·미서명 ZIP·수동 CI | 네이티브 빌드·DPAPI·GUI 확인은 사용자 담당 |
| Linux | 후속 대상 | 현재 앱 실행·패키징 경로 없음 |
| iOS/iPadOS | 미구현 | 이 저장소에 앱 소스·Swift/Xcode 빌드 타깃 없음 |

앱 ID는 Android applicationId / macOS 번들 ID / Windows AUMID 모두 `kim.jujin.dglc`입니다.
Kotlin 제품·테스트의 패키지와 소스 경로도 `kim.jujin.dglc`로 통일합니다.
tmux Control Mode 지원 범위는 3.3–3.7이며 실제 서버 검사 기록은 3.3a·3.7c입니다.
공개 배포용 서명·공증과 전체 플랫폼 인수는 완료되지 않았습니다.

## 빌드

필수 도구·환경 설정·검사 명령은 [개발 안내](docs/DEVELOPMENT.md)에 있습니다.

```sh
# Android (JDK 17, Android SDK/NDK, Rust 필요)
./gradlew :androidApp:assembleDebug
python3 scripts/collect_rust_android.py

# macOS (Xcode + Metal Toolchain 필요)
python3 scripts/package_rust_mac.py
```

Windows는 `scripts/run_windows.cmd`를 더블클릭하거나 PowerShell에서 실행합니다.
기본 빌드는 테스트를 실행하지 않으며 `-Test`로 선택합니다. [Windows 안내](docs/WINDOWS.md)를 따르세요.

| 산출물 | 경로 |
|---|---|
| Android Debug | `androidApp/build/outputs/apk/debug/androidApp-debug.apk` |
| Android 전달용 복사본 | `artifacts/rust-candidate/android/dgl-rust-debug.apk` |
| macOS | `artifacts/rust-candidate/mac/dgl Rust Candidate.app` |
| Windows ZIP | `artifacts/rust-candidate/windows/dgl-rust-windows-x64.zip` |

앱 아이콘은 Android adaptive·단색, macOS ICNS, Windows 실행 파일 리소스로 포함됩니다.
원본과 재생성 절차는 [앱 아이콘 안내](assets/app-icon/README.md)에 있습니다. 일반 빌드는 변환 도구를 요구하지 않습니다.

## 문서

- [문서 목록](docs/README.md) · [사용 안내](docs/USAGE.md) · [개발 안내](docs/DEVELOPMENT.md)
- [현재 구조](docs/ARCHITECTURE.md) · [스크립트 안내](scripts/README.md)
- [검증 범위](docs/VALIDATION.md) · [남은 작업](TASKS.md) · [보안](docs/SECURITY.md)
- [과거 결정·작업 요약](docs/HISTORY.md) · [제3자 고지](THIRD_PARTY_NOTICES.md)
