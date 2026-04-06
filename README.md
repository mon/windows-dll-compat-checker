# Windows DLL compatibility checker

This tool takes a bunch of input files from a program/DLL you have, and checks
that all of the imports are actually available. It was specifically made to
check that a DLL that I need to work on Windows XP will actually load on XP.

It also contains logic to take a System32 folder full of DLLs and export them
to a .ini file, so you don't need to parse a bunch of PE files every time.

There are pre-generated .ini files in the `premade_ini` folder for Windows XP as
well as the Windows 7 + XP images that Konami uses for their arcade machines.

The release build embeds all these .ini files for convenience, in a virtual
folder called PREMADE. Available embedded INIs are listed in the `--help` output.

Finally, it checks the SubsystemVersion of EXE files, because if that is larger
than the currently running OS, it will fail to run entirely. Interestingly, DLLs
do not have that same limitation, and Windows XP will happily load a DLL that
says it needs Windows 10 as long as the EXE loading it specifies XP.

## Install

There are binaries for Windows/Linux/Macos in the Releases section. Or if you
have rust installed, you can run:
```shell
cargo install --locked --git https://github.com/mon/windows-dll-compat-checker.git
```

## Running

You feed it a list of binaries, and a list of `--system/-s` DLLs that it will
check against:
```shell
# use a premade .ini
windows_dll_compat_checker my_cool_program.exe --system PREMADE/winxp_x86_64.ini
# use an actual folder, it will read every DLL inside (not recursively)
windows_dll_compat_checker my_cool_program.exe --system X:/Windows/System32
# override any detected SubsystemVersion with _WIN32_WINNT version constant,
# this example uses XP
windows_dll_compat_checker my_cool_program.exe --system X:/Windows/System32 --os-version 5,1
```

To create a .ini file from an install:
```shell
windows_dll_compat_checker --export-ini windows_exports.ini X:/Windows/System32
```

There are some more advanced ini flags like de-duplicating identical exports,
see [the helper script](./generate_ini_from_windows_folder.sh) I use for
SysWOW64+System32 merging for how it can be used.

## Caveats

This only checks the IAT, so any runtime imports (LoadLibrary + GetProcAddress)
will not be checked.

It's impossible to check (reliably) if a given binary uses the native Thread
Local Storage (TLS) introduced in Windows Vista, so this tool does not check
that either.

## Why not Dependency Walker or its many forks?

I wanted a single binary that runs on Linux for CI purposes, with the explicit
goal of checking against known Windows installs instead of the currently running
one. Dependency Walker, Dependencies, and WinDepends all have nifty looking CLI
modes, but they all run on Windows only. Maybe there's something out there for
my usecase, but I couldn't find it.

## WinSxS is pain

There's some special behaviour in the shell script that pulls in the DLLs for
- WinHTTP 5.1
- Shell Common Controls version 6.0 (Comctl32.dll)
- GDI Plus version 1.0 (GDIplus.dll)
- Visual C++ Run-time Libraries version 6.0

Here is some background reading as to why this is a thing:
- [Everything you Never Wanted to Know about WinSxS](https://web.archive.org/web/20140210074243/http://omnicognate.wordpress.com/2009/10/05/winsxs/)
- [MSDN - Assembly Searching Sequence](https://web.archive.org/web/20140210013454/http://msdn.microsoft.com/en-us/library/aa374224.aspx)
- [MSDN - Shared Assemblies](https://web.archive.org/web/20121121231305/http://msdn.microsoft.com/en-us/library/aa375996.aspx)
- [MSDN - Supported Microsoft Side-by-side Assemblies](https://web.archive.org/web/20130104185057/http://msdn.microsoft.com/en-us/library/aa376609.aspx)

The MDSN links are still live right now, but the Shared Assemblies page no
longer links to "Supported Microsoft Side-by-side Assemblies" which is odd.

## Building etc

**NOTE** the code is almost entirely AI generated, so don't look too far into
it. I've spent a good amount of time actually testing behaviour and edge-cases,
but everyone says that so can you really trust me?

Obviously this README is 100% human, I have some level of pride.

It's Rust, just `cargo build` and all that and it should Just Work.

Some notes:
- The test file binaries are all committed to the repo because they're small,
  and I can't expect everyone to grab a 600MiB docker image to build them
- Started using `pelite`, but because it has such a big focus on zero-copy, it
  fails if a binary has an unaligned IAT, which is a problem I actually ran into
- The notes about SubsystemVersion are correct and I explicitly tested them in
  an XP VM to be sure, because there's barely anything online about it, and an
  incorrect StackOverflow answer...
