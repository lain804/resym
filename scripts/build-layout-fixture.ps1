param([string]$Cl = 'cl.exe', [string]$Link = 'link.exe')
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$build = Join-Path $repo 'target/layout-fixture'
New-Item -ItemType Directory -Force -Path $build | Out-Null
$source = Join-Path $repo 'resym_core/tests/data/layout_regressions.cpp'
$object = Join-Path $build 'layout_regressions.obj'
$pdb = Join-Path $repo 'resym_core/tests/data/layout_regressions.pdb'
$dll = Join-Path $build 'layout_regressions.dll'
& $Cl /nologo /c /Z7 /GS- /std:c++17 "/Fo$object" $source
if ($LASTEXITCODE -ne 0) { throw 'Fixture compilation failed' }
& $Link /nologo /dll /noentry /nodefaultlib /debug:full /opt:noref /machine:x64 "/pdb:$pdb" "/out:$dll" $object
if ($LASTEXITCODE -ne 0) { throw 'Fixture linking failed' }
