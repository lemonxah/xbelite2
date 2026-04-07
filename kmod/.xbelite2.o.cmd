savedcmd_xbelite2.o := ld -m elf_x86_64 -z noexecstack --no-warn-rwx-segments   -r -o xbelite2.o @xbelite2.mod  ; /usr/lib/modules/6.19.11-arch1-1/build/tools/objtool/objtool --hacks=jump_label --hacks=noinstr --hacks=skylake --ibt --orc --retpoline --rethunk --sls --static-call --uaccess --prefix=16  --link  --module xbelite2.o

xbelite2.o: $(wildcard /usr/lib/modules/6.19.11-arch1-1/build/tools/objtool/objtool)
