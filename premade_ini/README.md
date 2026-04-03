Created using the `generate_ini_from_windows_folder.sh` script in the parent folder.

The `konami_*` inis are from my arcade cabs.

`winxp_x86_64` is from a SP2 Professional archive.org ISO I installed into a VM
(turns out there's no SP3 for x64). I then manually bumped the 32bit_dlls ini to
subsystem 5.1, as it only contained 5.0 DLLs (windows 2000).

I then created windows_base.ini (and modified the other common inis) by running:
```shell
cargo run --release -- --in-place --merge-common premade_ini/windows_base.ini \
    premade_ini/konami_win7_museca_x86_64_common.ini \
    premade_ini/winxp_x86_64_common.ini \
    premade_ini/konami_winxp_jubeat_i686.ini
```
