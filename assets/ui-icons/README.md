# UI 아이콘

DGL의 모바일 Compose와 Rust GPUI에서 사용하는 공통 도형 원본입니다.
직접 작성한 SVG를 24×24 좌표계, 선 두께 1.8, 둥근 선 끝·모서리, 채움 없음으로 유지합니다.
색은 SVG의 검은 선을 마스크로 사용해 화면의 전경·강조 색으로 지정합니다.
앱 아이콘·CLI 브랜드 로고와 Material 체크박스·스위치는 별도로 유지합니다.

SVG는 루트의 공통 속성과 `path d`만 사용합니다. 글꼴·텍스트·외부 리소스는 사용하지 않습니다.
추가·수정 후 저장소 루트에서 실행합니다.

```sh
python3 scripts/generate_ui_icons.py
python3 scripts/generate_ui_icons.py --check
```

생성물은 `app/src/jvmSharedMain/kotlin/kim/jujin/dglc/ui/UiIcons.kt`와
`desktop/src/ui_icons.rs`입니다. 생성물을 직접 편집하지 않습니다.
일반 빌드는 저장된 생성물을 사용하며 Rust는 `include_bytes!`로 SVG를 포함합니다.
Windows·macOS 배포 시 별도 SVG 파일이 필요하지 않습니다.

모바일 기본 크기는 20dp, 작은 입력 도구는 18dp, 데스크톱은 16px입니다.
버튼의 기존 영역과 입력·포커스 동작은 호출부에서 유지합니다.
아이콘 전용 버튼에는 번역된 접근성 설명과 데스크톱 툴팁을 제공합니다.
에이전트 상태는 번역 문구가 아닌 사건 종류로 아이콘을 고릅니다.
