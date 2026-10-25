# DGL 앱 아이콘

Enter 한 번으로 그럴듯하게 포장된 AI 찌꺼기가 찍혀 나오는 모습입니다.
아이보리 `↵` 키캡과 녹아내린 형광 라임 앱 타일, 과장된 반짝이 하나를 사용합니다.
기본 배경은 `#11151B`입니다. 앱 ID·버전과 독립적인 그래픽 리소스입니다.

## 원본과 출력

| 파일 | 역할 |
|---|---|
| [source.png](source.png) | built-in imagegen으로 생성한 투명 RGBA 원본, 1254×1254, 생성 파일 그대로 보존 |
| [prompt.txt](prompt.txt) | 최종 생성 프롬프트. 요청 크기는 1024px이며 실제 도구 출력은 위 크기 |
| [master.png](master.png) | 원본을 리샘플한 투명 1024×1024 PNG |
| [icon.png](icon.png) | 짙은 배경을 합성한 1024×1024 PNG, Windows·로컬 대시보드용 |
| [macos.png](macos.png) | 투명 여백과 둥근 짙은 타일을 적용한 macOS용 1024×1024 PNG |
| [monochrome.json](monochrome.json) | 같은 키캡·화살표·슬라임·반짝이를 단순화해 그린 단색 벡터 윤곽. M/L/C/Z 명령, 첫 윤곽은 채움·나머지는 구멍 |
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
생성 결과와 저장된 파일을 비교하며, 누락·차이가 있으면 실패합니다.
이미지 생성 자체는 비결정적이므로 재생성 스크립트가 호출하지 않습니다.
새 원본을 만들 때는 `source.png`·`prompt.txt`를 함께 갱신한 뒤 변환합니다.

## 플랫폼 연결

- Android: `androidApp/src/main/res/drawable-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/ic_launcher_foreground.png`.
  108dp 전경의 의미 있는 픽셀을 중심 64dp 원 안에 배치하고 원본 alpha를 보존합니다.
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
