# CMake toolchain file for the Windows release package (tools/scripts/package.ps1, M13-05).
#
# package.ps1 builds with `-C target-feature=+crt-static`, so td2192.exe needs neither
# VCRUNTIME140.dll nor MSVCP140.dll (the Visual C++ Redistributable, missing on a fresh Windows).
# Rust code and the C/C++ that the cc crate builds (ymfm, cxx) follow that flag; SDL 3, which
# sdl3-sys builds through CMake, picks its C runtime from CMAKE_MSVC_RUNTIME_LIBRARY instead.
# This file sets it to the static runtime; package.ps1 passes it in CMAKE_TOOLCHAIN_FILE.
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
