savedcmd_xbelite2.mod := printf '%s\n'   xbelite2_c.o xbelite2_rust.o | awk '!x[$$0]++ { print("./"$$0) }' > xbelite2.mod
