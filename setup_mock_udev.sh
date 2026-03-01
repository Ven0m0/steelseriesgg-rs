#!/bin/bash
set -e

mkdir -p mock_udev/include
mkdir -p mock_udev/lib
mkdir -p mock_udev/pkgconfig

cat << 'C_EOF' > mock_udev/include/libudev.h
#ifndef MOCK_LIBUDEV_H
#define MOCK_LIBUDEV_H

struct udev;
struct udev_enumerate;
struct udev_list_entry;
struct udev_device;

struct udev *udev_new(void);
struct udev *udev_ref(struct udev *udev);
struct udev *udev_unref(struct udev *udev);

struct udev_enumerate *udev_enumerate_new(struct udev *udev);
struct udev_enumerate *udev_enumerate_ref(struct udev_enumerate *udev_enumerate);
struct udev_enumerate *udev_enumerate_unref(struct udev_enumerate *udev_enumerate);
int udev_enumerate_add_match_subsystem(struct udev_enumerate *udev_enumerate, const char *subsystem);
int udev_enumerate_add_match_sysname(struct udev_enumerate *udev_enumerate, const char *sysname);
int udev_enumerate_scan_devices(struct udev_enumerate *udev_enumerate);
struct udev_list_entry *udev_enumerate_get_list_entry(struct udev_enumerate *udev_enumerate);

struct udev_list_entry *udev_list_entry_get_next(struct udev_list_entry *list_entry);
const char *udev_list_entry_get_name(struct udev_list_entry *list_entry);

struct udev_device *udev_device_new_from_syspath(struct udev *udev, const char *syspath);
struct udev_device *udev_device_new_from_devnum(struct udev *udev, char type, unsigned int devnum);
struct udev_device *udev_device_ref(struct udev_device *udev_device);
struct udev_device *udev_device_unref(struct udev_device *udev_device);
const char *udev_device_get_devnode(struct udev_device *udev_device);
struct udev_device *udev_device_get_parent_with_subsystem_devtype(struct udev_device *udev_device, const char *subsystem, const char *devtype);
const char *udev_device_get_sysattr_value(struct udev_device *udev_device, const char *sysattr);
const char *udev_device_get_syspath(struct udev_device *udev_device);
const char *udev_device_get_action(struct udev_device *udev_device);
const char *udev_device_get_devpath(struct udev_device *udev_device);

struct udev_monitor;
struct udev_monitor *udev_monitor_new_from_netlink(struct udev *udev, const char *name);
struct udev_monitor *udev_monitor_unref(struct udev_monitor *udev_monitor);
int udev_monitor_enable_receiving(struct udev_monitor *udev_monitor);
int udev_monitor_get_fd(struct udev_monitor *udev_monitor);
struct udev_device *udev_monitor_receive_device(struct udev_monitor *udev_monitor);
int udev_monitor_filter_add_match_subsystem_devtype(struct udev_monitor *udev_monitor, const char *subsystem, const char *devtype);

#define udev_list_entry_foreach(entry, first) \
    for (entry = first; entry != NULL; entry = udev_list_entry_get_next(entry))

#endif
C_EOF

cat << 'C_EOF' > mock_udev/mock_udev.c
#include "include/libudev.h"
#include <stddef.h>
#include <stdarg.h>

struct udev *udev_new(void) { return NULL; }
struct udev *udev_ref(struct udev *udev) { return udev; }
struct udev *udev_unref(struct udev *udev) { return NULL; }

struct udev_enumerate *udev_enumerate_new(struct udev *udev) { return NULL; }
struct udev_enumerate *udev_enumerate_ref(struct udev_enumerate *udev_enumerate) { return udev_enumerate; }
struct udev_enumerate *udev_enumerate_unref(struct udev_enumerate *udev_enumerate) { return NULL; }
int udev_enumerate_add_match_subsystem(struct udev_enumerate *udev_enumerate, const char *subsystem) { return 0; }
int udev_enumerate_add_match_sysname(struct udev_enumerate *udev_enumerate, const char *sysname) { return 0; }
int udev_enumerate_scan_devices(struct udev_enumerate *udev_enumerate) { return 0; }
struct udev_list_entry *udev_enumerate_get_list_entry(struct udev_enumerate *udev_enumerate) { return NULL; }

struct udev_list_entry *udev_list_entry_get_next(struct udev_list_entry *list_entry) { return NULL; }
const char *udev_list_entry_get_name(struct udev_list_entry *list_entry) { return NULL; }

struct udev_device *udev_device_new_from_syspath(struct udev *udev, const char *syspath) { return NULL; }
struct udev_device *udev_device_new_from_devnum(struct udev *udev, char type, unsigned int devnum) { return NULL; }
struct udev_device *udev_device_ref(struct udev_device *udev_device) { return udev_device; }
struct udev_device *udev_device_unref(struct udev_device *udev_device) { return NULL; }
const char *udev_device_get_devnode(struct udev_device *udev_device) { return NULL; }
struct udev_device *udev_device_get_parent_with_subsystem_devtype(struct udev_device *udev_device, const char *subsystem, const char *devtype) { return NULL; }
const char *udev_device_get_sysattr_value(struct udev_device *udev_device, const char *sysattr) { return NULL; }
const char *udev_device_get_syspath(struct udev_device *udev_device) { return NULL; }
const char *udev_device_get_action(struct udev_device *udev_device) { return NULL; }
const char *udev_device_get_devpath(struct udev_device *udev_device) { return NULL; }

struct udev_monitor *udev_monitor_new_from_netlink(struct udev *udev, const char *name) { return NULL; }
struct udev_monitor *udev_monitor_unref(struct udev_monitor *udev_monitor) { return NULL; }
int udev_monitor_enable_receiving(struct udev_monitor *udev_monitor) { return 0; }
int udev_monitor_get_fd(struct udev_monitor *udev_monitor) { return -1; }
struct udev_device *udev_monitor_receive_device(struct udev_monitor *udev_monitor) { return NULL; }
int udev_monitor_filter_add_match_subsystem_devtype(struct udev_monitor *udev_monitor, const char *subsystem, const char *devtype) { return 0; }

// Dummy stubs to prevent link errors
void __assert_fail(const char *__assertion, const char *__file, unsigned int __line, const char *__function) {}

C_EOF

gcc -c mock_udev/mock_udev.c -o mock_udev/mock_udev.o
ar rcs mock_udev/lib/libudev.a mock_udev/mock_udev.o

cat << 'C_EOF' > mock_udev/pkgconfig/libudev.pc
Name: libudev
Description: Library to access udev device information
Version: 247
Libs: -L${pcfiledir}/../lib -ludev
Cflags: -I${pcfiledir}/../include
C_EOF

echo "Setup complete."
