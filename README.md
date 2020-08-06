## Reading config over the network with GRUB and UEFI

```sh
insmod efinet
net_add_addr local efinet0 192.168.90.100
source (tftp,192.168.90.1)/samwise.cfg
```

In a non-janky setup (I have an ethernet cable directly connecting my RPi to my computer), you can use DHCP instead of `net_add_addr` (and maybe even get DNS
for the TFTP server address!).