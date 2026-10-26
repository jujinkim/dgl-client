# 과거 결정·작업 요약

현재 동작·검증 절차는 [문서 목록](README.md), 남은 작업은 [TASKS.md](../TASKS.md)를 기준으로 합니다.
이 문서는 과거 작업의 요약입니다. 과거 지시·완료 판정을 현재 규칙으로 적용하지 않습니다.
중복 계획·전체 로그·폐기된 명령은 Git에서 필요할 때 확인합니다.

## 2026-10-23: JVM MVP

KMP/Compose·SSHJ·JediTerm(데스크톱)·xterm.js(Android)로 첫 구현을 만들었습니다.
PTY 없는 tmux Control Mode, 호스트키 검증, 보호 저장, snapshot/live 순서와 입력 무재전송,
세션·창·pane 관리, 격리 SSH fixture와 공통 회귀 검사를 도입했습니다.

당시 tmux 3.3a·3.7c와 Android API34·35·36 에뮬레이터 검사를 수행했습니다.
[원격 CI 실행](https://github.com/jujinkim/dgl-client/actions/runs/39898373064)은
`899ad82b540a5b284c59630b0713a8950b09de4a`에서 Linux·tmux·Android 6개 job 성공을 기록했습니다.
이 결과는 현재 Rust 앱의 실기 인수를 대신하지 않습니다.
Windows JVM MSI/EXE·Temurin 소스 수집·두 사용자 DPAPI 검증 자동화는 작성했지만 Windows에서 실행하지 않았습니다.

의존성 371개 버전 OSV 0건, 라이선스·원문·Wrapper·비밀 검사 결과는 그 시점의 선택 그래프에 한정됩니다.
이후 코드·의존성의 현재 보안 결과로 간주하지 않습니다. 라이선스 원문과 해시 자료는 `licenses/`에 보존합니다.

## 2026-10-24: Rust 단일 실행 전환

공통 Rust 코어(russh), UniFFI Android 바인딩, GPUI/alacritty_terminal 데스크톱을 구현했습니다.
기존 JVM 앱 실행·설치물과 Android 엔진 선택을 제거했습니다. Kotlin 참조 엔진은 회귀 검사에만 남겼습니다.
공통 계약 fixture 120건으로 시작했고 현재 추가된 fixture는 원본 JSON에서 확인합니다.

성능 인수를 기능 이식의 선행 조건에서 분리했습니다. Android 기본 화면·키보드·SSH 흐름과
Mac 빌드·단위 검사를 진행했으나 실제 Mac GUI 조회 `cgWindowNotFound`, Keychain 쓰기 `-25308`로
끝난 시도는 미검증으로 기록했습니다. Windows Rust 빌드·ZIP·수동 CI는 유지하며 실기는 사용자 담당입니다.

API36/WebView 133.0.6943.137/software GPU의 첫 표시 프레임 진단은 정상 입력 8·지연 입력 8·
무변화 입력 1, 총 17개였습니다. WebView 수신 프레임 406개의 `surface_frame_trace_id=-1`,
compositor→Android surface 연결 부재, `misplaced_end_event=1` 때문에
**proven 0, unresolved 17, invalid_trace**로 남겼습니다. 콜백·상한 지연을 실제 표시 지연 통과로 쓰지 않습니다.
현재 도구 계약은 [PERFORMANCE.md](PERFORMANCE.md)에 있습니다.

## 2026-10-24–25: 기능 확장

데스크톱 탭 그룹·드래그 분할·배치 저장, tmux 패널 도구, 사용자 단축키,
CLI 실행·관측·알림, Drive 파일 동기화, 수동 재접속/정지, 모바일 텍스트 작성,
개인키 파일 선택과 호스트 오류 안내를 추가했습니다. AGY·Grok Build는 실험 지원입니다.

앱 식별자를 `kim.jujin.dglc`로 통일했습니다(`495c2ea`). 내부 Kotlin 패키지·저장 키는 유지했고
이전 Android 앱의 데이터 자동 이전은 제공하지 않습니다.
`5fa6b99`에서 선택 창의 모든 보이는 pane을 실제 tmux 배치로 표시하도록 바꿨습니다.
외부 창·pane 선택 동기화, 입력/렌더링 세대 분리, pane별 로컬 화면 유지와 창별 에이전트 집계를 적용했습니다.

당시 마지막 검사는 Rust 63 통과/6 ignore, Kotlin 45 통과/9 skip, 웹 8 통과, Python 100 통과,
Android 빌드·lint(오류 0/경고 12), API36 계측 11개, tmux 두 버전과 Mac ad-hoc 패키징을 기록했습니다.
tmux 3.3a의 공유 fixture에서는 간헐적인 세션 생성 `InvalidArgument`가 있어 각 검사에 새 fixture를 쓴
5개 개별 실행을 최종 근거로 삼았습니다. 초기 실패를 없던 것으로 처리하지 않습니다.
Mac 실제 GUI·Keychain, Windows 네이티브 빌드·DPAPI·GUI, Android 실물과 장시간 네트워크 전환은 미검증입니다.
사용자 지시에 따라 이후 기본 검증은 기능 검사·유닛 테스트이며 전체 API 계측을 자동 재개하지 않습니다.

## 2026-10-25: 문서·스크립트 정리

제품 소개를 SSH 재접속·tmux 재연결에 맞췄습니다.
슬로건은 **Reconnect less. Resume more.**, 랜딩 URL은 [ddal.ggak.life](https://ddal.ggak.life)입니다.
README는 소개·시작 안내, 상세 조작은 USAGE, 빌드·검사는 DEVELOPMENT, 남은 일은 TASKS로 분리했습니다.
`NEXT_WORK.md`, 과거 TASKS/VALIDATION 내용, 아키텍처 전환 계획·후보·기준선·기능 대조표,
초기 JVM ADR은 이 요약으로 합쳤습니다. 유효한 코어·성능 계약은 `docs/`로 옮겼습니다.

폐기된 JVM MSI/EXE 검사, Temurin 소스 수집, 두 사용자 JVM DPAPI harness와 관련 검증 코드를 제거했습니다.
현재 Windows Rust 패키저·단위 검사·빌드 메뉴·수동 CI와 공통 JVM 회귀 검사는 유지합니다.

당시 정리 검사는 Python 98개, Rust 63개(환경 ignore 6), Kotlin 45개(예상 skip 9), 웹 8개를 통과했습니다.
선택 runtime 121개 버전의 inventory·고지, 로컬 Markdown 링크 106개·구문·무결성·대시보드·비밀정보 검사도 수행했습니다.
JVM 54개·Android 15개 이름 목록과 선택 계측 결과 파서의 누락·중복·실패·skip 거절을 확인했으며,
앱 배포 파일 재생성·실제 SSH·Android 계측·Windows 네이티브 검사는 재실행하지 않았습니다.

## 2026-10-25: 입력 도구·텍스트 작성

하단 고정 입력 도구와 스크롤 단축키, 연결별 여러 줄 작성창·초안·IME·대상 세대 검사를 추가했습니다.
당시 검사는 Rust 데스크톱 48 통과/1 ignore, 코어·UniFFI 23 통과/5 ignore,
JVM 45 통과/예상 skip 9, Android Debug·lint와 결과 검증기를 통과했습니다.
실제 macOS GUI·Windows 네이티브·Android 계측은 수행하지 않았습니다.

## 2026-10-26: 강조 색상·동기화 v2

시스템·DGL 기본·사용자 HEX 강조 색상, 플랫폼 창 스타일과 설정 화면을 추가했습니다.
v1 동기화 데이터·캐시를 v2로 전환하고 데이터셋·이력과 강조 선택의 병합을 유지했습니다.
당시 Rust 코어·UniFFI 27 통과/5 ignore, 데스크톱 51 통과/1 ignore,
JVM 46 통과/예상 skip 9, Python 98·웹 8 통과, Android Debug·lint와 Mac ad-hoc 패키징을 확인했습니다.
Mac computer-use는 앱 식별자 연결 실패로 창을 관찰하지 못했으며 GUI·Keychain·OS 변경 인수는 남겼습니다.
Windows 네이티브·전체 Android 계측은 실행하지 않았습니다. 자세한 과거 검사 내용은 Git 이력에서 찾습니다.

## 2026-10-26: 앱 아이콘

투명 원본·프롬프트를 보존하고 Android 적응형·단색, macOS ICNS, Windows ICO 리소스를 연결했습니다.
당시 Android Debug·lint(오류 0, 기존 검토 경고 12), Mac ad-hoc 패키징·strict 서명 검사,
Python 98개·아이콘 재생성 일치·로컬 대시보드 스키마 검증을 통과했습니다.
전체 Rust 서식 검사는 기존 `password_shell.rs` 차이로 실패했고 변경 build.rs 서식은 통과했습니다.
Windows 네이티브 빌드·GUI와 macOS Dock/Finder·Android 실제 런처·계측은 실행하지 않았습니다.
원본·축소·마스크 미리보기와 재생성 절차는 [앱 아이콘 안내](../assets/app-icon/README.md)에 있습니다.

## 2026-10-26: 세션 목록·상태줄

복원 중 세션 행 위치·펼침·스크롤을 유지하고 알림·설정 버튼을 하단 상태줄에 고정했습니다.
당시 Rust 데스크톱 54·코어/UniFFI 27 통과, Keychain 1·SSH 5 ignore와 Python 98 통과를 기록했습니다.
Android 계측 APK는 빌드했으나 계측 실행과 Windows·macOS GUI 실기 인수는 남겼습니다.

## 2026-10-26: 호스트 순서·세션 탭 재사용

최초 연결 순서·기기별 드래그 정렬과 호스트/세션 ID별 탭 재사용을 추가했습니다.
당시 데스크톱 59 통과·대화형 Keychain 1 ignore, 대시보드 등록·문서 링크·원문 무결성 검사를 확인했습니다.
Android 빌드·실제 SSH/tmux·Windows/macOS 실기 인수는 수행하지 않았습니다.

## 2026-10-26: 언어·실시간 제목

데스크톱 제목을 DGL로 정리하고 한국어/영어·기기별 언어 선택과 세션·창의 실시간 pane 제목을 연결했습니다.
당시 코어/UniFFI 30·데스크톱 60·JVM 51·Python 98 통과, JVM 예상 skip 9·Rust ignore 6을 기록했습니다.
격리 tmux 3.3a·3.7c 제목 검사, Android Debug·lint·계측 컴파일, Mac ad-hoc 패키징을 확인했습니다.
Android 계측 실행·Windows/macOS 실기 인수는 수행하지 않았습니다. 상세는 해당 Git 이력에서 확인합니다.

## 2026-10-26: Android 연결 수명·IME

프로세스 공유 엔진·SSH 유지 서비스와 기본 켜짐 설정, 최근 앱/task 제거 시 로컬 정리,
입력 오류 분류·비차단 안내, IME 하단 정렬·최종 resize와 전환 시간 단축을 추가했습니다.
당시 Rust 코어/UniFFI 30·JVM 57·웹 12·Python 98 통과, 예상 JVM skip 9·기본 SSH ignore 5를 기록했습니다.
Android Debug·미서명 Release·lint·API36 단일 연결 수명 계측과 격리 tmux 3.7c 재접속 검사를 통과했습니다.
실제 손가락 swipe·화면 잠금·물리 기기 절전 정책과 Windows/macOS 실기 인수는 남겼습니다.

## 2026-10-26: Kotlin 패키지 정리

Kotlin/Java 제품·참조·검사를 `kim.jujin.dglc`로 통일하고 보호 저장 호환 식별자는 보존했습니다.
당시 Rust 코어/UniFFI 30·데스크톱 60·JVM 57·Python 98 통과, JVM 예상 skip 9·Rust ignore 6을 기록했습니다.
Android Debug·미서명 Release·계측 APK·lint와 API36 선택 2개 검사를 확인했습니다.
iOS/Swift/Xcode 타깃은 저장소에 없었으며 Windows/macOS·물리 Android 인수는 수행하지 않았습니다.

## 2026-10-26: Android 작업 공간

창 크기에 따른 탐색기·세션 탭·분할 작업 공간과 입력/렌더러 수명을 분리했습니다.
당시 JVM 64·Python 98·웹 12 통과, JVM 예상 skip 9개와 Android Debug·계측 APK·lint를 확인했습니다.
API36 작업 공간/입력/볼륨키/계층 선택 8개와 실제 Rust 연결 수명 1개를 통과했습니다.
화면·IME 설정은 원복했으며 전체 API 계측·Windows/macOS GUI·물리 Android 인수로 확대하지 않았습니다.

## 2026-10-26: Android 복귀 응답 확인·제한 재시도

별도 SSH exec의 3초 응답 검사, 중복 검사 병합·검사 중 오래된 입력 취소,
공통 네트워크 재시도 3회·1초 간격과 Android의 이전 창 보관·복구 표시를 도입했습니다.
당시 Rust 코어/UniFFI 31·데스크톱 60·JVM 67·Python 98·웹 13 통과,
격리 SSH/tmux 3.7c 복귀 검사와 API36 선택 2개·Android Debug/계측 APK/lint를 확인했습니다.
정상 확인도 전체 캡처·dim을 수행하던 동작은 후속 변경으로 분리했습니다. 상세는 Git 이력에서 찾습니다.

## 2026-10-26: 정상 복귀 표시 분리·유지 끔 자동 복귀

정상 응답 확인에서는 입력만 잠시 막고, 실제 복구에서만 이전 창 전체와 dim을 유지하도록 분리했습니다.
백그라운드 유지 설정을 끈 연결은 복귀 시 같은 탭·세션으로 한 번 재연결하며 수동 종료·실패·삭제 세션을 보존합니다.
당시 Rust 코어/UniFFI 31·JVM 69·Python 98·웹 13 통과, JVM 예상 skip 9와 Debug/계측 APK/lint를 확인했습니다.
격리 SSH/tmux 3.7c 복귀/무응답 1개와 전용 API36 실제 Rust 연결 수명 1개를 통과했습니다.
물리 기기·네트워크 전환·장시간 계측·Windows/macOS GUI는 수행하지 않았습니다.

## 2026-10-26: Android 스크롤 경계·탭별 글자 크기

숨긴 shell의 가짜 가로 overflow를 제한하고 탭별 폰트 핀치·최신 기본값 초기화를 추가했습니다.
당시 JVM 71·Python 98·웹 16 통과, JVM 예상 skip 9와 Android Debug/계측 APK/lint를 확인했습니다.
전용 API36의 결정적 연결 fixture로 작업 공간 2개·pane 렌더러 1개를 통과했습니다.
실제 SSH 재접속·물리 기기·전체 API 계측·Windows/macOS GUI는 수행하지 않았습니다.

## 2026-10-26: 터미널 전환 효과 범위 축소

터미널 크기 변경은 애니메이션 없이 교체하고 재접속 복구에서만 기본 fade·선택형 blur/크기 효과를 적용했습니다.
당시 웹 18개와 Android Debug 빌드로 확인했으며 계측·실제 SSH 재접속·네이티브 GUI는 재검사하지 않았습니다.

## 2026-10-26: 설정 상세·공통 조합키 입력

설정 상세 화면과 하단 Ctrl·Shift·Alt 토글, 공통 Rust 키 인코더를 추가했습니다.
당시 core·UniFFI 34·데스크톱 62·JVM 74·웹 21·Python 98개와 Android Debug/lint를 통과했습니다.
JVM 예상 skip 9·Rust SSH ignore 6·Keychain ignore 1은 실기 통과로 계산하지 않았습니다.
macOS ARM64 Debug 패키징·ad-hoc 서명을 확인했으며 연결된 Android 기기와 macOS GUI 인수는 없었습니다.

## 2026-10-26: 터미널 크기·글자 크기 초기화·IME 수명

resize 최신값 병합·실패 후 재시도, 연결별 폰트 초기화 FAB와 데스크톱 Ctrl+휠을 추가했습니다.
Android 재사용 렌더러의 IME 세대와 스냅샷 복원 중 조합 위치를 보완했습니다.
당시 core·UniFFI 35·데스크톱 63·JVM 76·웹 25·Python 98 통과,
JVM 예상 skip 9·Rust SSH ignore 6·Keychain ignore 1과 Android Debug/계측 APK/lint를 확인했습니다.
실제 GUI/IME·SSH/TUI·물리 기기·Windows 네이티브 검사는 수행하지 않았습니다.

## 2026-10-26: tmux 제목·탭 이름

기존 메타데이터 구독으로 세션·활성 창/pane 제목과 전환을 GUI 탭·Android 연결 선택 이름에 반영했습니다.
빈 값·중복·제어 문자를 정리하고, 데스크톱 말줄임·툴팁과 재접속·세션 소실 중 마지막 이름 보존을 추가했습니다.
당시 Rust 데스크톱 64·JVM 77·Python 98 통과, Keychain ignore 1·JVM 예상 skip 9와
격리 SSH/tmux 3.7c 제목 구독 1개·Android Debug APK의 ID/버전/SDK/네 ABI/서명을 확인했습니다.
Android lint·계측·실제 GUI/IME와 Windows 네이티브·macOS GUI 인수는 수행하지 않았습니다.

## 2026-10-27: 앱 아이콘 갱신

dagali 아이콘을 둥근 ㄱ자 엔터 머리와 두 pane의 터미널 얼굴로 갱신하고 공통 원본·미리보기·Android 컬러/단색·macOS ICNS·Windows ICO를 재생성했습니다.
당시 재생성 일치·정적 크기별 미리보기·Android 안전 영역과 APK 픽셀 일치, Debug·lint(오류 0·기존 경고 12),
APK ID/버전/SDK/네 ABI/서명, macOS ARM64 ad-hoc 패키징·strict 서명과 ICNS 연결, Windows ICO 7개 크기를 확인했습니다.
Python 98·문서 링크·대시보드 스키마 검사는 통과했으며 실제 Android 런처·macOS Dock/Finder·Windows 네이티브·GUI 검사는 수행하지 않았습니다.

## 2026-10-27: 탭 제목 길이·모바일 계층 정리

탭 제목 앞 20글자·호스트 별도 줄, 모바일 상단 세션명과 연결 메뉴의 tmux 관리·도구 전환,
계층의 두 줄 제목·개수 배지·우측 펼침을 정리했습니다.
당시 Rust 데스크톱 64·JVM 77 통과, Keychain ignore 1·JVM 예상 skip 9와 Android Debug/계측 APK 컴파일·lint를 확인했습니다.
실제 Android 조작·macOS GUI/IME·Windows 네이티브 검사는 수행하지 않았습니다.

## 과거 파일 찾기

정리 직전 기준 커밋은 `5fa6b99`입니다.

```sh
git show 5fa6b99:NEXT_WORK.md
git show 5fa6b99:docs/VALIDATION.md
git show 5fa6b99:docs/ARCHITECTURE_MIGRATION.md
git show 5fa6b99:docs/migration/R1_BASELINE.md
git show 5fa6b99:scripts/verify_windows_installers.py
git log --all --oneline -- docs scripts
```

Git 제외인 `artifacts/`·`.tools/`의 로컬 로그·화면은 Git 이력에 포함되지 않습니다.
원문 요구사항과 SHA-256은 [KICKOFF_PROMPT.md](../KICKOFF_PROMPT.md)에 그대로 보존합니다.
