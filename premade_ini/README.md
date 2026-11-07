Created using the `generate_ini_from_windows_folder.sh` script in the parent folder.

The `konami_*` inis are from my arcade cabs.

`winxp_x86_64` is from a SP2 Professional archive.org ISO I installed into a VM
(turns out there's no SP3 for x64) and updated using Legacy Update.
I then manually bumped the 32bit_dlls ini to subsystem 5.1, as it only contained
5.0 DLLs (windows 2000).

e.g. for my specific folder full of windows installs:
```shell
WINDOWS_FOLDERS=/my/cool/hdd/various_windows_folders
./generate_ini_from_windows_folder.sh konami_win7_museca "$WINDOWS_FOLDERS/konami_win7_museca_x86_64"
./generate_ini_from_windows_folder.sh konami_winxp_jubeat "$WINDOWS_FOLDERS/konami_winxp_jubeat_i686"
./generate_ini_from_windows_folder.sh winxp "$WINDOWS_FOLDERS/winxp_x86_64"
# I should just make this a flag in the autogen script but whatever
sed -i 's/^max_subsystem_version=5\.0$/max_subsystem_version=5.1/' premade_ini/winxp_x86_64_32bit_dlls.ini
```

I then created windows_base.ini (and modified the other common inis) by running:
```shell
cargo run --release -- --in-place --merge-common premade_ini/windows_base.ini \
    premade_ini/konami_win7_museca_x86_64_common.ini \
    premade_ini/winxp_x86_64_common.ini \
    premade_ini/konami_winxp_jubeat_i686.ini
```
