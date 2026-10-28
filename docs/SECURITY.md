# 보안 모델과 배포 전 확인

현재 Rust 앱의 동작과 검사 범위입니다. 과거 JVM 패키징·보안 검사 수치는 [HISTORY.md](HISTORY.md)에 요약합니다.

## 연결·저장 경계

- 최초 SSH 호스트키는 SHA-256 지문을 사용자가 확인합니다. 저장된 지문 불일치는 인증 전에 차단하며 자동 재시도하지 않습니다.
- Android Keystore, macOS Keychain, Windows CURRENT_USER DPAPI로 자격 증명을 보호합니다.
  Windows는 참조별 entropy·해시 파일명·암호문 교체 저장을 사용합니다. 보호 저장 실패 시 평문으로 대체하지 않습니다.
- 비밀번호·개인키·암호는 실행 중 메모리에 존재합니다. 모든 런타임의 메모리 완전 삭제를 보장하지 않습니다.
- 복구 중 입력을 차단하고 실패한 입력을 자동 재전송하지 않습니다. 연결 종료는 tmux 세션 삭제 명령을 보내지 않습니다.
- 원격 출력·사용자 입력·자격 증명을 기본 로그로 기록하지 않습니다. 진단과 수집 보고서에 비밀정보를 포함하지 않습니다.

WebView는 고정 HTTPS appassets origin의 번들만 사용합니다. 외부 탐색·네트워크 요청·file/content 접근·
권한 요청·OSC52 clipboard 접근을 차단하며 bridge는 origin·main frame·타입·크기·대상 세대를 검사합니다.
클립보드는 사용자의 복사·붙여넣기 동작에 사용합니다.

Drive 파일 동기화는 호스트·설정의 허용 필드만 공유합니다. 비밀번호·개인키·키 암호·credentialRef·
신뢰 지문·매크로·터미널 기록은 제외합니다. 주소·사용자명은 공유 파일에 평문으로 남으며 별도 종단간 암호화는 없습니다.
가져온 호스트의 인증은 로컬에서 다시 등록합니다. [동기화 범위](CLOUD_SYNC.md)를 따릅니다.

## 저장소와 CI

`scripts/scan_secrets.py`는 Gitleaks로 Git 관리 작업 파일·전체 staged snapshot·refs 이력을 검사합니다.
탐지 값은 출력하지 않습니다. 임시 SSH 키·설정·로그·빌드 출력은 Git에서 제외합니다.
CI는 `contents: read`, Action SHA 고정, `persist-credentials: false`를 사용합니다.
`pull_request_target`이나 자동 release 게시는 사용하지 않습니다. 산출물은 지정 APK·검사 보고서,
수동 Windows workflow의 ZIP·manifest·해시로 제한합니다.

Wrapper 배포/JAR checksum, Gradle verification metadata·dependency lock, Cargo.lock, npm lock을 유지합니다.
고정 값을 바꿀 때 출처와 변경 이유를 검토하며 검증 실패를 피하기 위한 자동 재생성은 하지 않습니다.
원문 요구사항은 `check_prompt.py`, Wrapper는 `verify_wrapper.py`로 검사합니다.

## 의존성·라이선스·lint

`writeDependencyInventory`는 Gradle의 실제 선택 버전·artifact 해시·사용 범위와 그래프를 수집합니다.
`audit_dependencies.py`는 이 inventory, `Cargo.lock`의 crates.io 패키지, npm lock을 OSV에 대조합니다.
필수 그래프 누락·오래된 입력·해시 오류·crates.io 외 Cargo 출처·OSV 통신 실패는 실패로 처리합니다.
Rust runtime 범위는 배포 타깃에 링크되는 일반 의존성이며 runtime 권고는 예외로 허용하지 않습니다.
과거 검사 결과의 패키지 수·0건 기록을 현재 버전의 결과로 재사용하지 않습니다.

`verify_licenses.py`는 선택 inventory와 `licenses/dependencies.json`의 원문 해시·고지를 대조합니다.
`refresh_licenses.py`는 검토를 위한 유지보수 도구이며 자동 승인 단계가 아닙니다.
Rust 패키징은 `collect_rust_notices.py`로 Cargo registry의 고지·manifest·DEPENDENCIES.json을 수집합니다.
라이선스 텍스트가 없는 항목은 빈 `licenseFiles`로 드러나며 공개 릴리스 감사 완료를 뜻하지 않습니다.
JediTerm·JVM 고지는 참조 테스트의 의존성 자료이며 Rust 앱이 JVM을 배포한다는 의미가 아닙니다.
[제3자 고지](../THIRD_PARTY_NOTICES.md)를 따릅니다.

lint 검토는 `security/lint-reviews.json`에서 진단 ID·메시지 해시·위치·조건·재검토/만료일을 관리합니다.
`verify_lint.py`는 변경·미검토·만료된 진단을 실패 처리합니다. 경고 수는 실제 실행 기록에만 기재합니다.

재현 명령은 [개발 안내](DEVELOPMENT.md)에 있습니다. 기본 CI와 수동 배포 검사의 범위는 다르며,
기본 CI 성공을 서명·실기·공개 배포 완료로 해석하지 않습니다.

## 남은 확인

- macOS 실제 Keychain 저장·재기동, Windows DPAPI round trip·다른 사용자 격리·변조/키 손실 검사.
- Windows ZIP 실행, 실제 기기·IME·OS 알림, 현재 Rust/Gradle/npm 그래프의 배포 전 감사.
- 서명된 실제 전달물의 고지·키/개인 설정/터미널 기록 제외 확인.
- GitHub Secret scanning·Push protection 활성 상태 확인. 현재 문서만으로 활성 여부를 단정하지 않습니다.

Windows 실행 확인은 사용자 담당입니다. [남은 작업](../TASKS.md)과 [Windows 절차](WINDOWS.md)를 따릅니다.
