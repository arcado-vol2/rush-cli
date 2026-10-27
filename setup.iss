#define AppName      "rush-cli"
#define AppPublisher "arcado"
#define AppExeName   "rush-cli.exe"
#define AppExe = "target\release\" + AppExeName

#ifndef AppVersion
  #define CargoToml "Cargo.toml"
  #define TomlHandle FileOpen(CargoToml)
  #if !TomlHandle
    #error Cannot open `Cargo.toml'
  #endif
  #define AppVersion ""
  #define InPackage 0
  #define TomlLine ""
  #define TomlRest ""
  #sub ReadTomlLine
    #expr TomlLine = Trim(FileRead(TomlHandle))
    #if Copy(TomlLine, 1, 1) == "["
      #expr InPackage = (TomlLine == "[package]")
    #elif InPackage && Pos("version", TomlLine) == 1 && Pos('"', TomlLine) > 0
      #expr TomlRest = Copy(TomlLine, Pos('"', TomlLine) + 1)
      #expr AppVersion = Copy(TomlRest, 1, Pos('"', TomlRest) - 1)
    #endif
  #endsub
  #for {0; !FileEof(TomlHandle) && AppVersion == ""; 0} ReadTomlLine
  #expr FileClose(TomlHandle)
  #if AppVersion == ""
    #error Cannot read version from [package] in Cargo.toml
  #endif
#endif


[Setup]
AppId={{72BE8182-0591-4ED5-8E2A-56788E0E0CBA}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
ChangesEnvironment=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=output
OutputBaseFilename={#AppName}-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
UninstallDisplayIcon={app}\{#AppExeName}

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
russian.AddToPath=Добавить %1 в переменную PATH
russian.AdditionalTasks=Дополнительно:
english.AddToPath=Add %1 to the PATH environment variable
english.AdditionalTasks=Additional tasks:

[Tasks]
Name: "addtopath"; Description: "{cm:AddToPath,{#AppName}}"; GroupDescription: "{cm:AdditionalTasks}"

[Files]
Source: {#AppExe}; DestDir: "{app}"; Flags: ignoreversion

[Code]
const
  SystemEnvKey = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';
  UserEnvKey   = 'Environment';

function EnvRootKey: Integer;
begin
  if IsAdminInstallMode then
    Result := HKEY_LOCAL_MACHINE
  else
    Result := HKEY_CURRENT_USER;
end;

function EnvSubKey: String;
begin
  if IsAdminInstallMode then
    Result := SystemEnvKey
  else
    Result := UserEnvKey;
end;

procedure AddToPath(Dir: String);
var
  Paths: String;
begin
  if not RegQueryStringValue(EnvRootKey, EnvSubKey, 'Path', Paths) then
    Paths := '';

  if Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Paths) + ';') > 0 then
    Exit;

  if (Paths <> '') and (Paths[Length(Paths)] <> ';') then
    Paths := Paths + ';';
  Paths := Paths + Dir;

  RegWriteExpandStringValue(EnvRootKey, EnvSubKey, 'Path', Paths);
end;

procedure RemoveFromPath(Dir: String);
var
  Paths: String;
  P: Integer;
begin
  if not RegQueryStringValue(EnvRootKey, EnvSubKey, 'Path', Paths) then
    Exit;

  Paths := ';' + Paths + ';';
  P := Pos(';' + Uppercase(Dir) + ';', Uppercase(Paths));
  if P = 0 then
    Exit;

  Delete(Paths, P, Length(Dir) + 1);
  Paths := Copy(Paths, 2, Length(Paths) - 2);

  RegWriteExpandStringValue(EnvRootKey, EnvSubKey, 'Path', Paths);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddToPath(ExpandConstant('{app}'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveFromPath(ExpandConstant('{app}'));
end;