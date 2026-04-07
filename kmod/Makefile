KDIR ?= /lib/modules/$(shell uname -r)/build

all:
	$(MAKE) -C $(KDIR) M=$(shell pwd) modules

clean:
	$(MAKE) -C $(KDIR) M=$(shell pwd) clean

load: all
	-sudo rmmod hid_xbelite2 2>/dev/null
	sudo insmod hid_xbelite2.ko

unload:
	-sudo rmmod hid_xbelite2

.PHONY: all clean load unload
