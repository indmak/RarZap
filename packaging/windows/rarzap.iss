; RarZap Inno Setup 脚本（Windows 安装程序，含卸载流程）。
; 版本号统一：CI 用 /DAppVersion=<cargo版本> 传入；本地手动构建时改 #define 默认值。
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef SrcDir
  #define SrcDir "..\..\target\release"
#endif

#define AppName "RarZap"
#define AppId "{{A276EF31-2870-4B49-8FB8-B9D9ABD0120E}"
#define AppPublisher "RarZap"
#define AppURL "https://github.com/indmak/RarZap"

[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
AppUpdatesURL={#AppURL}/releases
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
; 安装程序自身图标 / 安装过程图标 / 卸载显示图标（三处缺一不可，见命名文档 checklist）
SetupIconFile=..\..\assets\icons\rarzap.ico
WizardStyle=modern
UninstallDisplayIcon={app}\rarzap.exe
UninstallDisplayName={#AppName}
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
OutputDir=..\..\target\innosetup
OutputBaseFilename=RarZap-{#AppVersion}-setup
LicenseFile=..\..\LICENSE-MIT
; 无签名阶段：SignPath 开源免费计划获批后再加 SignTool 配置（见命名文档）

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#SrcDir}\rarzap.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; 开始菜单 + 桌面（可选）+ 任务栏固定用的都是 exe 内嵌图标（build.rs 已链入 rarzap.ico）
Name: "{group}\{#AppName}"; Filename: "{app}\rarzap.exe"
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\rarzap.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Run]
Filename: "{app}\rarzap.exe"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
