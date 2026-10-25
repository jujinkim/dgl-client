# 테마·강조 색상·창 스타일

설정의 테마는 **시스템 / 밝게 / 어둡게**로 선택합니다. 강조 색상은 별도로 선택합니다.

| 선택 | 동작 |
|---|---|
| 시스템 (기본값) | 현재 OS 강조 색상을 사용합니다. 조회 실패 시 현재 밝기 테마의 DGL 기본색으로 대체합니다. |
| DGL 기본 | 밝은 테마는 파랑 `#2468C9`, 어두운 테마는 `#75AAFF`입니다. |
| 사용자 지정 | 파랑·보라·분홍·빨강·주황·노랑·초록·청록 프리셋 또는 `#RRGGBB` 입력을 사용합니다. |

프리셋은 선택 즉시 적용·저장합니다. HEX는 견본을 확인하고 **적용** 또는 Enter로 저장합니다.
잘못된 값은 입력란에 오류를 표시하며 기존 색상은 유지합니다. 시스템이나 DGL 기본으로 바꿔도
마지막 사용자 지정 색상을 보존합니다. 편집 중 초안은 모드 전환으로 지우지 않으며, 재실행 시 저장된 값을 복원합니다.
설정 대화상자에서 입력한 키는 터미널로 보내지 않습니다.

탭·버튼·입력 포커스·앱 선택 영역에 강조색을 적용합니다. 강조 배경 위 글자는 대비에 따라
검정 또는 흰색을 선택하고, 강조색 글자·포커스 선은 배경에 맞춰 밝기를 조정합니다.
중립 배경과 터미널 내부 ANSI 색상·선택 팔레트는 기존 값을 유지합니다.

## 플랫폼 동작

| 플랫폼 | 창·시스템 색상 |
|---|---|
| Windows 11 | DWM 제목 표시줄·외곽선·제목 글자색을 앱 테마에 맞춥니다. 기본 창 버튼·시스템 메뉴·Snap·드래그·크기 변경을 유지합니다. 고대비 모드에서는 강제 창 색상을 해제합니다. |
| Windows 10 | 지원되지 않는 DWM 속성은 무시하고 기본 프레임을 유지합니다. UISettings 강조 색상은 사용합니다. |
| macOS | AppKit의 투명 제목 표시줄과 앱 배경색을 사용합니다. 기본 제목·신호등 버튼·위치·동작을 유지하며, AppKit이 실제 제목 표시줄 높이를 콘텐츠 영역에서 확보합니다. 그림자·윤곽선은 OS가 그립니다. |
| Android 14–16 | 상태·탐색 영역 뒤에 앱 배경을 그리고 아이콘 밝기를 맞춥니다. API 35 이상은 창 모드 제목 표시줄을 투명하게 해 앱 배경을 보이며, 기본 버튼과 안전 영역을 유지합니다. Material 동적 팔레트에서는 강조 역할만 가져옵니다. |

macOS는 `NSColor.controlAccentColor`를 현재 테마에서 해석해 sRGB로 변환합니다.
Windows는 `UISettings.GetColorValue(Accent)`와 색상·고대비 변경 이벤트를 사용합니다.
Android는 동적 팔레트를 구성 변경·배경화면 변경·앱 복귀 시 갱신합니다.
데스크톱은 알림·테마 변경·앱 복귀 시 캐시를 갱신하며, 렌더링에서 OS 색상을 조회하지 않습니다.
창 장식 API 실패는 앱 실행을 막지 않습니다.

## 저장·동기화

`preferences.accent`는 `{"mode":"system|default|custom","customColor":"#RRGGBB"}` 형식입니다.
`customColor`는 선택 항목이며 저장 값은 대문자 HEX로 정규화합니다. 항목이 없거나 잘못된 값이면
색상 설정만 기본값으로 읽고 호스트와 나머지 설정은 유지합니다.

선택 방식과 마지막 사용자 지정 색상을 한 항목으로 [동기화](CLOUD_SYNC.md)합니다.
OS에서 조회한 실제 색상은 저장·전송하지 않으므로 같은 ‘시스템’ 설정도 기기마다 다르게 보일 수 있습니다.
Linux·iOS/iPadOS 신규 앱은 이 기능의 범위에 포함하지 않습니다.

공식 API 근거:
[Windows DWM](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute),
[UISettings](https://learn.microsoft.com/en-us/uwp/api/windows.ui.viewmanagement.uisettings),
[AppKit 제목 표시줄](https://developer.apple.com/documentation/appkit/nswindow/titlebarappearstransparent),
[AppKit 강조 색상](https://developer.apple.com/documentation/appkit/nscolor/controlaccentcolor),
[Android 창 모드](https://developer.android.com/develop/adaptive-apps/guides/support-desktop-windowing).
