; HCS Local AI v3.0.0 Windows Installer Script (Inno Setup)
#define MyAppName "HCS Local AI"
#define MyAppVersion "3.0.0"
#define MyAppPublisher "HCS"
#define MyAppURL "https://github.com/timfromhcs/hcs-local-ai"
#define MyAppExeName "hcs-daemon.exe"

[Setup]
AppId={{D37E8863-A2E4-4861-9128-44A7EB23FF92}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\HCS-Local-AI
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE-MIT
OutputDir=..\dist
OutputBaseFilename=HCS-Local-AI-v3.0.0-Setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\hcs-daemon.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\config.yaml"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\start.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\stop.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\hcs-aider.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dashboard\*"; DestDir: "{app}\dashboard"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\models\*\manifest.yaml"; DestDir: "{app}\models"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\harnesses\*"; DestDir: "{app}\harnesses"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\start.bat"; WorkingDir: "{app}"; IconFilename: "{app}\{#MyAppExeName}"
Name: "{autoprograms}\Stop {#MyAppName}"; Filename: "{app}\stop.bat"; WorkingDir: "{app}"
Name: "{autoprograms}\HCS Aider Coding Studio"; Filename: "{app}\hcs-aider.bat"; WorkingDir: "{app}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\start.bat"; WorkingDir: "{app}"; IconFilename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\start.bat"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: shellexec postinstall nowait skipifsilent
