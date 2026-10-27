# DaGaLi 앱 아이콘

dagali의 머리는 위에서 내려다본, 모서리가 둥근 **ㄱ자 엔터 키캡**입니다.
넓은 윗부분과 오른쪽 아래 돌출부로 윤곽을 만들고 왼쪽 아래는 비워 둡니다.
안쪽에는 차콜색 터미널 pane 두 개를 담으며 오른쪽 pane은 아래 돌출부까지 이어집니다.
왼쪽의 `>` 프롬프트와 오른쪽 블록 커서를 눈으로, 그 아래의 짧은 `_` 커서를 입으로 삼아
무심한 표정을 만듭니다. 고유한 머리 윤곽은 딸깍, pane 분할과 커서 얼굴은 터미널·tmux를 나타냅니다.
큰 리턴 화살표·AI 반짝이·문자 라벨·원근·입체 음영·광택은 사용하지 않습니다.

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
| [monochrome.json](monochrome.json) | 컬러·단색이 공유하는 M/L/C/Z 도형. `contours`는 ㄱ자 키캡과 두 pane, `details`는 프롬프트 눈·블록 눈·커서 입 |
| [preview-sizes.png](preview-sizes.png) | 128·48·32·16px 실제 크기와 작은 크기의 최근접 확대 |
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
첫 윤곽은 아이보리 키캡, 나머지 윤곽은 차콜 pane, `details`는 라임으로 채웁니다.
단색 버전에서는 같은 pane을 뚫고 두 눈과 입을 전경색으로 채웁니다.
디자인을 바꾸면 `prompt.txt`도 함께 갱신한 뒤 재생성합니다.
`preview-sizes.png`의 128·48·32·16px에서 ㄱ자 윤곽·열린 왼쪽 아래·표정의 가독성을,
`preview-android.png`의 원형·둥근 사각형·단색에서 잘림과 얼굴 소실 여부를 확인합니다.

## 플랫폼 연결

- 모바일 상단: `app/src/jvmSharedMain/kotlin/kim/jujin/dglc/ui/AppIcon.kt`의 컬러 Compose 벡터를
  같은 공통 도형에서 생성합니다. 좁은 화면과 대화면 탐색기 왼쪽 상단에 24dp로 표시합니다.
  설정 맨 아래 `About app`에서도 같은 벡터를 사용합니다. 데스크톱 정보 화면은 `icon.png`를 바이너리에 포함합니다.
- Android: `androidApp/src/main/res/drawable-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/ic_launcher_foreground.png`.
  컬러·단색 모두 108dp 레이어의 중심 64dp 원 안에 같은 크기와 위치로 배치합니다.
  컬러 전경은 원본 alpha를 보존합니다.
  단색은 `drawable/ic_launcher_monochrome.xml`, 연결은 `mipmap-anydpi/ic_launcher.xml`입니다.
  배경색은 기존 `values/colors.xml`을 사용합니다. 앱의 최소 API는 34입니다.
  알림 전용 `drawable/ic_notification.xml`은 같은 도형을 24dp 영역 안의 20dp 크기로 맞춘
  흰색·투명 벡터입니다. adaptive launcher 여백과 배경을 사용하지 않습니다.
  연결 유지·에이전트 알림의 `setSmallIcon`에 연결하며 재생성·`--check`에 포함합니다.
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
