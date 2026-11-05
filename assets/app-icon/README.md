# DaGaLi 앱 아이콘

심볼은 **재접속 고리 터미널**입니다. 아이보리 터미널 창을 세로 구분선으로 나눠 tmux 두 pane을 만들고,
오른쪽 위 모서리를 끊은 뒤 그 자리를 라임 시계 방향 원호와 화살촉이 다시 잇습니다.
왼쪽 pane의 `>_` 프롬프트는 터미널, 오른쪽 pane의 블록 커서는 이어서 재개된 세션을 나타냅니다.
끊겼다 다시 이어지는 창 테두리가 SSH 재접속과 tmux 재부착, 즉 “Reconnect less. Resume more.”를 표현합니다.
문자 라벨·AI 반짝이·원근·입체 음영·광택은 사용하지 않습니다.

도형은 아이보리 `#F3EBDD`, 라임 `#C7F43B` 두 색만 칠하고 pane 안쪽은 투명하게 비워 배경이 보이게 합니다.
배경은 위쪽 중앙(변의 50%, 36%)의 `#232B38`에서 반경 78% 지점의 `#0A0D12`로 어두워지는 은은한 방사형 그라데이션입니다.
OS가 테두리를 처리하지 않는 Windows·MSIX·대시보드 타일과 macOS 타일은 바깥 1px급 검정 헤어라인(알파 45%)과
위가 밝고 아래로 옅어지는 안쪽 흰색 림(알파 16%→5%)으로 가장자리를 정리합니다.
32px 이하에서는 두 줄 대신 1px 중간톤 림 `#3A4352` 하나만 씁니다. 앱 ID·버전과 독립적인 그래픽 리소스입니다.

## 원본과 출력

| 파일 | 역할 |
|---|---|
| [source.png](source.png) | 공통 도형에서 Pillow로 생성한 투명 1024×1024 RGBA 원본 |
| [prompt.txt](prompt.txt) | 확정한 디자인 명세. 이미지 생성 서비스는 사용하지 않음 |
| [master.png](master.png) | 플랫폼 변환용 투명 1024×1024 PNG, 현재 원본과 동일 |
| [icon.png](icon.png) | 여백 없이 macOS와 같은 비율(196/896)로 모서리를 둥글린 그라데이션·테두리 타일의 1024×1024 PNG, Windows·로컬 대시보드용 |
| [macos.png](macos.png) | 투명 여백 안에 같은 그라데이션·테두리 타일(896px)을 둔 macOS용 1024×1024 PNG |
| [monochrome.json](monochrome.json) | 컬러·단색이 공유하는 M/L/C/Z 도형. `contours`는 끊긴 창 테두리·구분선과 왼쪽 pane 구멍, `details`는 재접속 원호·화살촉·`>`·`_`·블록 커서 |
| [preview-sizes.png](preview-sizes.png) | 어두운·밝은 배경 위 128·48·32·24·16px 실제 크기와 작은 크기의 최근접 확대 |
| [preview-android.png](preview-android.png) | Android 원형·둥근 사각형과 단색 테마의 정적 미리보기 |

## 재생성

일반 빌드는 저장소에 포함된 PNG·ICNS·ICO·Android XML·Compose 벡터만 사용합니다.
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
첫 윤곽은 아이보리 창 테두리, 나머지 윤곽은 투명 pane 구멍, `details`는 라임으로 채웁니다.
단색 버전에서는 같은 pane을 뚫고 원호·화살촉·프롬프트·커서를 전경색으로 채웁니다.
오른쪽 pane은 끊긴 모서리로 바깥과 이어지므로 별도 구멍 윤곽이 없습니다.
디자인을 바꾸면 `prompt.txt`도 함께 갱신한 뒤 재생성합니다.
`preview-sizes.png`의 128·48·32·24·16px에서 두 pane·재접속 화살표·`>_`와 어두운/밝은 배경 위 타일 테두리를,
`preview-android.png`의 원형·둥근 사각형·단색에서 화살촉 잘림과 세부 소실 여부를 확인합니다.

## 플랫폼 연결

- 모바일 상단: `app/src/jvmSharedMain/kotlin/kim/jujin/dglc/ui/AppIcon.kt`의 컬러 Compose 벡터를
  같은 공통 도형에서 생성합니다. 좁은 화면과 대화면 탐색기 왼쪽 상단에 24dp로 표시합니다.
  설정 맨 아래 `About app`에서도 같은 벡터를 사용합니다. 데스크톱 정보 화면은 `icon.png`를 바이너리에 포함합니다.
- Android: `androidApp/src/main/res/drawable-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/ic_launcher_foreground.png`.
  컬러·단색 모두 108dp 레이어의 중심 64dp 원 안에 같은 크기와 위치로 배치합니다.
  컬러 전경은 원본 alpha를 보존합니다.
  단색은 `drawable/ic_launcher_monochrome.xml`, 연결은 `mipmap-anydpi/ic_launcher.xml`입니다.
  배경은 생성되는 `drawable/ic_launcher_background.xml` 방사형 그라데이션 벡터이며, 보이는 중앙 72dp를 타일과 같은
  중심·반경 비율로 채웁니다. Android는 OS가 모양을 잘라 주므로 타일 테두리를 넣지 않습니다. 앱의 최소 API는 34입니다.
  알림 전용 `drawable/ic_notification.xml`은 같은 도형을 24dp 영역 안의 20dp 크기로 맞춘
  흰색·투명 벡터입니다. adaptive launcher 여백과 배경을 사용하지 않습니다.
  연결 유지·에이전트 알림의 `setSmallIcon`에 연결하며 재생성·`--check`에 포함합니다.
- macOS: `desktop/resources/dgl.icns`에 16·32·64·128·256·512·1024px 및 Retina 항목을 넣습니다.
  패키저가 `Contents/Resources/dgl.icns`를 복사하고 `CFBundleIconFile`을 설정한 뒤
  기존 로컬 ad-hoc 서명·검증을 수행합니다.
- Windows: `desktop/resources/windows/dgl.ico`에 16·24·32·48·64·128·256px를 넣습니다. 각 항목은 1024px 축소가 아니라
  해당 크기로 직접 렌더해 작은 크기에서도 테두리가 픽셀에 맞게 선명합니다.
  `desktop/build.rs`는 Windows target에서만 `app.rc`를 컴파일하고 실행 파일의 ID `1`에 연결합니다.
  GPUI 0.2.2의 기존 `LoadImageW`·창 클래스 아이콘 경로를 사용합니다.
  Windows SDK `rc.exe`는 기존 필수 도구이며 별도 변환기는 일반 빌드에 필요하지 않습니다.
- Windows MSIX: `desktop/resources/windows/msix/Assets/`의 `Square44x44Logo`(44px)·`Square150x150Logo`(150px)·
  `StoreLogo`(50px)는 ICO와 같은 둥근 타일을 배율 100 크기로 직접 렌더한 PNG입니다. `AppxManifest.xml`이 참조하며
  재생성·`--check`에 포함합니다.

Android 레이어 규격은 [공식 adaptive icon 안내](https://developer.android.com/develop/ui/compose/system/icon_design_adaptive),
Windows 리소스 선언은 [Microsoft ICON resource](https://learn.microsoft.com/en-us/windows/win32/menurc/icon-resource)를 따릅니다.

검사 범위와 미수행 플랫폼 인수는 [검증 안내](../../docs/VALIDATION.md)를 참고하세요.
