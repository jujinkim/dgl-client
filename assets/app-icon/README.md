# DGL 앱 아이콘

위에서 내려다본 플랫 엔터키 안에 두 개의 터미널 pane을 담습니다.
키캡과 큰 `↵`는 딸깍, 분할선은 tmux, 두 pane을 잇는 화살표는 SSH 연결,
우상단의 작은 4점 반짝이는 AI를 나타냅니다. 문자 라벨·원근·입체 음영·광택은 사용하지 않습니다.

배경을 포함해 차콜 `#11151B`, 아이보리 `#F3EBDD`, 라임 `#C7F43B`의 3색만 채색합니다.
경계 안티앨리어싱과 투명 여백은 허용하며 그라데이션은 사용하지 않습니다.
앱 ID·버전과 독립적인 그래픽 리소스입니다.

## 원본과 출력

| 파일 | 역할 |
|---|---|
| [source.png](source.png) | 공통 도형에서 Pillow로 생성한 투명 1024×1024 RGBA 원본 |
| [prompt.txt](prompt.txt) | 확정한 디자인 명세. 이미지 생성 서비스는 사용하지 않음 |
| [master.png](master.png) | 플랫폼 변환용 투명 1024×1024 PNG, 현재 원본과 동일 |
| [icon.png](icon.png) | 짙은 배경을 합성한 1024×1024 PNG, Windows·로컬 대시보드용 |
| [macos.png](macos.png) | 투명 여백과 둥근 짙은 타일을 적용한 macOS용 1024×1024 PNG |
| [monochrome.json](monochrome.json) | 컬러·단색이 공유하는 M/L/C/Z 도형. `contours`는 키캡과 두 pane, `details`는 화살표·반짝이 |
| [preview-sizes.png](preview-sizes.png) | 128·48·32·16px 실제 크기와 작은 크기의 최근접 확대 |
| [preview-android.png](preview-android.png) | Android 원형·둥근 사각형과 단색 테마의 정적 미리보기 |

## 재생성

일반 빌드는 저장소에 포함된 PNG·ICNS·ICO·Android XML만 사용합니다.
이미지 생성 호출이나 Pillow 설치가 필요하지 않습니다. 그래픽을 갱신할 때만:

```sh
python3 -m venv .tools/app-icon-venv
.tools/app-icon-venv/bin/python -m pip install Pillow==11.3.0
.tools/app-icon-venv/bin/python scripts/generate_app_icons.py
.tools/app-icon-venv/bin/python scripts/generate_app_icons.py --check
```

Python 3.9 이상을 사용합니다. Windows에서는 가상환경 실행 파일이
`.tools\app-icon-venv\Scripts\python.exe`입니다. `--check`는 파일을 쓰지 않고
생성 결과와 저장된 파일을 비교하며, 원본 PNG를 포함해 누락·차이가 있으면 실패합니다.
도형은 `monochrome.json`, 팔레트는 `scripts/generate_app_icons.py`에서 수정합니다.
첫 윤곽은 아이보리 키캡, 나머지 윤곽은 차콜 pane, `details`는 라임으로 채웁니다.
단색 버전에서는 같은 pane을 뚫고 화살표·반짝이를 전경색으로 채웁니다.
디자인을 바꾸면 `prompt.txt`도 함께 갱신한 뒤 재생성합니다.

## 플랫폼 연결

- Android: `androidApp/src/main/res/drawable-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/ic_launcher_foreground.png`.
  컬러·단색 모두 108dp 레이어의 중심 64dp 원 안에 같은 크기와 위치로 배치합니다.
  컬러 전경은 원본 alpha를 보존합니다.
  단색은 `drawable/ic_launcher_monochrome.xml`, 연결은 `mipmap-anydpi/ic_launcher.xml`입니다.
  배경색은 기존 `values/colors.xml`을 사용합니다. 앱의 최소 API는 34입니다.
- macOS: `desktop/resources/dgl.icns`에 16·32·64·128·256·512·1024px 및 Retina 항목을 넣습니다.
  패키저가 `Contents/Resources/dgl.icns`를 복사하고 `CFBundleIconFile`을 설정한 뒤
  기존 로컬 ad-hoc 서명·검증을 수행합니다.
- Windows: `desktop/resources/windows/dgl.ico`에 16·24·32·48·64·128·256px를 넣습니다.
  `desktop/build.rs`는 Windows target에서만 `app.rc`를 컴파일하고 실행 파일의 ID `1`에 연결합니다.
  GPUI 0.2.2의 기존 `LoadImageW`·창 클래스 아이콘 경로를 사용합니다.
  Windows SDK `rc.exe`는 기존 필수 도구이며 별도 변환기는 일반 빌드에 필요하지 않습니다.

Android 레이어 규격은 [공식 adaptive icon 안내](https://developer.android.com/develop/ui/compose/system/icon_design_adaptive),
Windows 리소스 선언은 [Microsoft ICON resource](https://learn.microsoft.com/en-us/windows/win32/menurc/icon-resource)를 따릅니다.

검사 범위와 미수행 플랫폼 인수는 [검증 안내](../../docs/VALIDATION.md)를 참고하세요.
