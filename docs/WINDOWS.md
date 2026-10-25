# Windows Rust 데스크톱 빌드·테스트

Windows **빌드 환경은 유지하며 실제 Windows 실행·실기 검증은 별도로 수행**한다.
현재 대상은 네이티브 Windows 10/11 x64, Rust GPUI 앱이다. 이전 JVM/MSI/EXE
설치물 작업은 사용하지 않는다. 아래 스크립트와 수동 CI를 추가했으며, 이 Mac 작업에서
Windows 네이티브 빌드·DPAPI 실행·GUI 인수를 통과했다고 표시하지 않는다.

하단 `…`의 Claude Code·Codex 실행 메뉴도 공통 GPUI 구현에 포함된다.
아이콘은 실행 파일에 내장되며 ZIP에는 `simple-icons-LICENSE.md`가 함께 포함된다.

설정의 Google Drive·iCloud Drive 동기화는 설치된 Drive 앱의 로컬 폴더를 선택해
`dgl-sync.json`을 읽고 쓰는 공통 파일 구현이다. OAuth·클라우드 SDK·추가 빌드 도구는
필요하지 않다. Windows 실제 Drive 폴더 연동은 미검증이며 [사용 방법](CLOUD_SYNC.md)을 따른다.

## 1. 빌드 환경 준비

1. Git for Windows, Python 3.9 이상(`python.exe`가 PATH에 있어야 함),
   [Rustup](https://www.rust-lang.org/tools/install)을 설치한다. Rust는 MSVC 호스트를 선택한다.
2. Visual Studio 2022 Build Tools에서 **Desktop development with C++**를 설치한다.
   Visual Studio Installer에서 [`desktop/windows.vsconfig`](../desktop/windows.vsconfig)를
   가져오면 x64 MSVC, CMake, Windows 11 SDK 10.0.26100 및 Spectre 라이브러리를 선택할 수 있다.
   Windows SDK의 `rc.exe`와 **`fxc.exe`**가 필요하다.
3. 저장소를 `C:\src\dgl`처럼 짧은 경로에 clone하고 `main`을 사용한다.
   Windows 데스크톱만 빌드할 때 JDK, Android SDK/NDK, Node.js는 필요하지 않다.

스크립트는 Visual Studio 개발 환경을 x64로 초기화하고 Rust **1.98.1**과
`x86_64-pc-windows-msvc` target을 설치한다. Rust의 전역 기본 버전은 변경하지 않는다.
VS·Python·Rustup 자체는 자동 설치하지 않으며 누락된 도구를 명시하고 중단한다.
WSL은 Linux 호스트이므로 이 Windows 패키지 빌드에는 사용하지 않는다.

빌드 전 도구만 확인하려면 Rust 1.98.1을 설치한 뒤:

```powershell
rustup toolchain install 1.98.1-x86_64-pc-windows-msvc --profile minimal --target x86_64-pc-windows-msvc
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\build_windows.ps1 -CheckOnly
```

`ExecutionPolicy Bypass`는 해당 PowerShell 프로세스에만 적용된다.
도구 구성은 [Zed의 Windows 개발 환경 안내](https://zed.dev/docs/development/windows)를 참고했고,
셰이더 경로는 이 저장소에 고정된 **GPUI 0.2.2의 build.rs**를 기준으로 한다.
현재 Zed의 다른 버전·그래픽 백엔드 요구사항을 그대로 적용하지 않는다.

## 2. 빌드와 산출물

Windows 작업 메뉴를 열려면 저장소 루트에서 다음 짧은 명령을 사용한다.
탐색기에서 `scripts\run_windows.cmd`를 더블클릭해도 된다.
현재 작업 폴더와 관계없이 동작하며 빌드 실패 시 앱을 실행하지 않는다.

```powershell
.\scripts\run_windows.cmd
```

메뉴: 1 빌드·실행, 2 빌드·ZIP만 생성, 3 빌드·테스트·실행,
4 전체 빌드 도구·업데이트 확인, 5 빌드 도구 설치·업데이트, 0 종료.
선택 작업이 실패해도 결과를 표시하고 메뉴로 돌아오므로 더블클릭한 창이 바로 닫히지 않는다.
메뉴 없이 바로 빌드·실행하려면 `.\scripts\run_windows.cmd -Run`,
테스트도 실행하려면 `.\scripts\run_windows.cmd -Test`를 사용한다.

도구 확인은 Git·Python·Rustup·VS 2022 Build Tools의 WinGet 업데이트 목록,
Rust 업데이트 정보, 로컬 MSVC·CMake·SDK(rc/fxc)·Python 버전·고정 Rust 도구와 target을
모두 확인한다. 하나가 실패해도 독립된 나머지 항목을 확인하고 마지막에 문제를 모아 표시한다.
WinGet의 업데이트 목록에 나타나지 않는 도구도 있으므로 로컬 필수 도구 검사와 함께 판단한다.
필수 도구에 문제가 있으면 전체 검사를 끝낸 뒤 빌드는 시작하지 않는다.

설치·업데이트 선택은 WinGet으로 위 네 패키지를 각각 설치/업데이트 시도한다.
VS에는 `desktop/windows.vsconfig`를 전달하며 Rust는 프로젝트 고정 버전 1.98.1과
MSVC target을 설치한다. 기존 VS에 빠진 구성 요소는 Visual Studio Installer에서
vsconfig를 가져와 추가해야 할 수 있다. 설치 중 관리자 권한 요청이 나올 수 있다.
WinGet이 없으면 Microsoft Store의 App Installer를 설치/업데이트하거나 도구를 수동 설치한다.
설치 실패도 나머지 항목 처리를 막지 않으며 마지막에 전체 로컬 검사를 다시 수행한다.
PATH 반영이나 재부팅이 필요하면 터미널 또는 Windows를 재시작하고 메뉴 4로 재확인한다.
설치는 메뉴 5에서만 수행하며 메뉴 4는 도구를 설치/업데이트하지 않는다.

저장소 루트에서:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\build_windows.ps1
```

기본 동작은 Release 빌드와 ZIP 생성이다. 앱 실행·설치·테스트를 자동 수행하지 않는다.
GPUI 0.2.2는 Release 빌드에서 `fxc.exe`로 HLSL 셰이더를 컴파일해 포함한다.
Debug 실행은 Cargo registry의 셰이더 소스를 참조하므로 전달용 ZIP은 Release만 제공한다.
MSVC CRT 정적 링크 플래그는 이 실행 파일 빌드에만 적용한다.

| 산출물 | 경로 |
|---|---|
| 실행 파일 | `target/x86_64-pc-windows-msvc/release/dgl-desktop.exe` |
| 전달용 ZIP | `artifacts/rust-candidate/windows/dgl-rust-windows-x64.zip` |
| 빌드 정보 | `artifacts/rust-candidate/windows/build.json` |
| ZIP SHA-256 | `artifacts/rust-candidate/windows/SHA256SUMS` |

`CARGO_TARGET_DIR`를 지정하면 실행 파일 경로는 Cargo metadata에서 가져온다.
ZIP에는 실행 파일, GPUI 고지, Rust 의존성 라이선스 원문·목록, 내부 파일 해시와
실행 안내가 들어간다. x64 PE 헤더를 검사한 뒤 패키지를 만든다.
**미서명 개발용 후보**이며 MSI 설치 프로그램이나 정식 서명 배포가 아니다.
빌드 성공과 GUI·그래픽 드라이버·보안 저장 검증 성공은 구분한다.

ZIP을 모두 풀고 `dgl-rust-windows-x64\dgl-desktop.exe`를 실행한다.
GPU 경로는 GPUI 0.2.2의 Direct3D 11이다. 설치·실행 성공 여부는 Windows에서 확인한다.

## 3. 수동 CI 빌드

변경이 원격 `main`에 반영된 후 GitHub Actions에서
**Build Windows Rust candidate → Run workflow**를 실행한다.
`windows-2022` runner가 같은 PowerShell 스크립트로 빌드하고
`windows-rust-candidate` artifact에 ZIP·빌드 정보·해시를 올린다.
수동 실행 전용이며 일반 push/PR의 기존 Linux 기본 검증은 그대로다.
이 workflow는 앱 실행·DPAPI 테스트·GUI 검증을 하지 않는다.

## 4. 사용자가 Windows에서 실행할 검사

빌드 후 Rust 코어·UniFFI·데스크톱 단위 테스트까지 실행하려면:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\build_windows.ps1 -Test
```

`-Test`는 선택 옵션이다. Windows DPAPI의 저장/읽기/덮어쓰기/삭제,
평문 미저장, 암호문 변조·다른 항목으로 바꿔치기 거부, 저장 실패 처리를 포함한다.
실제 SSH/tmux 통합 테스트는 기본 Cargo 검사에서 ignored이며 성공으로 계산하지 않는다.
격리 서버 통합 테스트는 [후보 기록](migration/CANDIDATE.md)의 별도 명령을 따른다.
기존 JVM `WindowsSecurityTest` 결과는 새 Rust 저장소 검증을 대신하지 않는다.

실기 확인 항목:

- 앱 창 열기·닫기, 배율/폰트, 한글·일본어 IME, 복사·붙여넣기.
- 최초 호스트키 확인, 비밀번호/키 인증, 저장 후 앱 재시작, 키 변경 시 차단.
- 개인키 파일 선택창의 `.ssh`/사용자 폴더 시작, 폴더·확장자 없는 파일·한글/공백 경로,
  취소 후 키 유지, 암호화 키·오류 수정 후 저장. [상세 검사](HOST_AUTH.md#windows-실기-확인-미실행).
- tmux 세션·창·패널 관리, 연결 종료 후 같은 원격 작업으로 복귀, 연결 복구.
- 다른 Windows 사용자에서 암호문 복호화 거부, 실제 프로필/DPAPI 키 손실 처리.

## 5. 저장 영역과 키보드

설정은 `%LOCALAPPDATA%\dgl-rust-candidate-v1\settings.json`, 자격 증명은 같은 폴더의
`credentials\<SHA-256>.sealed`에 저장한다. `DGL_CANDIDATE_DIR`로 둘 다 옮길 수 있다.
기존 JVM 앱 데이터를 자동 이전하거나 덮어쓰지 않는다.

보호는 [현재 사용자 범위 DPAPI](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)를
사용하며 MACHINE scope와 평문 fallback은 없다. 참조별 entropy로 항목 바꿔치기를 거부한다.

| 동작 | Windows 키 |
|---|---|
| 입력 필드 전체 선택·복사·붙여넣기·잘라내기 | `Ctrl+A/C/V/X` |
| 터미널 복사·붙여넣기 | `Ctrl+Shift+C/V` |
| 원격 인터럽트 | `Ctrl+C` |
| 호스트 추가 / 세션 생성 | `Ctrl+Shift+N` / `Ctrl+Shift+S` |
| 연결 종료 / 연결 전환 | `Ctrl+Shift+W` / `Ctrl+Shift+F1–F9` |
| 새로고침 / 설정 / 앱 종료 | `Ctrl+Shift+R` / `Ctrl+Shift+P` / `Ctrl+Shift+Q` |

## 6. 에이전트 알림

탭·패널의 프로세스 식별과 선택적 공식 훅 연동은 [공통 안내](AGENT_STATUS.md)를 따른다.
Windows 알림은 WinRT 네이티브 토스트이며 PowerShell이나 다른 앱의 알림 ID를 사용하지 않는다.
ZIP 배포와 기존 빌드 진입점은 유지한다. 추가 빌드 도구는 없으며 `windows` crate의
알림·COM·레지스트리 API를 사용한다.

설정에서 **OS 알림**을 켜면 현재 사용자에게 아래 항목을 등록한다. 관리자 권한은 필요하지 않다.
이후 OS 알림이 켜진 앱 시작에도 등록 경로를 갱신하므로 ZIP 폴더를 옮긴 뒤에는 앱을 직접 실행한다.

- AUMID: `kim.jujin.dglc`
- 바로가기: `%APPDATA%\Microsoft\Windows\Start Menu\Programs\dgl Rust Candidate.lnk`
- `HKCU\Software\Classes\AppUserModelId\kim.jujin.dglc`
- COM 활성화 CLSID: `{5FD514F1-B83F-4F6D-A85B-156AD0E79E33}`
- `HKCU\Software\Classes\CLSID\{5FD514F1-B83F-4F6D-A85B-156AD0E79E33}\LocalServer32`
  값은 현재 실행 파일과 `--agent-toast` 인자이다.

클릭은 사건 ID로 앱의 기존 연결·패널을 찾는다. 앱이 종료되어 최근 사건이 없거나 패널이 사라졌으면
연결 종료 안내를 표시하며 자동 재접속·원격 입력·승인을 하지 않는다. OS 알림을 꺼도 기존 사용자
등록은 남는다. 앱을 완전히 제거할 때 위 **dgl 전용** 바로가기와 두 레지스트리 하위 트리만 제거할 수 있다.
사용자 설정·자격 증명과 다른 앱의 등록은 별개다.

이번 Mac 작업에서는 Windows GNU 대상 토스트 어댑터의 독립 타입 검사와 패키저 단위 검사를 수행했다.
MSVC 전체 빌드와 Windows에서의 COM 활성화·알림 표시 검증을 대체하지 않는다. 사용자 실기 항목:

1. ZIP을 풀어 실행하고 OS 알림을 켠 뒤 Windows 알림 설정에 dgl 자체 이름이 나타나는지 확인한다.
2. 다른 탭·패널 또는 다른 앱을 보는 동안 공식 훅 사건을 발생시켜 알림·클릭 이동을 확인한다.
3. 현재 보이는 패널의 알림 생략, 집중 모드/권한 거부 시 앱 내 사건 보존을 확인한다.
4. 패널 종료·앱 종료 뒤 기존 토스트 클릭 시 종료 안내와 재접속/입력 미전송을 확인한다.
