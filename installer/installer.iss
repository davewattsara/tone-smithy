; Tone Smithy — Inno Setup installer script.
;
; Compiled by `cargo xtask dist` on Windows, which passes the version and the
; staging directory via /D defines:
;
;     iscc /DAppVersion=1.0.0 /DDistDir=C:\...\target\dist\1.0.0 installer\installer.iss
;
; It can also be compiled standalone for a quick check; the defaults below then
; apply. See docs/planning/07-distribution/packaging.md for the spec.

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif

; Where the staged artefacts live (exe, licences, README, etc.). Defaults to the
; conventional dist path for the current version so a manual compile still works.
#ifndef DistDir
  #define DistDir "..\target\dist\" + AppVersion
#endif

#define AppName "Tone Smithy"
#define AppPublisher "Tone Smithy"
#define AppExeName "tonesmithy.exe"
#define AppId "{{8E2B6F4A-7C1D-4E5A-9B3F-2A6C8D0E1F23}"

; Optional application icon. Authored separately as a binary art asset; the
; script tolerates its absence so builds stay green until it lands.
#define IconFile "..\assets\icons\tonesmithy.ico"
#define HasIcon FileExists(AddBackslash(SourcePath) + IconFile)

[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
; Per-user install: no UAC elevation, no machine-wide footprint. This is the
; only install mode in v1 (see packaging spec, "Out of scope").
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile={#DistDir}\LICENSE-MIT
OutputDir=.
OutputBaseFilename=tonesmithy-{#AppVersion}-windows-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayName={#AppName} {#AppVersion}
#if HasIcon
SetupIconFile={#IconFile}
UninstallDisplayIcon={app}\{#AppExeName}
#else
UninstallDisplayIcon={app}\{#AppExeName}
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "associate"; Description: "Associate .tsmith preset files with {#AppName}"; GroupDescription: "File associations:"

[Files]
Source: "{#DistDir}\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\README.txt"; DestDir: "{app}"; Flags: ignoreversion isreadme
Source: "{#DistDir}\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#DistDir}\THIRD-PARTY-LICENSES.txt"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
#if HasIcon
Source: "{#IconFile}"; DestDir: "{app}"; DestName: "tonesmithy.ico"; Flags: ignoreversion
#endif

[Icons]
#if HasIcon
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\tonesmithy.ico"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\tonesmithy.ico"; Tasks: desktopicon
#else
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon
#endif
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"

[Registry]
; .tsmith file association (per-user, under HKCU — removed cleanly on uninstall).
Root: HKCU; Subkey: "Software\Classes\.tsmith"; ValueType: string; ValueName: ""; ValueData: "ToneSmithy.Preset"; Flags: uninsdeletevalue; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\ToneSmithy.Preset"; ValueType: string; ValueName: ""; ValueData: "Tone Smithy Preset"; Flags: uninsdeletekey; Tasks: associate
#if HasIcon
Root: HKCU; Subkey: "Software\Classes\ToneSmithy.Preset\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\tonesmithy.ico"; Flags: uninsdeletekey; Tasks: associate
#else
Root: HKCU; Subkey: "Software\Classes\ToneSmithy.Preset\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExeName},0"; Flags: uninsdeletekey; Tasks: associate
#endif
Root: HKCU; Subkey: "Software\Classes\ToneSmithy.Preset\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: associate

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

; The uninstaller removes the install directory, Start Menu group, shortcuts, and
; the file-association keys above. It does NOT delete %APPDATA%\Tone Smithy\
; (user settings + presets); that data is left untouched by design.
