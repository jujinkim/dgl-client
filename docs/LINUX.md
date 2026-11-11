# Linux 앱·AppImage

배포 기준은 **Ubuntu 24.04 이상 x86_64, X11·Wayland**입니다. 배포 바이너리는 Ubuntu 24.04에서
빌드합니다. Vulkan을 지원하는 그래픽 드라이버, 시스템 폰트, 파일 선택용 XDG Desktop Portal이 필요합니다.
현재 Linux 네이티브 빌드·AppImage 실행·X11/Wayland 실기 인수는 아직 수행하지 않았습니다.

## Local과 SSH

Hosts 맨 위의 Local은 현재 사용자 권한으로 기본 tmux 서버의 세션을 조회합니다. SSH 서버·비밀번호가
필요하지 않으며 호스트 편집·삭제·정렬·동기화에 포함되지 않습니다. 앱 시작 시 자동 연결하거나 세션을
생성하지 않습니다. Local을 선택한 뒤 세션을 선택하거나 세션 만들기를 사용합니다.
지원 tmux는 3.3–3.7(접미 문자 포함)이며 `sudo apt install tmux`로 설치합니다.
설치 안내에는 명령 복사·공식 안내 열기·다시 확인이 있습니다. 앱은 설치를 자동 실행하지 않습니다.
연결 종료·앱 종료는 control client만 닫으며 tmux 세션과 작업은 계속 실행됩니다.
Local의 최근 세션·pane은 기기 전용 `local-session.json`에 저장합니다.

SSH 비밀번호·개인키·호스트키는 로그인 세션의 Secret Service(GNOME Keyring/KWallet)에 저장합니다.
서비스 부재·잠금·기타 오류는 각각 표시하며 평문 저장으로 전환하지 않습니다. 잠금은 데스크톱의
키링 관리 도구에서 해제하세요. Secret Service가 없어도 Local을 사용할 수 있습니다.
Local에서는 SSH 전용 에이전트 관측 helper를 실행하지 않습니다.

설정 경로는 `$XDG_DATA_HOME/dgl-rust-candidate-v1` 또는 `~/.local/share/dgl-rust-candidate-v1`이며
`DGL_CANDIDATE_DIR`로 바꿀 수 있습니다. 앱 단축키는 Ctrl+Shift+N(호스트), S(세션), W(연결 종료),
F1–F9(탭), R(새로고침), C/V(복사/붙여넣기)입니다. Ctrl+C 등은 터미널에 전달합니다.

## 빌드·패키징

Ubuntu 24.04 x86_64에서 Python 3.9 이상과 Rust 1.98.1을 준비합니다.

```sh
sudo apt install build-essential pkg-config cmake clang libclang-dev libssl-dev libasound2-dev libudev-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libxcb1-dev libxcb-randr0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxi-dev libx11-xcb-dev libfontconfig1-dev libfreetype-dev libvulkan-dev libdbus-1-dev desktop-file-utils squashfs-tools tmux
cargo test --locked --workspace
DGL_LOCAL_TMUX=/usr/bin/tmux cargo test --locked -p dgl-core --test local_integration -- --ignored
python3 scripts/package_rust_linux.py
python3 scripts/verify_app_versions.py --linux
```

수동 [Build Linux Rust candidate](../.github/workflows/linux-build.yml)도 같은 경로를 사용합니다.
산출물은 `artifacts/rust-candidate/linux/DaGaLi-<version>-x86_64.AppImage`, `build.json`, `SHA256SUMS`입니다.
Release 실행 파일, 아이콘, desktop entry, Rust·시스템 라이브러리 라이선스, 빌드 메타데이터를 포함합니다.
appimagetool 1.9.1과 type2-runtime 20251108은 URL·SHA-256을 고정하고 캐시도 매번 검증합니다.
패키징 검사는 AppImage 추출과 실행 파일 해시를 확인하며 GUI 실행 통과를 뜻하지 않습니다.
호스트의 glibc·C++ 런타임·그래픽 드라이버를 사용하고 tmux는 번들하지 않습니다.
AppImage의 라이브러리 환경은 Local tmux 자식 프로세스에서 제거합니다.

## 실행·짧은 실기 확인

```sh
cd artifacts/rust-candidate/linux
sha256sum -c SHA256SUMS
chmod +x DaGaLi-*-x86_64.AppImage
./DaGaLi-<version>-x86_64.AppImage
```

FUSE가 없는 환경은 AppImage의 `--appimage-extract-and-run` 또는 아래 추출 실행을 사용합니다.

```sh
./DaGaLi-<version>-x86_64.AppImage --appimage-extract
./squashfs-root/AppRun
```

X11와 Wayland 로그인 세션에서 각각 시작→Local 선택→격리된 테스트 세션 생성→짧은 입력→연결 종료를
한 번씩 확인합니다. 외부 터미널에서 세션이 살아 있는지 확인하고 다시 연결합니다. 완료 후 테스트 세션만
삭제합니다. 개인 기본 세션을 자동 검사에 사용하지 않습니다. SSH 지문·인증·재접속, 키링 오류 화면,
파일 선택·테마·폰트·툴팁·알림도 실제 배포 환경에서 별도로 인수합니다.

기준: [tmux 설치](https://github.com/tmux/tmux/wiki/Installing),
[AppImage 배포 권고](https://docs.appimage.org/reference/best-practices.html).
