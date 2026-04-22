KDIR ?= /lib/modules/$(shell uname -r)/build

all:
	$(MAKE) -C $(KDIR) M=$(shell pwd) CC=clang CXX=clang++ LD=ld.lld modules

clean:
	$(MAKE) -C $(KDIR) M=$(shell pwd) clean

load: all
	-sudo rmmod xbelite2 2>/dev/null
	sudo insmod xbelite2.ko

unload:
	-sudo rmmod xbelite2

.PHONY: all clean load unload
